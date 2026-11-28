//! Multi-site search + filters, shared by the CLI and the web interface.
use crate::config::Config;
use crate::log::{log_info, log_warn};
use crate::providers::{provider, Model, Site};
use std::cmp::Reverse;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PriceFilter {
    #[default]
    All,
    Free,
    Paid,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Sort {
    #[default]
    Relevance,
    Likes,
    Downloads,
}

#[derive(Clone, Debug, Default)]
pub struct Filters {
    pub price: PriceFilter,
    pub sort: Sort,
    pub min_likes: u64,
    pub min_downloads: u64,
}

pub struct Outcome {
    pub results: Vec<Model>,
    /// Number of results received from the sites before filtering (0 = nothing more to load)
    pub raw: usize,
    pub errors: BTreeMap<String, String>,
    /// Sites that need an API key which is not configured yet (not an error: they are simply not searched)
    pub skipped: Vec<Site>,
}

fn keep(m: &Model, f: &Filters) -> bool {
    let paid = m.price.map_or(false, |p| p > 0.0);
    match f.price {
        PriceFilter::Free if paid => return false,
        PriceFilter::Paid if !paid => return false,
        _ => {}
    }
    // An unknown value (None) does not exclude the model.
    m.likes.map_or(true, |l| l >= f.min_likes) && m.downloads.map_or(true, |d| d >= f.min_downloads)
}

pub async fn search_all(cfg: &Config, sites: &[Site], query: &str, limit: usize, page: usize, f: &Filters) -> Outcome {
    let shown: String = query.chars().take(120).collect(); // for the log: never an oversized line
    let (searchable, skipped): (Vec<Site>, Vec<Site>) = sites.iter().copied().partition(|s| !s.requires_key() || cfg.token_for(*s).is_some());
    let tasks = searchable.iter().map(|&s| {
        let tok = cfg.token_for(s);
        let q = query.to_string();
        async move {
            let started = std::time::Instant::now();
            let r = provider(s).search(&q, limit, page, tok.as_deref()).await;
            (s, r, started.elapsed())
        }
    });

    let mut lists: Vec<Vec<Model>> = vec![];
    let mut errors = BTreeMap::new();
    for (s, r, took) in futures::future::join_all(tasks).await {
        match r {
            Ok(v) => {
                log_info!("search {} {shown:?} (page {page}): {} result(s) in {} ms", s.key(), v.len(), took.as_millis());
                lists.push(v);
            }
            Err(e) => {
                let message = cfg.scrub(&format!("{e:#}")); // never let a key reach the log or the screen
                log_warn!("search {} {shown:?} (page {page}) failed after {} ms: {message}", s.key(), took.as_millis());
                errors.insert(s.key().to_string(), message);
            }
        }
    }
    let raw = lists.iter().map(Vec::len).sum();

    // Interleaving: one result from each site in turn.
    let mut merged = vec![];
    for i in 0..lists.iter().map(Vec::len).max().unwrap_or(0) {
        for l in &lists {
            if let Some(m) = l.get(i) {
                merged.push(m.clone());
            }
        }
    }
    merged.retain(|m| keep(m, f));
    match f.sort {
        Sort::Relevance => {}
        Sort::Likes => merged.sort_by_key(|m| Reverse(m.likes.unwrap_or(0))),
        Sort::Downloads => merged.sort_by_key(|m| Reverse(m.downloads.unwrap_or(0))),
    }
    Outcome { results: merged, raw, errors, skipped }
}
