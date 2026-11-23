mod browser;
mod config;
mod providers;
mod search;
mod server;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use config::Config;
use providers::{provider, Site};
use search::{search_all, Filters, PriceFilter, Sort};

#[derive(Parser)]
#[command(name = "printsearch", version, about = "Unified search: MakerWorld, Thingiverse, Printables")]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Search one or more sites
    Search {
        query: String,
        #[arg(short, long, value_enum, num_args = 1.., value_delimiter = ',')]
        sites: Vec<Site>,
        /// Results requested per site
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
    /// Log in (Thingiverse: OAuth or --token; others: --token pasted from the browser)
    Login {
        #[arg(value_enum)]
        site: Site,
        #[arg(long)]
        token: Option<String>,
    },
    /// Start the local web interface (http://localhost:8787). With no command: starts it and opens the browser
    Serve {
        /// Open the browser automatically
        #[arg(long)]
        open: bool,
    },
    /// List compatible browsers, or pick one: `browser auto|chrome|edge|brave|vivaldi|chromium|<path>`
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
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut cfg = Config::load()?;
    browser::set_choice(cfg.browser.clone());

    match cli.cmd.unwrap_or(Cmd::Serve { open: true }) {
        Cmd::Search { query, sites, limit, page, price, sort, min_likes, min_downloads } => {
            let sites = if sites.is_empty() { Site::ALL.to_vec() } else { sites };
            let filters = Filters { price, sort, min_likes, min_downloads };
            let out = search_all(&cfg, &sites, &query, limit, page.max(1), &filters).await;
            for (s, e) in &out.errors {
                eprintln!("[{s}] error: {e}");
            }
            for m in &out.results {
                let price = match m.price {
                    Some(p) if p > 0.0 => format!("{p}"),
                    Some(_) => "free".into(),
                    None => "?".into(),
                };
                println!(
                    "[{:<11}] {} — {}  ♥{} ⬇{}  price:{}\n              {}  (id: {})",
                    m.site.key(),
                    m.title,
                    m.author,
                    m.likes.map_or("?".into(), |x| x.to_string()),
                    m.downloads.map_or("?".into(), |x| x.to_string()),
                    price,
                    m.url,
                    m.id
                );
            }
            eprintln!("{} result(s)", out.results.len());
        }
        Cmd::Login { site, token } => {
            let t = match (site, token) {
                (_, Some(t)) => t,
                (Site::Thingiverse, None) => providers::thingiverse::oauth_login().await?,
                _ => anyhow::bail!("for this site, provide --token (see README)"),
            };
            cfg.tokens.insert(site.key().into(), t);
            cfg.save()?;
            println!("Logged in to {}.", site.key());
        }
        Cmd::Serve { open } => server::run(open).await?,
        Cmd::Browser { choice } => match choice {
            None => {
                let current = cfg.browser.clone();
                println!("Selected: {}", current.as_deref().unwrap_or("auto"));
                for f in browser::detect() {
                    println!("  {:<9} {}  ({})", f.id, f.name, f.path);
                }
                match browser::effective(current.as_deref()) {
                    Some(f) => println!("In use: {} ({})", f.name, f.path),
                    None => println!("No compatible browser found (Chrome, Edge, Brave, Vivaldi, Chromium)."),
                }
            }
            Some(c) => {
                browser::validate_choice(&c)?;
                cfg.browser = if c == "auto" { None } else { Some(c.clone()) };
                cfg.save()?;
                println!("Browser set to {c}.");
            }
        },
        Cmd::Like { site, id } => {
            let t = cfg.token_for(site).context("not logged in: run `login` first")?;
            provider(site).like(&id, &t).await?;
            println!("Model {id} liked on {}.", site.key());
        }
        Cmd::Collect { site, collection, id } => {
            let t = cfg.token_for(site).context("not logged in: run `login` first")?;
            provider(site).add_to_collection(&collection, &id, &t).await?;
            println!("Model {id} added to collection {collection}.");
        }
    }
    Ok(())
}
