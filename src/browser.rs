//! Reads web pages rendered in JavaScript with a headless Chromium-based browser
//! (Chrome, Edge, Brave, Vivaldi, Chromium…). Used for sites without an accessible API (MakerWorld, Thangs)
//! and as a fallback for Thingiverse. The browser to use is chosen in Settings.
//!
//! Optimizations: a single browser reused between searches (started on first use, closed when idle),
//! images/media disabled, no wait for the "load" event, the page is read every
//! 250 ms with an early exit as soon as enough cards are there, and a 10 min cache.
use crate::log::{log_info, log_warn};
use anyhow::{anyhow, bail, Context, Result};
use headless_chrome::{Browser, LaunchOptions, Tab};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    ffi::OsStr,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Deserialize)]
pub struct Card {
    pub id: String,
    pub title: String,
    pub href: String,
    pub img: Option<String>,
}

#[derive(Deserialize, Default)]
struct Scan {
    #[serde(default)]
    cards: Vec<Card>,
    #[serde(default)]
    title: String,
    #[serde(default)]
    text: String,
}

/// A compatible browser found on this machine.
#[derive(Debug, Clone, Serialize)]
pub struct Found {
    pub id: String,
    pub name: String,
    pub path: String,
}

const UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36";
const CACHE_TTL: Duration = Duration::from_secs(600);
const CACHE_MAX: usize = 40;
const MAX_TABS: usize = 3;

type CacheMap = HashMap<String, (Instant, Vec<Card>, bool)>;
static BROWSER: Mutex<Option<Arc<Browser>>> = Mutex::new(None);
static CACHE: Mutex<Option<CacheMap>> = Mutex::new(None);
static CHOICE: Mutex<Option<String>> = Mutex::new(None);
/// Last launch failure (to avoid retrying in a loop for a few seconds).
static FAILED: Mutex<Option<(Instant, String)>> = Mutex::new(None);
/// Last use of the background browser: after 10 min of inactivity it is closed (it restarts by itself
/// on the next search). An application that stays for days in the tray must not keep a Chrome open.
static LAST_USE: Mutex<Option<Instant>> = Mutex::new(None);
static REAPER: std::sync::Once = std::sync::Once::new();
const IDLE_CLOSE: Duration = Duration::from_secs(600);
const LAUNCH_TIMEOUT: Duration = Duration::from_secs(35);
const RETRY_AFTER_FAILURE: Duration = Duration::from_secs(20);

// ---------------------------------------------------------------------------------------------
// Browser detection and choice
// ---------------------------------------------------------------------------------------------

fn first_in_path(names: &[&str]) -> Option<PathBuf> {
    let dirs = std::env::split_paths(&std::env::var_os("PATH")?).collect::<Vec<_>>();
    names.iter().find_map(|n| dirs.iter().map(|d| d.join(n)).find(|p| p.is_file()))
}

fn first_existing(paths: Vec<PathBuf>) -> Option<PathBuf> {
    paths.into_iter().find(|p| p.is_file())
}

/// Chromium-based browsers installed here, in the preference order of the automatic mode.
pub fn detect() -> Vec<Found> {
    let mut out = vec![];
    let mut add = |id: &str, name: &str, p: Option<PathBuf>| {
        if let Some(p) = p {
            out.push(Found { id: id.into(), name: name.into(), path: p.to_string_lossy().into_owned() });
        }
    };
    if cfg!(windows) {
        let roots: Vec<PathBuf> = ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"]
            .iter()
            .filter_map(|k| std::env::var_os(k).map(PathBuf::from))
            .collect();
        let find = |rel: &str| first_existing(roots.iter().map(|r| r.join(rel)).collect());
        add("chrome", "Google Chrome", find(r"Google\Chrome\Application\chrome.exe"));
        add("edge", "Microsoft Edge", find(r"Microsoft\Edge\Application\msedge.exe")); // preinstalled on Windows
        add("brave", "Brave", find(r"BraveSoftware\Brave-Browser\Application\brave.exe"));
        add("vivaldi", "Vivaldi", find(r"Vivaldi\Application\vivaldi.exe"));
        add("chromium", "Chromium", find(r"Chromium\Application\chrome.exe"));
    } else if cfg!(target_os = "macos") {
        let app = |a: &str, bin: &str| first_existing(vec![PathBuf::from(format!("/Applications/{a}.app/Contents/MacOS/{bin}"))]);
        add("chrome", "Google Chrome", app("Google Chrome", "Google Chrome"));
        add("edge", "Microsoft Edge", app("Microsoft Edge", "Microsoft Edge"));
        add("brave", "Brave", app("Brave Browser", "Brave Browser"));
        add("vivaldi", "Vivaldi", app("Vivaldi", "Vivaldi"));
        add("chromium", "Chromium", app("Chromium", "Chromium"));
    } else {
        add("chrome", "Google Chrome", first_in_path(&["google-chrome", "google-chrome-stable"]));
        add("edge", "Microsoft Edge", first_in_path(&["microsoft-edge", "microsoft-edge-stable"]));
        add("brave", "Brave", first_in_path(&["brave-browser", "brave", "brave-browser-stable"]));
        add("vivaldi", "Vivaldi", first_in_path(&["vivaldi", "vivaldi-stable"]));
        add("chromium", "Chromium", first_in_path(&["chromium", "chromium-browser"]));
    }
    out
}

/// Turns a choice ("auto", identifier, or path) into an actual browser.
fn resolve(choice: Option<&str>) -> Result<Option<Found>> {
    match choice {
        None | Some("") | Some("auto") => Ok(detect().into_iter().next()),
        Some(c) => {
            if let Some(f) = detect().into_iter().find(|f| f.id == c) {
                return Ok(Some(f));
            }
            let p = Path::new(c);
            if p.is_file() {
                let name = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "Custom".into());
                return Ok(Some(Found { id: "custom".into(), name, path: c.into() }));
            }
            bail!("Browser \"{c}\" not found")
        }
    }
}

/// Checks that a choice is usable (without applying it).
pub fn validate_choice(choice: &str) -> Result<()> {
    resolve(Some(choice)).map(|_| ())
}

/// Browser currently selected (None = none found).
pub fn effective(choice: Option<&str>) -> Option<Found> {
    resolve(choice).ok().flatten()
}

/// Changes the browser: the running instance is closed and the cache cleared.
pub fn set_choice(choice: Option<String>) {
    *CHOICE.lock().unwrap_or_else(|e| e.into_inner()) = choice;
    *BROWSER.lock().unwrap_or_else(|e| e.into_inner()) = None;
    *CACHE.lock().unwrap_or_else(|e| e.into_inner()) = None;
    *FAILED.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

// ---------------------------------------------------------------------------------------------
// Reading pages
// ---------------------------------------------------------------------------------------------

/// Extracts all cards having a link that matches `__RE__` (1st group = id).
const JS: &str = r#"(() => {
  const re = new RegExp(__RE__);
  const map = new Map();
  const line = t => (t || '').trim().split('\n').map(s => s.trim()).find(s => s) || '';
  document.querySelectorAll('a[href]').forEach(a => {
    const m = a.getAttribute('href').match(re);
    if (!m) return;
    const img = a.querySelector('img');
    let title = a.getAttribute('title') || a.getAttribute('aria-label') || (img && img.alt) || line(a.innerText);
    if (!title) {
      let p = a.parentElement;
      for (let i = 0; i < 3 && p && !title; i++, p = p.parentElement) {
        if ((p.innerText || '').length < 300) title = line(p.innerText);
      }
    }
    const cur = map.get(m[1]) || { id: m[1], title: '', href: a.href, img: null };
    if (!cur.title && title) cur.title = title;
    if (!cur.img && img) {
      const s = img.currentSrc || img.getAttribute('data-src') || img.src || '';
      if (s && !s.startsWith('data:')) cur.img = s;
    }
    map.set(m[1], cur);
  });
  const cards = [...map.values()].map(c => ({ ...c, title: c.title || ('#' + c.id) }));
  return JSON.stringify({
    cards,
    title: document.title,
    text: (document.body ? document.body.innerText : '').replace(/\s+/g, ' ').slice(0, 120)
  });
})()"#;

pub fn page_slice(cards: Vec<Card>, limit: usize, page: usize) -> Vec<Card> {
    cards.into_iter().skip((page.max(1) - 1) * limit).take(limit).collect()
}

/// Closes the background browser (call it before exiting: otherwise it would keep running).
pub fn shutdown() {
    if BROWSER.lock().unwrap_or_else(|e| e.into_inner()).take().is_some() {
        log_info!("background browser closed");
    }
}

fn touch() {
    *LAST_USE.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
}

/// Background thread that releases the unused browser. A running read keeps its own reference: it is not cut.
fn start_reaper() {
    REAPER.call_once(|| {
        let _ = std::thread::Builder::new().name("browser-reaper".into()).spawn(|| loop {
            std::thread::sleep(Duration::from_secs(30));
            let idle = LAST_USE.lock().unwrap_or_else(|e| e.into_inner()).map_or(false, |t| t.elapsed() > IDLE_CLOSE);
            if idle && BROWSER.lock().unwrap_or_else(|e| e.into_inner()).take().is_some() {
                log_info!("background browser closed after {} min without use", IDLE_CLOSE.as_secs() / 60);
            }
        });
    });
}

fn browser() -> Result<Arc<Browser>> {
    start_reaper();
    touch();
    let mut guard = BROWSER.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(b) = guard.as_ref() {
        if b.get_version().is_ok() {
            return Ok(b.clone());
        }
    }
    if let Some((t, msg)) = FAILED.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        if t.elapsed() < RETRY_AFTER_FAILURE {
            bail!("{msg}");
        }
    }
    let result = launch();
    match &result {
        Ok(b) => {
            *guard = Some(b.clone());
            *FAILED.lock().unwrap_or_else(|e| e.into_inner()) = None;
        }
        Err(e) => {
            log_warn!("background browser could not start: {e:#}");
            *FAILED.lock().unwrap_or_else(|e| e.into_inner()) = Some((Instant::now(), format!("{e:#}")));
        }
    }
    result
}

fn launch() -> Result<Arc<Browser>> {
    let choice = CHOICE.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let found = resolve(choice.as_deref())?;
    let started = Instant::now();
    log_info!("starting the background browser: {}", found.as_ref().map(|f| f.path.as_str()).unwrap_or("(automatic detection by the library)"));
    let opts = LaunchOptions::default_builder()
        .path(found.map(|f| PathBuf::from(f.path)))
        .headless(true)
        .window_size(Some((1366, 900)))
        .idle_browser_timeout(Duration::from_secs(3600))
        .args(vec![
            OsStr::new("--disable-blink-features=AutomationControlled"),
            OsStr::new("--blink-settings=imagesEnabled=false"),
            OsStr::new("--mute-audio"),
            OsStr::new("--disable-extensions"),
            OsStr::new("--disable-background-networking"),
            OsStr::new("--no-first-run"),
        ])
        .ignore_default_args(vec![OsStr::new("--enable-automation")])
        .build()
        .map_err(|e| anyhow!("{e}"))?;

    // The library reads the browser output in a blocking way: its internal timeout is not enough if the
    // launched program never answers (wrong path chosen, for example). The wait is therefore bounded here.
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(Browser::new(opts));
    });
    match rx.recv_timeout(LAUNCH_TIMEOUT) {
        Ok(r) => Ok(Arc::new(r.map(|b| { log_info!("background browser ready in {} ms", started.elapsed().as_millis()); b }).context(
            "No Chromium-based browser found or it could not start (Chrome, Edge, Brave, Vivaldi, Chromium): pick one in Settings",
        )?)),
        Err(_) => bail!(
            "The browser did not start within {} s. Is it really a Chromium-based browser? Pick another one in Settings",
            LAUNCH_TIMEOUT.as_secs()
        ),
    }
}

fn cache_get(url: &str, needed: usize) -> Option<Vec<Card>> {
    let g = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let (t, cards, exhausted) = g.as_ref()?.get(url)?;
    (t.elapsed() < CACHE_TTL && (cards.len() >= needed || *exhausted)).then(|| cards.clone())
}

fn cache_put(url: &str, cards: &[Card], exhausted: bool) {
    let mut g = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let map = g.get_or_insert_with(HashMap::new);
    map.retain(|_, (t, _, _)| t.elapsed() < CACHE_TTL);
    while map.len() >= CACHE_MAX {
        // the cache does not grow indefinitely: remove the oldest entry
        match map.iter().min_by_key(|(_, (t, _, _))| *t).map(|(k, _)| k.clone()) {
            Some(k) => map.remove(&k),
            None => break,
        };
    }
    map.insert(url.to_string(), (Instant::now(), cards.to_vec(), exhausted));
}

/// Loads `url`, scrolls the page until there are `needed` cards (or nothing new).
pub async fn scrape_cards(url: String, id_re: &'static str, needed: usize) -> Result<Vec<Card>> {
    // At most MAX_TABS simultaneous reads: several sites are read by the same browser, and opening
    // ten tabs at once would slow it down for everybody. The others wait for their turn.
    static TABS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(MAX_TABS);
    let job = async {
        let _permit = TABS.acquire().await?;
        tokio::task::spawn_blocking(move || scrape_blocking(&url, id_re, needed)).await?
    };
    // Global safeguard: a read (waiting included) can never block the search for more than 150 s
    match tokio::time::timeout(Duration::from_secs(150), job).await {
        Ok(r) => r,
        Err(_) => bail!("page reading timed out"),
    }
}

fn scrape_blocking(url: &str, id_re: &str, needed: usize) -> Result<Vec<Card>> {
    if let Some(c) = cache_get(url, needed) {
        return Ok(c);
    }
    let browser = browser()?;
    let tab = browser.new_tab()?;
    tab.set_default_timeout(Duration::from_secs(10)); // no call to the browser stays blocked for long
    let _ = tab.set_user_agent(UA, None, None);
    let js = JS.replace("__RE__", &serde_json::to_string(id_re)?);
    let result = poll(&tab, url, &js, needed);
    let _ = tab.close(true);
    touch();
    let (cards, exhausted) = result?;
    cache_put(url, &cards, exhausted);
    Ok(cards)
}

fn poll(tab: &Arc<Tab>, url: &str, js: &str, needed: usize) -> Result<(Vec<Card>, bool)> {
    tab.navigate_to(url)?; // no wait for "load": the page is read as it renders
    let start = Instant::now();
    let (mut cards, mut stagnant, mut diag) = (Vec::<Card>::new(), 0u32, String::new());
    let mut exhausted = false;

    while start.elapsed() < Duration::from_secs(35) {
        std::thread::sleep(Duration::from_millis(250));
        let scan: Scan = tab
            .evaluate(js, false)
            .ok()
            .and_then(|o| o.value)
            .and_then(|v| v.as_str().map(String::from))
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        if scan.cards.is_empty() {
            diag = format!("title \"{}\", start of the text: \"{}\"", scan.title, scan.text);
            continue;
        }
        stagnant = if scan.cards.len() > cards.len() { 0 } else { stagnant + 1 };
        cards = scan.cards;
        if cards.len() >= needed {
            break;
        }
        if stagnant >= 12 {
            exhausted = true; // ~3 s without a new card after scrolling: end of the results
            break;
        }
        let _ = tab.evaluate("window.scrollTo(0, document.body.scrollHeight)", false);
    }
    if cards.is_empty() {
        bail!("no result read after {} s — {}", start.elapsed().as_secs(), diag);
    }
    Ok((cards, exhausted))
}
