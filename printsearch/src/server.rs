//! Local web interface. Listens on 127.0.0.1 only by default: tokens never leave your machine.
//! To reach it from another machine set `PRINTSEARCH_BIND` (address to listen on) and, if you use a
//! host name or a wildcard bind (0.0.0.0), `PRINTSEARCH_ALLOWED_HOSTS` (comma-separated names).
use crate::config::Config;
use crate::providers::{provider, thingiverse, Site};
use crate::search::{search_all, Filters, PriceFilter, Sort};
use axum::{
    extract::{Path, Query, Request, State},
    http::{header, StatusCode},
    middleware::{self, Next},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use anyhow::Context;
use serde_json::{json, Value};
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

const PORT: u16 = 8787;
const DEFAULT_BIND: &str = "127.0.0.1";

/// Address to listen on: `PRINTSEARCH_BIND`, or 127.0.0.1.
fn bind_addr() -> String {
    std::env::var("PRINTSEARCH_BIND")
        .ok()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_BIND.into())
}

/// True for loopback addresses (127.0.0.0/8, ::1) and `localhost`: not reachable from the network.
fn is_loopback(addr: &str) -> bool {
    addr.parse::<std::net::IpAddr>().map_or(addr == "localhost", |ip| ip.is_loopback())
}

fn is_wildcard(addr: &str) -> bool {
    addr == "0.0.0.0" || addr == "::"
}

/// `Host` / `Origin` values that are accepted: localhost, 127.0.0.1, the bind address (unless it is
/// a wildcard) and every name in `PRINTSEARCH_ALLOWED_HOSTS` (comma-separated), each with the port.
fn allowed_hosts() -> &'static Vec<String> {
    static HOSTS: OnceLock<Vec<String>> = OnceLock::new();
    HOSTS.get_or_init(|| {
        let mut hosts = vec!["localhost".to_string(), DEFAULT_BIND.to_string()];
        let bind = bind_addr();
        if !is_wildcard(&bind) {
            hosts.push(bind);
        }
        if let Ok(extra) = std::env::var("PRINTSEARCH_ALLOWED_HOSTS") {
            hosts.extend(extra.split(',').map(|h| h.trim().to_lowercase()).filter(|h| !h.is_empty()));
        }
        hosts
            .into_iter()
            .map(|h| if h.contains(':') && !h.starts_with('[') { format!("[{h}]:{PORT}") } else { format!("{h}:{PORT}") })
            .collect()
    })
}

/// Protection against third-party web pages (CSRF, "DNS rebinding"): only requests addressed
/// to an allowed host and, if they carry an origin, coming from an allowed host, are accepted.
async fn local_only(req: Request, next: Next) -> Response {
    let ok = allowed_hosts();
    let host_ok = req.headers().get(header::HOST).and_then(|h| h.to_str().ok()).map_or(false, |h| ok.iter().any(|a| a == h));
    let origin_ok = match req.headers().get(header::ORIGIN).and_then(|o| o.to_str().ok()) {
        None => true,
        Some(o) => o.strip_prefix("http://").map_or(false, |h| ok.iter().any(|a| a == h)),
    };
    if host_ok && origin_ok {
        next.run(req).await
    } else {
        (StatusCode::FORBIDDEN, "forbidden").into_response()
    }
}

pub async fn run(open_browser: bool) -> anyhow::Result<()> {
    let cfg = Config::load()?;
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
        .route("/api/like", post(like))
        .route("/api/collect", post(collect))
        .route("/auth/thingiverse", get(auth_thingiverse))
        .route("/callback", get(callback))
        .layer(middleware::from_fn(local_only))
        .with_state(state);
    let bind = bind_addr();
    let listener = tokio::net::TcpListener::bind((bind.as_str(), PORT))
        .await
        .with_context(|| format!("cannot listen on {bind}:{PORT} (is the address valid and assigned to this machine, and is the port free?)"))?;
    tokio::task::spawn_blocking(crate::browser::warm); // browser ready before the first search
    let bracketed = if bind.contains(':') { format!("[{bind}]") } else { bind.clone() }; // IPv6 literal
    let shown = if bind == DEFAULT_BIND || is_wildcard(&bind) { "localhost".to_string() } else { bracketed.clone() };
    let url = format!("http://{shown}:{PORT}");
    println!("Interface available at {url} (close this window or press Ctrl+C to stop)");
    if !is_loopback(&bind) {
        eprintln!("Warning: listening on {bracketed}:{PORT}. There is no authentication: anyone who can reach this address can use the interface and change the saved tokens.");
    }
    if open_browser {
        let _ = open::that(&url);
    }
    axum::serve(listener, app).await?;
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
    Ok(Json(json!({ "ok": true })))
}

async fn logout(State(cfg): State<Shared>, Json(b): Json<LoginBody>) -> ApiResult {
    let site = site_of(&b.site)?;
    let mut c = cfg.lock().await;
    c.tokens.remove(site.key());
    c.save().map_err(err)?;
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
    let t = token_of(&cfg, site).await?;
    provider(site).like(&b.id, &t).await.map_err(err)?;
    Ok(Json(json!({ "ok": true })))
}

async fn collect(State(cfg): State<Shared>, Json(b): Json<ActionBody>) -> ApiResult {
    let site = site_of(&b.site)?;
    let t = token_of(&cfg, site).await?;
    let col = b.collection.ok_or((StatusCode::BAD_REQUEST, "missing collection".into()))?;
    provider(site).add_to_collection(&col, &b.id, &t).await.map_err(err)?;
    Ok(Json(json!({ "ok": true })))
}

async fn auth_thingiverse() -> Result<Redirect, (StatusCode, String)> {
    Ok(Redirect::to(&thingiverse::authorize_url().map_err(err)?))
}

#[derive(Deserialize)]
struct CallbackParams {
    code: String,
}

async fn callback(State(cfg): State<Shared>, Query(p): Query<CallbackParams>) -> Result<Redirect, (StatusCode, String)> {
    let token = thingiverse::exchange_code(&p.code).await.map_err(err)?;
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
        _ => return Err(StatusCode::NOT_FOUND),
    };
    Ok(([(header::CONTENT_TYPE, "image/svg+xml")], svg))
}

fn browser_json(choice: Option<String>) -> Value {
    json!({
        "choice": choice.clone().unwrap_or_else(|| "auto".into()),
        "detected": crate::browser::detect(),
        "effective": crate::browser::effective(choice.as_deref()),
        "custom_allowed": is_loopback(&bind_addr()),
    })
}

async fn browser_info(State(cfg): State<Shared>) -> Json<Value> {
    let choice = cfg.lock().await.browser.clone();
    Json(browser_json(choice))
}

#[derive(Deserialize)]
struct BrowserBody {
    choice: String,
}

async fn browser_set(State(cfg): State<Shared>, Json(b): Json<BrowserBody>) -> ApiResult {
    let choice = b.choice.trim().to_string();
    // A custom path would let anyone who can reach a network listener pick a program to launch.
    let known = choice.is_empty() || choice == "auto" || crate::browser::detect().iter().any(|f| f.id == choice);
    if !is_loopback(&bind_addr()) && !known {
        return Err((
            StatusCode::FORBIDDEN,
            "custom browser paths are disabled while listening on a non-loopback address (PRINTSEARCH_BIND); pick a detected browser, or run `printsearch browser <path>` on the server".into(),
        ));
    }
    crate::browser::validate_choice(&choice).map_err(|e| (StatusCode::BAD_REQUEST, format!("{e:#}")))?;
    let stored = if choice.is_empty() || choice == "auto" { None } else { Some(choice) };
    {
        let mut c = cfg.lock().await;
        c.browser = stored.clone();
        c.save().map_err(err)?;
    }
    crate::browser::set_choice(stored.clone());
    tokio::task::spawn_blocking(crate::browser::warm);
    Ok(Json(browser_json(stored)))
}
