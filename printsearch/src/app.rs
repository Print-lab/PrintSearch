//! Application lifecycle: single instance, server, tray icon, clean shutdown.
#[cfg(has_tray)]
use crate::log::log_error;
use crate::log::{log_info, log_warn};
use anyhow::{bail, Result};
use crate::server::{bind_addr, display_host, PORT};
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener},
    sync::OnceLock,
    time::{Duration, Instant},
};
use tokio::sync::watch;

/// Address a user types in a browser: always an authority the Host/Origin allow-list accepts
/// (`localhost` for the default or a wildcard bind, otherwise the configured address or name).
pub fn url() -> String {
    format!("http://{}:{PORT}", display_host(&bind_addr()))
}

/// True for an address of this very machine: loopback, or one we are able to bind a socket to.
fn is_local_ip(ip: IpAddr) -> bool {
    ip.is_loopback() || std::net::UdpSocket::bind((ip, 0)).is_ok()
}

/// The addresses of THIS machine that `PRINTSEARCH_BIND` names, in resolver order, without duplicates. An IP literal gives
/// itself (a wildcard is always local); a host name is resolved under its own deadline, and only addresses of this
/// machine are kept: the listener, the instance check and the CLI therefore always talk about the same addresses.
fn resolve_local(bind: &str) -> Vec<IpAddr> {
    if let Ok(ip) = bind.parse::<IpAddr>() {
        return if ip.is_unspecified() || is_local_ip(ip) { vec![ip] } else { vec![] };
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let name = bind.to_string();
    std::thread::spawn(move || {
        use std::net::ToSocketAddrs;
        let found: Vec<IpAddr> = (name.as_str(), PORT).to_socket_addrs().map(|i| i.map(|a| a.ip()).collect()).unwrap_or_default();
        let _ = tx.send(found);
    });
    let mut out: Vec<IpAddr> = Vec::new();
    for ip in rx.recv_timeout(Duration::from_secs(3)).unwrap_or_default() {
        if is_local_ip(ip) && !out.contains(&ip) {
            out.push(ip);
        }
    }
    out
}

/// Where to listen.
fn bind_addrs() -> Vec<SocketAddr> {
    resolve_local(&bind_addr()).into_iter().map(|ip| SocketAddr::new(ip, PORT)).collect()
}

/// Where the CLI and the duplicate-start check look for a running instance: the same addresses, a wildcard becoming loopback.
fn client_targets() -> Vec<SocketAddr> {
    resolve_local(&bind_addr())
        .into_iter()
        .map(|ip| {
            let ip = match ip {
                IpAddr::V4(v) if v.is_unspecified() => IpAddr::V4(Ipv4Addr::LOCALHOST),
                IpAddr::V6(v) if v.is_unspecified() => IpAddr::V6(Ipv6Addr::LOCALHOST),
                other => other,
            };
            SocketAddr::new(ip, PORT)
        })
        .collect()
}

/// A raw HTTP answer is accepted as PrintSearch only if it is `200` with a JSON object that has a string
/// `version` and a boolean `makerworld`: two marker words in any text are not enough.
fn is_printsearch_status(raw: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(raw) else { return false };
    let Some((head, body)) = text.split_once("\r\n\r\n") else { return false };
    let mut first = head.lines().next().unwrap_or("").split_whitespace();
    if !matches!(first.next(), Some("HTTP/1.1" | "HTTP/1.0")) || first.next() != Some("200") {
        return false;
    }
    let Ok(v) = serde_json::from_str::<serde_json::Value>(body.trim()) else { return false };
    v.get("version").map_or(false, |x| x.is_string()) && v.get("makerworld").map_or(false, |x| x.is_boolean())
}

/// The address of the running PrintSearch of this machine, if any. Everything (connect, write, every read) stays
/// within `deadline`.
pub fn probe_until(deadline: Instant) -> Option<SocketAddr> {
    use std::io::{Read, Write};
    for addr in client_targets() {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        let Ok(mut s) = std::net::TcpStream::connect_timeout(&addr, left.min(Duration::from_secs(1))) else { continue };
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        let _ = s.set_write_timeout(Some(left));
        if s.write_all(b"GET /api/status HTTP/1.1\r\nHost: localhost:8787\r\nConnection: close\r\n\r\n").is_err() {
            continue;
        }
        let (mut buf, mut chunk) = (Vec::new(), [0u8; 2048]);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() || buf.len() >= 16384 {
                break;
            }
            let _ = s.set_read_timeout(Some(left));
            match s.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => buf.extend_from_slice(&chunk[..n]),
            }
        }
        if is_printsearch_status(&buf) {
            return Some(addr);
        }
    }
    None
}

pub fn probe() -> Option<SocketAddr> {
    probe_until(Instant::now() + Duration::from_secs(3))
}

pub async fn probe_async() -> Option<SocketAddr> {
    tokio::task::spawn_blocking(probe).await.ok().flatten()
}

/// Asks the running instance of this machine to quit. True only for an HTTP success carrying `{"ok":true}`.
/// Dedicated client: no proxy, no redirect, short timeout, `Host: localhost:8787` (always accepted).
pub async fn local_quit() -> bool {
    let Some(addr) = probe_async().await else { return false };
    let Ok(client) = reqwest::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none()).timeout(Duration::from_secs(5)).build() else {
        return false;
    };
    let Ok(resp) = client.post(format!("http://{addr}/api/quit")).header("Host", "localhost:8787").send().await else { return false };
    resp.status().is_success() && resp.json::<serde_json::Value>().await.ok().and_then(|v| v.get("ok").and_then(|b| b.as_bool())) == Some(true)
}

static QUIT: OnceLock<watch::Sender<bool>> = OnceLock::new();

fn quit_tx() -> &'static watch::Sender<bool> {
    QUIT.get_or_init(|| watch::channel(false).0)
}

/// Requests a clean shutdown of the application (tray menu, `quit` command, Ctrl+C…).
pub fn request_quit() {
    if !quit_tx().send_replace(true) {
        log_info!("quit requested"); // only once, even if several sources ask for it
    }
}

/// Completes when shutdown has been requested (even if the request came before the call).
pub async fn wait_quit() {
    let mut rx = quit_tx().subscribe();
    let _ = rx.wait_for(|v| *v).await;
}

/// Starts the application. `tray`: show the tray icon (if the system supports it).
/// `foreground`: stay attached to the terminal instead of detaching (Linux).
pub fn run(open: bool, tray: bool, foreground: bool) -> Result<()> {
    // Started from a terminal: detach and give the prompt back, so no terminal window has to stay open.
    #[cfg(all(unix, has_tray))]
    if tray && !foreground && detach_from_terminal(open)? {
        return Ok(());
    }
    let _ = foreground;
    let bind = bind_addr();
    let (mut listener, mut in_use, mut last_error) = (None, None, None);
    for addr in bind_addrs() {
        match TcpListener::bind(addr) {
            Ok(l) => {
                listener = Some(l);
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                // a busy candidate means an instance of this user may already run: never start a second one on the next address
                in_use = Some(e);
                break;
            }
            Err(e) => last_error = Some(e),
        }
    }
    let Some(listener) = listener else {
        if let Some(e) = in_use {
            return already_running(open, e);
        }
        let why = last_error.map(|e| format!(": {e}")).unwrap_or_default();
        bail!("cannot listen on {bind}:{PORT} (is the address valid and assigned to this machine, and is the port free?){why}");
    };
    log_info!("PrintSearch {} starting on {} {} ({})", crate::update::current(), std::env::consts::OS, std::env::consts::ARCH, url());

    #[cfg(has_tray)]
    if tray {
        return run_with_tray(listener, open);
    }
    let _ = tray; // no tray available: run in the terminal
    run_foreground(listener, open)
}

/// Unix: if started from a terminal, relaunches PrintSearch in a new session without a terminal,
/// waits until it answers, then returns. Returns `true` when the current process has nothing left to do.
#[cfg(all(unix, has_tray))]
fn detach_from_terminal(open: bool) -> Result<bool> {
    use std::io::IsTerminal;
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;

    let from_terminal = std::io::stdin().is_terminal() || std::io::stderr().is_terminal();
    if std::env::var_os("PRINTSEARCH_DETACHED").is_some() || !from_terminal {
        return Ok(false); // started from a menu, a service, or already detached
    }
    crate::log::set_echo(false); // the program talks to the user itself here: no log lines mixed into its messages
    if probe().is_some() {
        log_info!("PrintSearch is already running: reusing the existing instance");
        println!("PrintSearch is already running ({}).", url());
        if open {
            let _ = ::open::that(url());
        }
        return Ok(true);
    }
    let mut cmd = std::process::Command::new(std::env::current_exe()?);
    cmd.args(std::env::args_os().skip(1))
        .env("PRINTSEARCH_DETACHED", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // SAFETY: only `setsid` is called between fork and exec; it is safe in this context.
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid(); // new session: no controlling terminal, closing the terminal does not kill the application
            Ok(())
        });
    }
    let mut child = cmd.spawn()?;
    log_info!("detached into the background (pid {})", child.id());
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            bail!("PrintSearch could not start in the background ({status}); run `printsearch logs` to see why");
        }
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        if probe_until(deadline.min(now + Duration::from_millis(1500))).is_some() {
            println!("PrintSearch is running in the background ({}). Look for its icon in the system tray.", url());
            println!("To stop it: `printsearch quit`, or Quit in the tray menu.");
            return Ok(true);
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    println!("PrintSearch is starting in the background. If nothing appears, run `printsearch logs` to see why.");
    Ok(true)
}

/// The port is taken: if it is PrintSearch itself, just open its interface.
fn already_running(open: bool, e: std::io::Error) -> Result<()> {
    if probe().is_some() {
        log_info!("PrintSearch is already running: reusing the existing instance");
        println!("PrintSearch is already running ({}).", url());
        if open {
            let _ = ::open::that(url());
        }
        return Ok(());
    }
    bail!("port {PORT} is not available ({e}); is another program using it?")
}

fn run_foreground(listener: TcpListener, open: bool) -> Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    rt.block_on(serve(listener, open))
}

/// Server + signal handling; returns only after shutdown (with a maximum delay of 6 s).
async fn serve(listener: TcpListener, open: bool) -> Result<()> {
    tokio::spawn(async {
        let _ = tokio::signal::ctrl_c().await;
        log_info!("Ctrl+C received");
        request_quit();
    });
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        // SIGTERM (shutdown requested) and SIGHUP (terminal closed): clean rather than abrupt shutdown
        for (kind, name) in [(SignalKind::terminate(), "SIGTERM"), (SignalKind::hangup(), "SIGHUP")] {
            if let Ok(mut s) = signal(kind) {
                tokio::spawn(async move {
                    s.recv().await;
                    log_info!("{name} received");
                    request_quit();
                });
            }
        }
    }

    let result = tokio::select! {
        r = crate::server::run(listener, open) => r,
        _ = async { wait_quit().await; tokio::time::sleep(Duration::from_secs(6)).await; } => {
            log_warn!("shutdown took too long: exiting anyway");
            Ok(())
        }
    };
    crate::browser::shutdown(); // closes the background browser: no orphan process
    log_info!("PrintSearch stopped");
    result
}

/// Server in a thread, tray icon in the main thread (which has to run its message loop).
#[cfg(has_tray)]
fn run_with_tray(listener: TcpListener, open: bool) -> Result<()> {
    let lang = crate::config::Config::load().lang.unwrap_or_else(|| "en".into());
    // Even if the server thread crashes, the icon must disappear (otherwise it would stay on screen with nothing behind it).
    struct StopTrayOnExit;
    impl Drop for StopTrayOnExit {
        fn drop(&mut self) {
            crate::tray::stop();
        }
    }
    let server = std::thread::Builder::new().name("server".into()).spawn(move || -> Result<()> {
        let _stop_tray = StopTrayOnExit;
        let res = tokio::runtime::Builder::new_multi_thread().enable_all().build().map_err(anyhow::Error::from).and_then(|rt| rt.block_on(serve(listener, open)));
        if let Err(e) = &res {
            log_error!("server stopped with an error: {e:#}");
        }
        res
    })?;

    match crate::tray::run(&lang) {
        Ok(()) => request_quit(), // icon closed -> clean server shutdown
        Err(e) => log_warn!("tray icon unavailable ({e:#}); PrintSearch keeps running without it (close it from the web interface or with `printsearch quit`)"),
    }
    match server.join() {
        Ok(r) => r,
        Err(_) => bail!("the server thread crashed (see the log)"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn http(body: &str) -> Vec<u8> {
        format!("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{body}", body.len()).into_bytes()
    }

    #[test]
    fn status_line_is_checked_exactly() {
        let ok = r#"{"makerworld":false,"version":"1"}"#;
        assert!(is_printsearch_status(format!("HTTP/1.1 200 OK\r\n\r\n{ok}").as_bytes()));
        assert!(!is_printsearch_status(format!("HTTP/1.1 2000 OK\r\n\r\n{ok}").as_bytes()));
        assert!(!is_printsearch_status(format!("HTTP/1.1 201 Created\r\n\r\n{ok}").as_bytes()));
    }

    #[test]
    fn every_spelling_of_a_wildcard_is_local() {
        for w in ["0.0.0.0", "::", "0:0:0:0:0:0:0:0"] {
            assert_eq!(resolve_local(w).len(), 1, "{w}");
        }
        assert_eq!(resolve_local("127.0.0.1"), vec!["127.0.0.1".parse::<IpAddr>().unwrap()]);
    }

    #[test]
    fn a_foreign_address_is_not_local() {
        assert!(resolve_local("192.0.2.1").is_empty()); // TEST-NET-1: never assigned to this machine
        assert!(resolve_local("999.1.1.1").is_empty());
    }

    #[test]
    fn real_status_document_is_accepted() {
        assert!(is_printsearch_status(&http(r#"{"thingiverse":false,"makerworld":false,"version":"0.3.0"}"#)));
    }

    #[test]
    fn marker_words_alone_are_not_enough() {
        assert!(!is_printsearch_status(&http(r#""makerworld" "version" nonsense"#)));
        assert!(!is_printsearch_status(&http(r#"["makerworld","version"]"#)));
        assert!(!is_printsearch_status(&http(r#"{"makerworld":"yes","version":"0.3.0"}"#)));
        assert!(!is_printsearch_status(&http(r#"{"makerworld":false,"version":3}"#)));
        assert!(!is_printsearch_status(b"HTTP/1.1 403 Forbidden\r\n\r\n{\"makerworld\":false,\"version\":\"1\"}"));
        assert!(!is_printsearch_status(b"no http at all"));
    }
}
