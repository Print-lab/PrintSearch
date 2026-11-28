//! Application commands. The same commands are used:
//!  - in a terminal: `printsearch search "vase" --price free`
//!  - in the console built into the web interface ("Console" page).
use crate::config::Config;
use crate::log::{self, log_info, Level};
use crate::providers::{provider, Site};
use crate::search::{search_all, Filters, PriceFilter, Sort};
use crate::{browser, update};
use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use std::{fmt::Write as _, path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::Mutex;

#[derive(Parser)]
#[command(name = "printsearch", version, about = "Unified search for 3D models: Thingiverse, Printables, MakerWorld, Thangs, Cults and MyMiniFactory")]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Option<Cmd>,
}

#[derive(Subcommand)]
pub enum Cmd {
    /// Search one or several sites
    Search {
        query: String,
        #[arg(short, long, value_enum, num_args = 1.., value_delimiter = ',')]
        sites: Vec<Site>,
        /// Results requested per site and per page
        #[arg(short, long, default_value_t = 10)]
        limit: usize,
        /// Results page (starts at 1)
        #[arg(long, default_value_t = 1)]
        page: usize,
        #[arg(long, value_enum, default_value_t = PriceFilter::All)]
        price: PriceFilter,
        #[arg(long, value_enum, default_value_t = Sort::Relevance)]
        sort: Sort,
        #[arg(long, default_value_t = 0)]
        min_likes: u64,
        #[arg(long, default_value_t = 0)]
        min_downloads: u64,
    },
    /// Connect to a site (Thingiverse: OAuth or --token; others: --token copied from the browser)
    Login {
        #[arg(value_enum)]
        site: Site,
        #[arg(long)]
        token: Option<String>,
    },
    /// Start the application (default command). Launched from a terminal, it detaches and runs in the background with a tray icon
    Serve {
        /// Open the web interface in your browser
        #[arg(long)]
        open: bool,
        /// Do not show the tray icon (keeps running in this terminal; Ctrl+C to stop)
        #[arg(long)]
        no_tray: bool,
        /// Stay attached to this terminal instead of detaching into the background (Ctrl+C to stop)
        #[arg(long)]
        foreground: bool,
    },
    /// Check GitHub for a newer version
    Update,
    /// List compatible browsers, or choose one: `browser auto|chrome|edge|brave|vivaldi|chromium|<path>`
    Browser { choice: Option<String> },
    /// Like a model
    Like {
        #[arg(value_enum)]
        site: Site,
        id: String,
    },
    /// Add a model to a collection
    Collect {
        #[arg(value_enum)]
        site: Site,
        collection: String,
        id: String,
    },
    /// Show the application log, or export / clear it: `logs`, `logs -n 200 --level warn`, `logs export [path]`, `logs path`, `logs clear`
    Logs {
        #[command(subcommand)]
        action: Option<LogsAction>,
        /// Number of lines to show
        #[arg(short = 'n', long, default_value_t = 50)]
        lines: usize,
        /// Minimum level to show
        #[arg(long, value_enum, default_value_t = Level::Info)]
        level: Level,
    },
    /// Show the version, browser, connections and log location
    Status,
    /// Close the running PrintSearch
    Quit,
}

#[derive(Subcommand)]
pub enum LogsAction {
    /// Write the whole log, with a diagnostic header, to a file (default: your Downloads folder)
    Export { path: Option<String> },
    /// Print the location of the log file
    Path,
    /// Delete the log
    Clear,
}

/// Execution context: the shared configuration and where it runs.
#[derive(Clone)]
pub struct Ctx {
    pub cfg: Arc<Mutex<Config>>,
    /// true = executed inside the running application (built-in console); false = terminal
    pub in_app: bool,
}

fn price_text(p: Option<f64>) -> String {
    match p {
        Some(p) if p > 0.0 => format!("{p}"),
        Some(_) => "free".into(),
        None => "?".into(),
    }
}

pub async fn execute(cmd: Cmd, ctx: &Ctx, out: &mut String) -> Result<()> {
    match cmd {
        Cmd::Search { query, sites, limit, page, price, sort, min_likes, min_downloads } => {
            let sites = if sites.is_empty() { Site::ALL.to_vec() } else { sites };
            let filters = Filters { price, sort, min_likes, min_downloads };
            let snapshot = ctx.cfg.lock().await.clone();
            let res = search_all(&snapshot, &sites, &query, limit, page.max(1), &filters).await;
            for s in &res.skipped {
                writeln!(out, "[{}] skipped: needs an API key (`login {} --token ...`, see the README)", s.key(), s.key())?;
            }
            for (s, e) in &res.errors {
                writeln!(out, "[{s}] error: {e}")?;
            }
            for m in &res.results {
                writeln!(
                    out,
                    "[{:<11}] {} - {}  ♥{} ⬇{}  price: {}\n              {}  (id: {})",
                    m.site.key(),
                    m.title,
                    m.author,
                    m.likes.map_or("?".into(), |x| x.to_string()),
                    m.downloads.map_or("?".into(), |x| x.to_string()),
                    price_text(m.price),
                    m.url,
                    m.id
                )?;
            }
            writeln!(out, "{} result(s)", res.results.len())?;
        }
        Cmd::Login { site, token } => {
            if !site.supports_account() {
                bail!("{} is searched without an account: there is nothing to connect", site.key());
            }
            let t = match (site, token) {
                (Site::Cults3d, Some(t)) if crate::providers::cults::split_credentials(&t).is_none() => {
                    bail!("for Cults, use --token <username>:<api_key>")
                }
                (_, Some(t)) if !t.trim().is_empty() => t.trim().to_string(),
                (Site::Thingiverse, None) if !ctx.in_app => crate::providers::thingiverse::oauth_login().await?,
                (Site::Thingiverse, None) => bail!("the OAuth login needs your browser: use Settings → Thingiverse, or `login thingiverse --token <APP_TOKEN>`"),
                _ => bail!("this site needs --token <TOKEN> (see the README)"),
            };
            {
                let mut c = ctx.cfg.lock().await;
                c.tokens.insert(site.key().into(), t);
                c.save()?;
            }
            log_info!("connected to {}", site.key());
            writeln!(out, "Connected to {}.", site.key())?;
        }
        Cmd::Serve { .. } => {
            writeln!(out, "PrintSearch is already running ({}).", crate::net::url())?;
        }
        Cmd::Update => {
            let u = update::check(true).await?;
            writeln!(out, "Current version: {}", u.current)?;
            match &u.latest {
                None => writeln!(out, "No release published on GitHub yet ({}).", u.url)?,
                Some(v) if u.update_available => {
                    writeln!(out, "New version available: {v}\n{}", u.url)?;
                    if let Some(n) = &u.notes {
                        writeln!(out, "\n{n}")?;
                    }
                }
                Some(v) => writeln!(out, "You are up to date (latest published: {v}).")?,
            }
        }
        Cmd::Browser { choice } => match choice {
            None => {
                let current = ctx.cfg.lock().await.browser.clone();
                writeln!(out, "Selected: {}", current.as_deref().unwrap_or("auto"))?;
                for f in browser::detect() {
                    writeln!(out, "  {:<9} {}  ({})", f.id, f.name, f.path)?;
                }
                match browser::effective(current.as_deref()) {
                    Some(f) => writeln!(out, "In use: {} ({})", f.name, f.path)?,
                    None => writeln!(out, "No compatible browser found (Chrome, Edge, Brave, Vivaldi, Chromium).")?,
                }
            }
            Some(c) => {
                browser::validate_choice(&c)?;
                let stored = if c == "auto" { None } else { Some(c.clone()) };
                {
                    let mut cfg = ctx.cfg.lock().await;
                    cfg.browser = stored.clone();
                    cfg.save()?;
                }
                browser::set_choice(stored);
                log_info!("search browser set to {c}");
                writeln!(out, "Browser set to {c}.")?;
                if !ctx.in_app {
                    writeln!(out, "(A PrintSearch that is already running applies it after a restart, or from Settings.)")?;
                }
            }
        },
        Cmd::Like { site, id } => {
            if !site.can_like() {
                bail!("Like is not available for {}", site.key());
            }
            let t = ctx.cfg.lock().await.token_for(site).context("not connected: run `login` first")?;
            provider(site).like(&id, &t).await?;
            log_info!("liked {} {id}", site.key());
            writeln!(out, "Model {id} liked on {}.", site.key())?;
        }
        Cmd::Collect { site, collection, id } => {
            if !site.can_collect() {
                bail!("Collections are not available for {}", site.key());
            }
            let t = ctx.cfg.lock().await.token_for(site).context("not connected: run `login` first")?;
            provider(site).add_to_collection(&collection, &id, &t).await?;
            log_info!("added {} {id} to collection {collection}", site.key());
            writeln!(out, "Model {id} added to collection {collection}.")?;
        }
        Cmd::Logs { action, lines, level } => match action {
            None => {
                let v = log::tail(lines, level);
                if v.is_empty() {
                    writeln!(out, "(no log entries)")?;
                }
                for l in v {
                    writeln!(out, "{l}")?;
                }
            }
            Some(LogsAction::Path) => writeln!(out, "{}", log::file_location())?,
            Some(LogsAction::Clear) => {
                log::clear();
                writeln!(out, "Log cleared.")?;
            }
            Some(LogsAction::Export { path }) => {
                let dest = path.map(PathBuf::from).map(|p| if p.is_dir() { p.join(format!("printsearch-logs-{}.txt", log::stamp_for_filename())) } else { p });
                let written = log::export_to(dest, &system_info(ctx).await).context("could not write the export")?;
                writeln!(out, "Log exported to {}", written.display())?;
            }
        },
        Cmd::Status => {
            writeln!(out, "{}", system_info(ctx).await)?;
            if ctx.in_app {
                writeln!(out, "Running: yes ({})", crate::net::url())?;
            } else {
                let up = probe_running().await;
                writeln!(out, "Running: {}", if up { "yes" } else { "no" })?;
            }
        }
        Cmd::Quit => {
            if ctx.in_app {
                crate::app::request_quit();
                writeln!(out, "PrintSearch is closing.")?;
            } else {
                let r = crate::providers::client().post(format!("{}/api/quit", crate::net::url())).timeout(Duration::from_secs(5)).send().await;
                match r {
                    Ok(resp) if resp.status().is_success() => writeln!(out, "PrintSearch is closing.")?,
                    _ => writeln!(out, "No running PrintSearch found.")?,
                }
            }
        }
    }
    Ok(())
}

async fn probe_running() -> bool {
    crate::providers::client()
        .get(format!("{}/api/status", crate::net::url()))
        .timeout(Duration::from_secs(2))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// Diagnostic header (version, system, browser, connected sites…). Contains no token.
pub async fn system_info(ctx: &Ctx) -> String {
    let c = ctx.cfg.lock().await.clone();
    let connected: Vec<&str> = Site::ALL.iter().filter(|s| c.token_for(**s).is_some()).map(|s| s.key()).collect();
    let in_use = match browser::effective(c.browser.as_deref()) {
        Some(f) => format!("{} ({})", f.name, f.path),
        None => "none found".into(),
    };
    format!(
        "PrintSearch {}\nPlatform: {} {}\nBrowser choice: {} | in use: {}\nConnected sites: {}\nInterface language: {}\nLog file: {}",
        update::current(),
        std::env::consts::OS,
        std::env::consts::ARCH,
        c.browser.as_deref().unwrap_or("auto"),
        in_use,
        if connected.is_empty() { "none".to_string() } else { connected.join(", ") },
        c.lang.as_deref().unwrap_or("en"),
        log::file_location()
    )
}

// ---------------------------------------------------------------------------------------------
// Built-in console: one line of text -> one command
// ---------------------------------------------------------------------------------------------

/// Splits a line like a simple shell: spaces, "double quotes" and 'single quotes'.
/// Inside "…", only \" and \\ are escapes (a Windows path such as "C:\Users\me" stays intact).
pub fn split_args(s: &str) -> Result<Vec<String>, String> {
    let (mut out, mut cur, mut in_tok, mut quote) = (Vec::new(), String::new(), false, None::<char>);
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) if c == q => quote = None,
            Some('"') if c == '\\' => match chars.peek() {
                Some(&n) if n == '"' || n == '\\' => {
                    cur.push(n);
                    chars.next();
                }
                _ => cur.push('\\'),
            },
            Some(_) => cur.push(c),
            None => match c {
                '"' | '\'' => {
                    quote = Some(c);
                    in_tok = true;
                }
                c if c.is_whitespace() => {
                    if in_tok {
                        out.push(std::mem::take(&mut cur));
                        in_tok = false;
                    }
                }
                _ => {
                    cur.push(c);
                    in_tok = true;
                }
            },
        }
    }
    if quote.is_some() {
        return Err("unclosed quote".into());
    }
    if in_tok {
        out.push(cur);
    }
    Ok(out)
}

/// Masks the value of --token so that the token never appears in the log.
fn redact(args: &[String]) -> String {
    let mut shown = Vec::with_capacity(args.len());
    let mut mask_next = false;
    for a in args {
        if mask_next {
            shown.push("***".to_string());
            mask_next = false;
        } else if a == "--token" {
            shown.push(a.clone());
            mask_next = true;
        } else if a.starts_with("--token=") {
            shown.push("--token=***".to_string());
        } else {
            shown.push(a.clone());
        }
    }
    shown.join(" ")
}

/// Runs a console line and returns the text to display.
pub async fn run_line(ctx: &Ctx, line: &str) -> String {
    let line = line.trim();
    if line.is_empty() {
        return String::new();
    }
    let args = match split_args(line) {
        Ok(a) => a,
        Err(e) => return format!("error: {e}\n"),
    };
    log_info!("console: {}", redact(&args));
    let cli = match Cli::try_parse_from(std::iter::once("printsearch".to_string()).chain(args)) {
        Ok(c) => c,
        Err(e) => return e.render().to_string(), // help, version or usage error
    };
    let Some(cmd) = cli.cmd else {
        return "Type `help` to list the commands.\n".into();
    };
    let mut out = String::new();
    if let Err(e) = execute(cmd, ctx, &mut out).await {
        let _ = writeln!(out, "error: {}", ctx.cfg.lock().await.scrub(&format!("{e:#}")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sp(s: &str) -> Vec<String> {
        split_args(s).unwrap()
    }

    #[test]
    fn splits_words_and_quotes() {
        assert_eq!(sp("search benchy --limit 5"), ["search", "benchy", "--limit", "5"]);
        assert_eq!(sp(r#"search "phone stand" --price free"#), ["search", "phone stand", "--price", "free"]);
        assert_eq!(sp("search 'a b'  c"), ["search", "a b", "c"]);
        assert_eq!(sp("  "), Vec::<String>::new());
        assert_eq!(sp(r#"x """#), ["x", ""]);
    }

    #[test]
    fn keeps_windows_paths() {
        assert_eq!(sp(r#"logs export "C:\Users\me\log.txt""#), ["logs", "export", r"C:\Users\me\log.txt"]);
        assert_eq!(sp(r"logs export C:\Users\me\log.txt"), ["logs", "export", r"C:\Users\me\log.txt"]);
        assert_eq!(sp(r#"say "a \"quoted\" word""#), ["say", r#"a "quoted" word"#]);
    }

    #[test]
    fn unclosed_quote_is_an_error() {
        assert!(split_args("search \"oops").is_err());
    }

    #[test]
    fn tokens_are_redacted() {
        let a = sp("login printables --token SECRET123");
        assert!(!redact(&a).contains("SECRET123"));
        assert!(redact(&a).contains("--token ***"));
        let b = sp("login printables --token=SECRET123");
        assert!(!redact(&b).contains("SECRET123"));
    }

    #[test]
    fn parses_commands() {
        assert!(Cli::try_parse_from(["printsearch", "search", "vase", "--price", "free", "--sort", "likes"]).is_ok());
        assert!(Cli::try_parse_from(["printsearch", "logs", "-n", "20", "--level", "warn"]).is_ok());
        assert!(Cli::try_parse_from(["printsearch", "logs", "export", "out.txt"]).is_ok());
        assert!(Cli::try_parse_from(["printsearch", "bogus"]).is_err());
    }
}
