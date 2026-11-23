use anyhow::{bail, Result};
use async_trait::async_trait;
use serde_json::Value;

pub mod makerworld;
pub mod printables;
pub mod thingiverse;

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Site {
    Thingiverse,
    Printables,
    Makerworld,
}

impl Site {
    pub const ALL: [Site; 3] = [Site::Thingiverse, Site::Printables, Site::Makerworld];
    pub fn from_key(k: &str) -> Option<Site> {
        Site::ALL.into_iter().find(|s| s.key() == k)
    }
    pub fn key(&self) -> &'static str {
        match self {
            Site::Thingiverse => "thingiverse",
            Site::Printables => "printables",
            Site::Makerworld => "makerworld",
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Model {
    pub site: Site,
    pub id: String,
    pub title: String,
    pub author: String,
    pub url: String,
    pub thumbnail: Option<String>,
    /// None = unknown; Some(0.0) = free; Some(x>0) = paid
    pub price: Option<f64>,
    pub likes: Option<u64>,
    pub downloads: Option<u64>,
}

#[async_trait]
pub trait Provider: Send + Sync {
    /// `limit` = results per page, `page` starts at 1.
    async fn search(&self, query: &str, limit: usize, page: usize, token: Option<&str>) -> Result<Vec<Model>>;
    async fn like(&self, _id: &str, _token: &str) -> Result<()> {
        bail!("'like' not supported for this site")
    }
    async fn add_to_collection(&self, _collection: &str, _id: &str, _token: &str) -> Result<()> {
        bail!("adding to a collection not supported for this site")
    }
}

pub fn provider(site: Site) -> Box<dyn Provider> {
    match site {
        Site::Thingiverse => Box::new(thingiverse::Thingiverse::new()),
        Site::Printables => Box::new(printables::Printables::new()),
        Site::Makerworld => Box::new(makerworld::MakerWorld::new()),
    }
}

/// A single HTTP client for the whole program (connections reused) with timeouts,
/// so that a site that does not respond never blocks a search.
pub(crate) fn client() -> reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .user_agent(concat!("printsearch/", env!("CARGO_PKG_VERSION"), " (+personal tool)"))
                .connect_timeout(std::time::Duration::from_secs(8))
                .timeout(std::time::Duration::from_secs(25))
                .build()
                .expect("client http")
        })
        .clone()
}

pub(crate) fn text(v: &Value, key: &str) -> Option<String> {
    match v.get(key)? {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}
