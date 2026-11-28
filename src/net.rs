//! Network settings: where PrintSearch listens, which Host / Origin it accepts, and what remote clients may do.
//!
//! By default the server listens on 127.0.0.1 only. `PRINTSEARCH_BIND=<ip>` (for example `0.0.0.0`) makes it reachable from
//! other computers; `PRINTSEARCH_ALLOWED_HOSTS=name1,name2` adds the host names or addresses clients use to reach it.
//! The port is fixed (the Thingiverse OAuth redirect depends on it).
//!
//! There is no authentication, so when the server listens beyond this computer, requests that come from *other* computers
//! are limited to searching (see `remote_may`). Anything that acts on your accounts or administers the app (tokens, browser
//! choice, console, logs, quit) stays reserved to this computer.
use anyhow::{anyhow, Result};
use axum::http::Method;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

pub const PORT: u16 = 8787;

fn parse_bind(value: Option<&str>) -> Result<IpAddr> {
    match value.map(str::trim).filter(|v| !v.is_empty()) {
        None => Ok(Ipv4Addr::LOCALHOST.into()),
        Some(v) => v.parse().map_err(|_| anyhow!("PRINTSEARCH_BIND must be an IP address such as 127.0.0.1 or 0.0.0.0 (got {v:?})")),
    }
}

/// Address the server listens on (`PRINTSEARCH_BIND`, default 127.0.0.1).
pub fn bind_ip() -> Result<IpAddr> {
    parse_bind(std::env::var("PRINTSEARCH_BIND").ok().as_deref())
}

/// True when only this computer can reach the server.
pub fn is_loopback_only() -> bool {
    bind_ip().map_or(true, |ip| ip.is_loopback())
}

/// The specific (non-wildcard, non-loopback) address the server is bound to, if any.
pub fn specific_bind_ip() -> Option<IpAddr> {
    bind_ip().ok().filter(|ip| !ip.is_unspecified() && !ip.is_loopback())
}

fn url_for(ip: Option<IpAddr>) -> String {
    let host = match ip {
        Some(IpAddr::V6(v6)) => format!("[{v6}]"),
        Some(v4) => v4.to_string(),
        None => "localhost".into(),
    };
    format!("http://{host}:{PORT}")
}

/// Where this computer reaches the app (opening the browser, `quit`, `status`…).
pub fn url() -> String {
    url_for(specific_bind_ip())
}

/// Address used to check whether a PrintSearch is already running.
pub fn probe_addr() -> SocketAddr {
    let ip = match bind_ip() {
        Ok(ip) if ip.is_unspecified() => if ip.is_ipv6() { IpAddr::V6(std::net::Ipv6Addr::LOCALHOST) } else { Ipv4Addr::LOCALHOST.into() },
        Ok(ip) => ip,
        Err(_) => Ipv4Addr::LOCALHOST.into(),
    };
    SocketAddr::new(ip, PORT)
}

/// "http://Host.Example:8787/path" or "host.example:8787" -> "host.example" (IPv6 keeps its brackets).
pub fn normalize_host(raw: &str) -> String {
    let raw = raw.trim().to_ascii_lowercase();
    let raw = raw.strip_prefix("http://").or_else(|| raw.strip_prefix("https://")).unwrap_or(&raw);
    split_host_port(raw.split('/').next().unwrap_or("")).0
}

fn split_host_port(value: &str) -> (String, Option<u16>) {
    let v = value.trim();
    if let Some(rest) = v.strip_prefix('[') {
        if let Some((host, tail)) = rest.split_once(']') {
            return (format!("[{}]", host.to_ascii_lowercase()), tail.strip_prefix(':').and_then(|p| p.parse().ok()));
        }
    }
    match v.rsplit_once(':') {
        Some((h, p)) if !h.contains(':') => (h.to_ascii_lowercase(), p.parse().ok()),
        _ => (v.to_ascii_lowercase(), None),
    }
}

/// Host names accepted in the `Host` header: this computer, the bound address, and `PRINTSEARCH_ALLOWED_HOSTS`.
pub fn allowed_hosts() -> Vec<String> {
    let mut hosts: Vec<String> = ["localhost", "127.0.0.1", "[::1]"].map(String::from).to_vec();
    match specific_bind_ip() {
        Some(IpAddr::V6(v6)) => hosts.push(format!("[{v6}]")),
        Some(v4) => hosts.push(v4.to_string()),
        None => {}
    }
    if let Ok(extra) = std::env::var("PRINTSEARCH_ALLOWED_HOSTS") {
        for h in extra.split(',').map(normalize_host).filter(|h| !h.is_empty()) {
            if !hosts.contains(&h) {
                hosts.push(h);
            }
        }
    }
    hosts
}

/// Is this `Host` header value one of ours? (the port, when present, must be ours too)
pub fn host_ok(value: &str, hosts: &[String], port: u16) -> bool {
    let (host, p) = split_host_port(value);
    hosts.iter().any(|h| *h == host) && p.map_or(true, |p| p == port)
}

/// Is this `Origin` header value one of ours?
pub fn origin_ok(origin: &str, hosts: &[String], port: u16) -> bool {
    origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))
        .map_or(false, |rest| host_ok(rest.trim_end_matches('/'), hosts, port))
}

/// What a client on another computer may do when the server listens on the network: load the page and search.
pub fn remote_may(method: &Method, path: &str) -> bool {
    (method == Method::GET || method == Method::HEAD) && (path == "/" || path.starts_with("/icons/") || matches!(path, "/api/search" | "/api/status" | "/api/update"))
}

/// Does this peer count as "this computer"? (loopback, or the machine's own address when bound to a specific one)
pub fn is_local_peer(peer: IpAddr, bound: Option<IpAddr>) -> bool {
    peer.to_canonical().is_loopback() || bound.map_or(false, |b| b.to_canonical() == peer.to_canonical())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hosts(extra: &[&str]) -> Vec<String> {
        let mut v: Vec<String> = ["localhost", "127.0.0.1", "[::1]"].map(String::from).to_vec();
        v.extend(extra.iter().map(|s| s.to_string()));
        v
    }

    #[test]
    fn bind_defaults_to_loopback_and_validates() {
        assert_eq!(parse_bind(None).unwrap(), IpAddr::from(Ipv4Addr::LOCALHOST));
        assert_eq!(parse_bind(Some("  ")).unwrap(), IpAddr::from(Ipv4Addr::LOCALHOST));
        assert_eq!(parse_bind(Some("0.0.0.0")).unwrap(), IpAddr::from(Ipv4Addr::UNSPECIFIED));
        assert!(parse_bind(Some("::")).unwrap().is_unspecified());
        assert!(parse_bind(Some("my-server")).is_err());
        assert!(parse_bind(Some("300.1.1.1")).is_err());
    }

    #[test]
    fn urls_follow_the_bound_address() {
        assert_eq!(url_for(None), "http://localhost:8787");
        assert_eq!(url_for(Some("192.168.1.10".parse().unwrap())), "http://192.168.1.10:8787");
        assert_eq!(url_for(Some("fd00::5".parse().unwrap())), "http://[fd00::5]:8787");
    }

    #[test]
    fn host_header_is_checked_with_its_port() {
        let h = hosts(&["printsearch.lan"]);
        assert!(host_ok("localhost:8787", &h, 8787));
        assert!(host_ok("LOCALHOST:8787", &h, 8787));
        assert!(host_ok("127.0.0.1:8787", &h, 8787));
        assert!(host_ok("[::1]:8787", &h, 8787));
        assert!(host_ok("printsearch.lan:8787", &h, 8787));
        assert!(host_ok("printsearch.lan", &h, 8787)); // behind a proxy on the default port
        assert!(!host_ok("printsearch.lan:9999", &h, 8787));
        assert!(!host_ok("evil.example:8787", &h, 8787));
        assert!(!host_ok("evil.example", &h, 8787));
        assert!(!host_ok("", &h, 8787));
        assert!(!host_ok("localhost.evil.example:8787", &h, 8787));
    }

    #[test]
    fn origin_is_checked_like_host() {
        let h = hosts(&["printsearch.lan"]);
        assert!(origin_ok("http://localhost:8787", &h, 8787));
        assert!(origin_ok("http://printsearch.lan:8787", &h, 8787));
        assert!(origin_ok("https://printsearch.lan", &h, 8787));
        assert!(!origin_ok("http://evil.example", &h, 8787));
        assert!(!origin_ok("null", &h, 8787));
        assert!(!origin_ok("file://localhost", &h, 8787));
    }

    #[test]
    fn allowed_host_entries_are_normalized() {
        assert_eq!(normalize_host("  HTTP://Printsearch.LAN:8787/path "), "printsearch.lan");
        assert_eq!(normalize_host("192.168.1.10:8787"), "192.168.1.10");
        assert_eq!(normalize_host("[FD00::5]:8787"), "[fd00::5]");
        assert_eq!(normalize_host(""), "");
    }

    #[test]
    fn remote_clients_can_only_search() {
        for (m, p) in [(Method::GET, "/"), (Method::GET, "/icons/thangs.svg"), (Method::GET, "/api/search"), (Method::GET, "/api/status"), (Method::GET, "/api/update")] {
            assert!(remote_may(&m, p), "{m} {p} should be allowed");
        }
        for (m, p) in [
            (Method::POST, "/api/quit"),
            (Method::POST, "/api/command"),
            (Method::GET, "/api/logs"),
            (Method::GET, "/api/logs/export"),
            (Method::GET, "/api/browser"),
            (Method::POST, "/api/browser"),
            (Method::POST, "/api/login"),
            (Method::POST, "/api/logout"),
            (Method::POST, "/api/like"),
            (Method::POST, "/api/collect"),
            (Method::POST, "/api/lang"),
            (Method::GET, "/auth/thingiverse"),
            (Method::GET, "/callback"),
            (Method::POST, "/api/search"),
            (Method::GET, "/api/search/../quit"),
        ] {
            assert!(!remote_may(&m, p), "{m} {p} must be refused");
        }
    }

    #[test]
    fn local_peers() {
        let lan: IpAddr = "192.168.1.10".parse().unwrap();
        let other: IpAddr = "192.168.1.77".parse().unwrap();
        assert!(is_local_peer("127.0.0.1".parse().unwrap(), None));
        assert!(is_local_peer("::1".parse().unwrap(), None));
        assert!(is_local_peer("::ffff:127.0.0.1".parse().unwrap(), None));
        assert!(!is_local_peer(other, None));
        assert!(is_local_peer(lan, Some(lan))); // this machine reaching itself through its own LAN address
        assert!(!is_local_peer(other, Some(lan)));
    }
}
