//! Local web interface. Listens on 127.0.0.1 only by default: tokens never leave your machine.
//! To reach it from another machine set `PRINTSEARCH_BIND` (address to listen on) and, for a host name or a
//! wildcard bind (0.0.0.0), `PRINTSEARCH_ALLOWED_HOSTS` (comma-separated names). Clients that are not on this
//! computer (decided from the TCP peer address) cannot do the things listed in `commands::remote_refusal`.
use crate::commands::{self, valid_id, Ctx, ONLY_HERE};
use crate::config::Config;
use crate::log::{self, log_info, log_warn};
use crate::providers::{provider, thingiverse, Site};
use crate::search::{search_all, Filters, PriceFilter, Sort};
use axum::{
    extract::{connect_info::Connected, ConnectInfo, DefaultBodyLimit, Path, Query, Request, State},
    http::{header, StatusCode},
    middleware::{self, Next},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;

type Shared = Arc<Mutex<Config>>;
type ApiResult = Result<Json<Value>, (StatusCode, String)>;

fn err(e: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::BAD_GATEWAY, format!("{e:#}"))
}
fn site_of(k: &str) -> Result<Site, (StatusCode, String)> {
    Site::from_key(k).ok_or((StatusCode::BAD_REQUEST, format!("unknown site: {k}")))
}

pub(crate) const PORT: u16 = 8787;
pub(crate) const DEFAULT_BIND: &str = "127.0.0.1";

/// Address to listen on: `PRINTSEARCH_BIND` (trimmed, lower-cased), or 127.0.0.1.
pub(crate) fn bind_addr() -> String {
    std::env::var("PRINTSEARCH_BIND")
        .ok()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_BIND.into())
}

/// 0.0.0.0, :: (and their longer spellings): "every interface".
pub(crate) fn is_wildcard(addr: &str) -> bool {
    addr.parse::<IpAddr>().map_or(false, |ip| ip.is_unspecified())
}

/// IPv6 text the way a browser writes it in a URL (compressed; an IPv4-mapped address in hexadecimal).
fn v6_text(ip: Ipv6Addr) -> String {
    let s = ip.segments();
    if s[..5] == [0; 5] && s[5] == 0xffff {
        format!("::ffff:{:x}:{:x}", s[6], s[7])
    } else {
        ip.to_string()
    }
}

/// Canonical `host:port` of a Host header or an Origin authority (lower-case host, bracketed compressed IPv6).
/// None if it is malformed. Used for the allow-list AND for what we print, so both always agree.
pub(crate) fn canon_authority(a: &str) -> Option<String> {
    let (host, port) = if let Some(rest) = a.strip_prefix('[') {
        let (inner, tail) = rest.split_once(']')?;
        let ip: Ipv6Addr = inner.parse().ok()?;
        (format!("[{}]", v6_text(ip)), tail.strip_prefix(':')?)
    } else {
        let (h, p) = a.rsplit_once(':')?;
        if h.is_empty() || h.contains(':') {
            return None;
        }
        (h.to_ascii_lowercase(), p)
    };
    if port.is_empty() || !port.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(format!("{host}:{port}"))
}

/// An allowed host name or address (without port) as the canonical `host:PORT`.
fn host_entry(h: &str) -> Option<String> {
    let h = h.trim();
    if h.is_empty() {
        return None;
    }
    let bare = h.trim_start_matches('[').trim_end_matches(']');
    if bare.parse::<Ipv6Addr>().is_ok() {
        return canon_authority(&format!("[{bare}]:{PORT}"));
    }
    canon_authority(&format!("{h}:{PORT}"))
}

fn allowed_from(bind: &str, extra: Option<&str>) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    let mut add = |h: &str| match host_entry(h) {
        Some(a) => {
            if !v.contains(&a) {
                v.push(a);
            }
        }
        None => log_warn!("ignoring an invalid allowed host: {h:?}"),
    };
    add("localhost");
    add(DEFAULT_BIND);
    if !is_wildcard(bind) {
        add(bind);
    }
    for h in extra.unwrap_or("").split(',') {
        if !h.trim().is_empty() {
            add(h);
        }
    }
    v
}

/// Host / Origin values that are accepted: localhost, 127.0.0.1, the bind address (unless it is a wildcard)
/// and every name in `PRINTSEARCH_ALLOWED_HOSTS`, each with the port. Fixed for the life of the process.
fn allowed_hosts() -> &'static Vec<String> {
    static HOSTS: OnceLock<Vec<String>> = OnceLock::new();
    HOSTS.get_or_init(|| allowed_from(&bind_addr(), std::env::var("PRINTSEARCH_ALLOWED_HOSTS").ok().as_deref()))
}

/// The host part of the address we advertise (printed, opened, shown by `status`): always accepted by the allow-list.
pub(crate) fn display_host(bind: &str) -> String {
    if bind == DEFAULT_BIND || is_wildcard(bind) {
        return "localhost".into();
    }
    match bind.parse::<IpAddr>() {
        Ok(IpAddr::V6(ip)) => format!("[{}]", v6_text(ip)),
        Ok(IpAddr::V4(ip)) => ip.to_string(),
        Err(_) => bind.to_string(),
    }
}

/// The two addresses of one accepted TCP connection.
#[derive(Clone, Copy, Debug)]
pub struct Peer {
    remote: SocketAddr,
    local: Option<SocketAddr>,
}

impl Connected<axum::serve::IncomingStream<'_>> for Peer {
    fn connect_info(stream: axum::serve::IncomingStream<'_>) -> Self {
        Peer { remote: stream.remote_addr(), local: stream.local_addr().ok() }
    }
}

/// True when the client is on this computer: a loopback source, or a source equal to the address it connected to
/// (the CLI of the server reaching a LAN-address bind). It cannot be forged by a header, and without the local
/// address only loopback counts (fails closed). Every local user, an SSH tunnel and a proxy on this host qualify:
/// do not put an untrusted proxy in front of a network bind.
pub(crate) fn host_local(p: &Peer) -> bool {
    let r = p.remote.ip().to_canonical();
    r.is_loopback() || p.local.map_or(false, |l| l.ip().to_canonical() == r)
}

/// Protection against third-party web pages (CSRF, "DNS rebinding"): only requests addressed to an allowed host
/// and, if they carry an origin, coming from an allowed host are accepted. This is not authentication.
async fn local_only(req: Request, next: Next) -> Response {
    let ok = allowed_hosts();
    let host_ok = req.headers().get(header::HOST).and_then(|h| h.to_str().ok()).and_then(canon_authority).map_or(false, |h| ok.contains(&h));
    let origin_ok = match req.headers().get(header::ORIGIN).and_then(|o| o.to_str().ok()) {
        None => true,
        Some(o) => o.strip_prefix("http://").and_then(canon_authority).map_or(false, |h| ok.contains(&h)),
    };
    if host_ok && origin_ok {
        next.run(req).await
    } else {
        (StatusCode::FORBIDDEN, "forbidden").into_response()
    }
}

pub async fn run(listener: std::net::TcpListener, open_browser: bool) -> anyhow::Result<()> {
    let cfg = Config::load();
    crate::browser::set_choice(cfg.browser.clone());
    let state: Shared = Arc::new(Mutex::new(cfg));
    let app = Router::new()
        .route("/", get(|| async { Html(include_str!("../web/index.html")) }))
        .route("/icons/:name", get(icon))
        .route("/api/search", get(search))
        .route("/api/status", get(status))
        .route("/api/login", post(login))
        .route("/api/logout", post(logout))
        .route("/api/browser", get(browser_info).post(browser_set))
        .route("/api/update", get(update_info))
        .route("/api/like", post(like))
        .route("/api/collect", post(collect))
        .route("/api/command", post(command))
        .route("/api/logs", get(logs_text))
        .route("/api/logs/export", get(logs_export))
        .route("/api/quit", post(quit))
        .route("/api/lang", post(set_lang))
        .route("/auth/thingiverse", get(auth_thingiverse))
        .route("/callback", get(callback))
        .layer(DefaultBodyLimit::max(64 * 1024)) // no request of the interface needs more
        .layer(middleware::from_fn(local_only))
        .with_state(state);
    listener.set_nonblocking(true)?;
    let listener = tokio::net::TcpListener::from_std(listener)?;
    let local = listener.local_addr()?;
    let url = crate::app::url();
    log_info!("web interface ready on {url}");
    println!("Web interface: {url} (Ctrl+C to stop)");
    if !local.ip().to_canonical().is_loopback() {
        let bind = bind_addr();
        let shown = if bind.contains(':') { format!("[{bind}]") } else { bind };
        let msg = format!("listening on {shown}:{PORT}. There is no authentication: anyone who can reach this address can use the interface and change the saved tokens.");
        eprintln!("Warning: {msg}");
        log_warn!("{msg}");
    }
    if open_browser {
        let _ = open::that(&url);
    }
    axum::serve(listener, app.into_make_service_with_connect_info::<Peer>()).with_graceful_shutdown(crate::app::wait_quit()).await?;
    Ok(())
}

#[derive(Deserialize)]
struct SearchParams {
    q: String,
    sites: Option<String>,
    limit: Option<usize>,
    page: Option<usize>,
    price: Option<PriceFilter>,
    sort: Option<Sort>,
    min_likes: Option<u64>,
    min_downloads: Option<u64>,
}

async fn search(State(cfg): State<Shared>, Query(p): Query<SearchParams>) -> Json<Value> {
    let sites: Vec<Site> = match &p.sites {
        Some(s) if !s.is_empty() => s.split(',').filter_map(Site::from_key).collect(),
        _ => Site::ALL.to_vec(),
    };
    let filters = Filters {
        price: p.price.unwrap_or_default(),
        sort: p.sort.unwrap_or_default(),
        min_likes: p.min_likes.unwrap_or(0),
        min_downloads: p.min_downloads.unwrap_or(0),
    };
    let snapshot = cfg.lock().await.clone();
    let out = search_all(&snapshot, &sites, &p.q, p.limit.unwrap_or(12).clamp(1, 100), p.page.unwrap_or(1).max(1), &filters).await;
    Json(json!({ "results": out.results, "errors": out.errors, "raw": out.raw }))
}

async fn status(State(cfg): State<Shared>) -> Json<Value> {
    let c = cfg.lock().await;
    let m: serde_json::Map<String, Value> = Site::ALL
        .iter()
        .map(|s| (s.key().to_string(), json!(c.token_for(*s).is_some())))
        .collect();
    let mut m = m;
    let oauth = std::env::var("THINGIVERSE_CLIENT_ID").is_ok() && std::env::var("THINGIVERSE_CLIENT_SECRET").is_ok();
    m.insert("thingiverse_oauth".into(), json!(oauth));
    m.insert("version".into(), json!(crate::update::current()));
    Json(Value::Object(m))
}

#[derive(Deserialize)]
struct LoginBody {
    site: String,
    token: Option<String>,
}

async fn login(State(cfg): State<Shared>, Json(b): Json<LoginBody>) -> ApiResult {
    let site = site_of(&b.site)?;
    let token = b.token.filter(|t| !t.trim().is_empty()).ok_or((StatusCode::BAD_REQUEST, "empty token".into()))?;
    let mut c = cfg.lock().await;
    c.tokens.insert(site.key().into(), token.trim().to_string());
    c.save().map_err(err)?;
    log_info!("token saved for {}", site.key());
    Ok(Json(json!({ "ok": true })))
}

async fn logout(State(cfg): State<Shared>, Json(b): Json<LoginBody>) -> ApiResult {
    let site = site_of(&b.site)?;
    let mut c = cfg.lock().await;
    c.tokens.remove(site.key());
    c.save().map_err(err)?;
    log_info!("token removed for {}", site.key());
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct ActionBody {
    site: String,
    id: String,
    collection: Option<String>,
}

async fn token_of(cfg: &Shared, site: Site) -> Result<String, (StatusCode, String)> {
    cfg.lock().await.token_for(site).ok_or((StatusCode::UNAUTHORIZED, "not connected to this site".into()))
}

async fn like(State(cfg): State<Shared>, Json(b): Json<ActionBody>) -> ApiResult {
    let site = site_of(&b.site)?;
    if !site.can_like() {
        return Err((StatusCode::BAD_REQUEST, format!("Like is not available for {}", site.key())));
    }
    if !valid_id(&b.id) {
        return Err((StatusCode::BAD_REQUEST, "invalid model id".into()));
    }
    let t = token_of(&cfg, site).await?;
    match provider(site).like(&b.id, &t).await {
        Ok(()) => log_info!("liked {} {}", site.key(), b.id),
        Err(e) => {
            log_warn!("like failed on {} {}: {e:#}", site.key(), b.id);
            return Err(err(e));
        }
    }
    Ok(Json(json!({ "ok": true })))
}

async fn collect(State(cfg): State<Shared>, Json(b): Json<ActionBody>) -> ApiResult {
    let site = site_of(&b.site)?;
    if !site.can_collect() {
        return Err((StatusCode::BAD_REQUEST, format!("Collections are not available for {}", site.key())));
    }
    let t = token_of(&cfg, site).await?;
    let col = b.collection.ok_or((StatusCode::BAD_REQUEST, "missing collection".into()))?;
    if !valid_id(&col) || !valid_id(&b.id) {
        return Err((StatusCode::BAD_REQUEST, "invalid id".into()));
    }
    match provider(site).add_to_collection(&col, &b.id, &t).await {
        Ok(()) => log_info!("added {} {} to collection {col}", site.key(), b.id),
        Err(e) => {
            log_warn!("add to collection failed on {} {}: {e:#}", site.key(), b.id);
            return Err(err(e));
        }
    }
    Ok(Json(json!({ "ok": true })))
}

/// Moment an OAuth login was started from the interface: a third-party page cannot
/// slip its own authorization code into /callback unless the user clicked "Connect".
static OAUTH_PENDING: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);

/// The OAuth redirect is fixed to localhost:8787: the sign-in only works in a browser on this computer.
fn oauth_local_only(peer: &Peer) -> Result<(), (StatusCode, String)> {
    if host_local(peer) {
        return Ok(());
    }
    log_warn!("Thingiverse sign-in refused for a network client");
    Err((StatusCode::FORBIDDEN, "Thingiverse sign-in works only in a browser on the PrintSearch computer (or through an SSH tunnel); paste an App Token instead".into()))
}

async fn auth_thingiverse(ConnectInfo(peer): ConnectInfo<Peer>) -> Result<Redirect, (StatusCode, String)> {
    oauth_local_only(&peer)?;
    let url = thingiverse::authorize_url().map_err(err)?;
    *OAUTH_PENDING.lock().unwrap_or_else(|e| e.into_inner()) = Some(std::time::Instant::now());
    Ok(Redirect::to(&url))
}

#[derive(Deserialize)]
struct CallbackParams {
    code: String,
}

async fn callback(State(cfg): State<Shared>, ConnectInfo(peer): ConnectInfo<Peer>, Query(p): Query<CallbackParams>) -> Result<Redirect, (StatusCode, String)> {
    oauth_local_only(&peer)?;
    let pending = OAUTH_PENDING.lock().unwrap_or_else(|e| e.into_inner()).take();
    if !pending.map_or(false, |t| t.elapsed() < std::time::Duration::from_secs(600)) {
        log_warn!("OAuth callback refused: no login in progress");
        return Err((StatusCode::FORBIDDEN, "no Thingiverse login in progress".into()));
    }
    let token = thingiverse::exchange_code(&p.code).await.map_err(|e| {
        log_warn!("Thingiverse OAuth failed: {e:#}");
        err(e)
    })?;
    log_info!("Thingiverse connected through OAuth");
    let mut c = cfg.lock().await;
    c.tokens.insert("thingiverse".into(), token);
    c.save().map_err(err)?;
    Ok(Redirect::to("/#parametres"))
}

/// Site icons, embedded in the binary (replaceable: see web/icons/).
async fn icon(Path(name): Path<String>) -> Result<([(header::HeaderName, &'static str); 1], &'static str), StatusCode> {
    let svg = match name.as_str() {
        "thingiverse.svg" => include_str!("../web/icons/thingiverse.svg"),
        "printables.svg" => include_str!("../web/icons/printables.svg"),
        "makerworld.svg" => include_str!("../web/icons/makerworld.svg"),
        "thangs.svg" => include_str!("../web/icons/thangs.svg"),
        "cults3d.svg" => include_str!("../web/icons/cults3d.svg"),
        "myminifactory.svg" => include_str!("../web/icons/myminifactory.svg"),
        _ => return Err(StatusCode::NOT_FOUND),
    };
    Ok(([(header::CONTENT_TYPE, "image/svg+xml")], svg))
}

fn browser_json(choice: Option<String>, custom_allowed: bool) -> Value {
    json!({
        "choice": choice.clone().unwrap_or_else(|| "auto".into()),
        "detected": crate::browser::detect(),
        "effective": crate::browser::effective(choice.as_deref()),
        "custom_allowed": custom_allowed,
    })
}

async fn browser_info(State(cfg): State<Shared>, ConnectInfo(peer): ConnectInfo<Peer>) -> Json<Value> {
    let choice = cfg.lock().await.browser.clone();
    Json(browser_json(choice, host_local(&peer)))
}

#[derive(Deserialize)]
struct BrowserBody {
    choice: String,
}

async fn browser_set(State(cfg): State<Shared>, ConnectInfo(peer): ConnectInfo<Peer>, Json(b): Json<BrowserBody>) -> ApiResult {
    let choice = b.choice.trim().to_string();
    let local = host_local(&peer);
    if !local && !crate::browser::is_known_choice(&choice) {
        log_warn!("custom browser path refused for a network client");
        return Err((StatusCode::FORBIDDEN, format!("choosing a custom browser path {ONLY_HERE}")));
    }
    crate::browser::validate_choice(&choice).map_err(|e| (StatusCode::BAD_REQUEST, format!("{e:#}")))?;
    let stored = if choice.is_empty() || choice == "auto" { None } else { Some(choice) };
    {
        let mut c = cfg.lock().await;
        c.browser = stored.clone();
        c.save().map_err(err)?;
    }
    log_info!("search browser set to {}", stored.as_deref().unwrap_or("auto"));
    crate::browser::set_choice(stored.clone());
    Ok(Json(browser_json(stored, local)))
}

#[derive(Deserialize)]
struct UpdateParams {
    force: Option<u8>,
}

async fn update_info(Query(p): Query<UpdateParams>) -> ApiResult {
    let info = crate::update::check(p.force == Some(1)).await.map_err(err)?;
    Ok(Json(json!(info)))
}

// ---------------------------------------------------------------------------------------------
// Console, log, shutdown, language
// ---------------------------------------------------------------------------------------------

#[derive(Deserialize)]
struct CommandBody {
    line: String,
}

async fn command(State(cfg): State<Shared>, ConnectInfo(peer): ConnectInfo<Peer>, Json(b): Json<CommandBody>) -> ApiResult {
    if b.line.len() > 2000 {
        return Err((StatusCode::BAD_REQUEST, "command too long".into()));
    }
    let output = commands::run_line(&Ctx { cfg, in_app: true, host_local: host_local(&peer) }, &b.line).await;
    Ok(Json(json!({ "output": output })))
}

#[derive(Deserialize)]
struct LogParams {
    n: Option<usize>,
    level: Option<log::Level>,
}

async fn logs_text(Query(p): Query<LogParams>) -> impl IntoResponse {
    let lines = log::tail(p.n.unwrap_or(200).min(5000), p.level.unwrap_or(log::Level::Info));
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], lines.join("\n"))
}

async fn logs_export(State(cfg): State<Shared>, ConnectInfo(peer): ConnectInfo<Peer>) -> impl IntoResponse {
    let body = log::export_text(&commands::system_info(&Ctx { cfg, in_app: true, host_local: host_local(&peer) }).await);
    (
        [
            (header::CONTENT_TYPE, "text/plain; charset=utf-8".to_string()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"printsearch-logs-{}.txt\"", log::stamp_for_filename())),
        ],
        body,
    )
}

async fn quit(ConnectInfo(peer): ConnectInfo<Peer>) -> ApiResult {
    if !host_local(&peer) {
        log_warn!("quit refused for a network client");
        return Err((StatusCode::FORBIDDEN, format!("quitting PrintSearch {ONLY_HERE}")));
    }
    crate::app::request_quit();
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct LangBody {
    lang: String,
}

/// Remembers the interface language (for the tray icon menu at the next start).
async fn set_lang(State(cfg): State<Shared>, Json(b): Json<LangBody>) -> ApiResult {
    if !matches!(b.lang.as_str(), "en" | "de" | "fr") {
        return Err((StatusCode::BAD_REQUEST, "unsupported language".into()));
    }
    let mut c = cfg.lock().await;
    if c.lang.as_deref() != Some(b.lang.as_str()) {
        c.lang = Some(b.lang);
        c.save().map_err(err)?;
    }
    Ok(Json(json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer(remote: &str, local: Option<&str>) -> Peer {
        Peer { remote: remote.parse().unwrap(), local: local.map(|l| l.parse().unwrap()) }
    }

    #[test]
    fn authorities_are_canonical_like_a_browser_sends_them() {
        assert_eq!(canon_authority("LocalHost:8787").as_deref(), Some("localhost:8787"));
        assert_eq!(canon_authority("[0:0:0:0:0:0:0:1]:8787").as_deref(), Some("[::1]:8787"));
        assert_eq!(canon_authority("[::FFFF:127.0.0.1]:8787").as_deref(), Some("[::ffff:7f00:1]:8787"));
        assert_eq!(canon_authority("192.168.1.10:8787").as_deref(), Some("192.168.1.10:8787"));
        for bad in ["", "localhost", ":8787", "a:b", "[::1]", "[::1]8787", "[nope]:8787", "a:b:8787", "host:"] {
            assert_eq!(canon_authority(bad), None, "{bad}");
        }
    }

    #[test]
    fn allow_list_defaults_to_loopback_only() {
        assert_eq!(allowed_from("127.0.0.1", None), ["localhost:8787", "127.0.0.1:8787"]);
        assert_eq!(allowed_from("0.0.0.0", None), ["localhost:8787", "127.0.0.1:8787"]);
        assert_eq!(allowed_from("::", None), ["localhost:8787", "127.0.0.1:8787"]);
    }

    #[test]
    fn allow_list_takes_the_bind_address_and_extra_names() {
        let v = allowed_from("192.0.2.10", Some(" MyHost, ,[::1], 0:0:0:0:0:0:0:1 ,bad:1"));
        assert_eq!(v, ["localhost:8787", "127.0.0.1:8787", "192.0.2.10:8787", "myhost:8787", "[::1]:8787"]);
        assert!(allowed_from("ps-srv", None).contains(&"ps-srv:8787".to_string()));
        assert!(allowed_from("::1", None).contains(&"[::1]:8787".to_string()));
    }

    #[test]
    fn advertised_host_is_always_allowed() {
        for bind in ["127.0.0.1", "0.0.0.0", "::", "::1", "192.0.2.10", "ps-srv", "0:0:0:0:0:0:0:1", "::ffff:192.0.2.10"] {
            let shown = display_host(bind);
            let ok = allowed_from(bind, None);
            assert!(ok.contains(&canon_authority(&format!("{shown}:{PORT}")).unwrap()), "{bind} -> {shown}");
        }
        assert_eq!(display_host("::1"), "[::1]");
        assert_eq!(display_host("0.0.0.0"), "localhost");
        assert_eq!(display_host("192.0.2.10"), "192.0.2.10");
    }

    #[test]
    fn host_local_rule() {
        assert!(host_local(&peer("127.0.0.1:5000", Some("127.0.0.1:8787"))));
        assert!(host_local(&peer("[::1]:5000", Some("[::1]:8787"))));
        assert!(host_local(&peer("127.0.0.1:5000", None)), "loopback needs no local address");
        assert!(host_local(&peer("192.0.2.10:5000", Some("192.0.2.10:8787"))), "own address");
        assert!(host_local(&peer("[::ffff:127.0.0.1]:5000", Some("[::ffff:127.0.0.1]:8787"))), "mapped loopback");
        assert!(host_local(&peer("[::ffff:192.0.2.10]:5000", Some("192.0.2.10:8787"))), "mapped own address");
        assert!(!host_local(&peer("192.0.2.77:5000", Some("192.0.2.10:8787"))), "another machine");
        assert!(!host_local(&peer("192.0.2.77:5000", None)), "fails closed without the local address");
        assert!(!host_local(&peer("192.0.2.10:5000", None)), "fails closed without the local address");
    }
}
