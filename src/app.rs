//! Application life cycle: single instance, server, tray icon, clean shutdown.
#[cfg(has_tray)]
use crate::log::log_error;
use crate::log::{log_info, log_warn};
use anyhow::{bail, Result};
use std::{net::TcpListener, sync::OnceLock, time::Duration};
use tokio::sync::watch;

static QUIT: OnceLock<watch::Sender<bool>> = OnceLock::new();

fn quit_tx() -> &'static watch::Sender<bool> {
    QUIT.get_or_init(|| watch::channel(false).0)
}

/// Asks the application to shut down cleanly (tray menu, `quit` command, Ctrl+C…).
pub fn request_quit() {
    if !quit_tx().send_replace(true) {
        log_info!("quit requested"); // only once, even if several sources ask for it
    }
}

/// Completes once shutdown has been requested (even if the request was made before the call).
pub async fn wait_quit() {
    let mut rx = quit_tx().subscribe();
    let _ = rx.wait_for(|v| *v).await;
}

/// Starts the application. `tray`: show the tray icon (if the system supports it).
/// `foreground`: stay attached to the terminal instead of detaching (Linux).
pub fn run(open: bool, tray: bool, foreground: bool) -> Result<()> {
    // Started from a terminal, detach and give the prompt back: no terminal window has to stay open.
    #[cfg(all(unix, has_tray))]
    if tray && !foreground && detach_from_terminal(open)? {
        return Ok(());
    }
    let _ = foreground;
    let ip = crate::net::bind_ip()?;
    let listener = match TcpListener::bind(std::net::SocketAddr::new(ip, crate::net::PORT)) {
        Ok(l) => l,
        Err(e) => return already_running(open, e),
    };
    log_info!("PrintSearch {} starting on {} {} ({})", crate::update::current(), std::env::consts::OS, std::env::consts::ARCH, crate::net::url());

    #[cfg(has_tray)]
    if tray {
        return run_with_tray(listener, open);
    }
    let _ = tray; // without a tray available: run in the terminal
    run_foreground(listener, open)
}

/// Unix: when started from a terminal, relaunches PrintSearch in a new session without a terminal,
/// waits until it answers, then gives the prompt back. Returns `true` when the current process has nothing left to do.
#[cfg(all(unix, has_tray))]
fn detach_from_terminal(open: bool) -> Result<bool> {
    use std::io::IsTerminal;
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;

    let from_terminal = std::io::stdin().is_terminal() || std::io::stderr().is_terminal();
    if std::env::var_os("PRINTSEARCH_DETACHED").is_some() || !from_terminal {
        return Ok(false); // started from a menu, a service, or already detached
    }
    crate::log::set_echo(false); // here the program talks by itself: no log lines mixed into its messages
    if probe() {
        log_info!("PrintSearch is already running: reusing the existing instance");
        println!("PrintSearch is already running ({}).", crate::net::url());
        if open {
            let _ = ::open::that(crate::net::url());
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
    let child = cmd.spawn()?;
    log_info!("detached into the background (pid {})", child.id());
    for _ in 0..40 {
        std::thread::sleep(Duration::from_millis(200));
        if probe() {
            println!("PrintSearch is running in the background ({}). Look for its icon in the system tray.", crate::net::url());
            println!("To stop it: `printsearch quit`, or Quit in the tray menu.");
            return Ok(true);
        }
    }
    println!("PrintSearch is starting in the background. If nothing appears, run `printsearch logs` to see why.");
    Ok(true)
}

/// The port is taken: if it is already PrintSearch, just open its interface.
fn already_running(open: bool, e: std::io::Error) -> Result<()> {
    if probe() {
        log_info!("PrintSearch is already running: reusing the existing instance");
        println!("PrintSearch is already running ({}).", crate::net::url());
        if open {
            let _ = ::open::that(crate::net::url());
        }
        return Ok(());
    }
    bail!("port 8787 is not available ({e}); is another program using it?")
}

/// True if the instance listening on the port really is PrintSearch (minimal HTTP request, no runtime).
fn probe() -> bool {
    use std::io::{Read, Write};
    let addr = crate::net::probe_addr();
    let Ok(mut s) = std::net::TcpStream::connect_timeout(&addr, Duration::from_secs(1)) else { return false };
    let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = s.set_write_timeout(Some(Duration::from_secs(2)));
    if s.write_all(b"GET /api/status HTTP/1.1\r\nHost: localhost:8787\r\nConnection: close\r\n\r\n").is_err() {
        return false;
    }
    let mut buf = String::new();
    let _ = s.take(8192).read_to_string(&mut buf);
    buf.contains("\"makerworld\"") && buf.contains("\"version\"")
}

fn run_foreground(listener: TcpListener, open: bool) -> Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    rt.block_on(serve(listener, open))
}

/// Server + signal handling; only returns after shutdown (with a maximum delay of 6 s).
async fn serve(listener: TcpListener, open: bool) -> Result<()> {
    tokio::spawn(async {
        let _ = tokio::signal::ctrl_c().await;
        log_info!("Ctrl+C received");
        request_quit();
    });
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        // SIGTERM (stop requested) and SIGHUP (terminal closed): clean shutdown rather than abrupt
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
    crate::browser::shutdown(); // close the background browser: no orphan process
    log_info!("PrintSearch stopped");
    result
}

/// Server in a thread, tray icon in the main thread (which must run its message loop).
#[cfg(has_tray)]
fn run_with_tray(listener: TcpListener, open: bool) -> Result<()> {
    let lang = crate::config::Config::load().lang.unwrap_or_else(|| "en".into());
    // Even if the server thread crashes, the icon must disappear (otherwise it would stay displayed with nothing behind it).
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
