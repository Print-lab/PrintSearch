use anyhow::{bail, Result};
use async_trait::async_trait;
use serde_json::Value;

pub mod cults;
pub mod makerworld;
pub mod myminifactory;
pub mod printables;
pub mod thingiverse;
pub mod web;

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Site {
    Thingiverse,
    Printables,
    Makerworld,
    Thangs,
    Cults3d,
    Myminifactory,
}

impl Site {
    pub const ALL: [Site; 6] = [Site::Thingiverse, Site::Printables, Site::Makerworld, Site::Thangs, Site::Cults3d, Site::Myminifactory];
    pub fn from_key(k: &str) -> Option<Site> {
        Site::ALL.into_iter().find(|s| s.key() == k)
    }
    pub fn key(&self) -> &'static str {
        match self {
            Site::Thingiverse => "thingiverse",
            Site::Printables => "printables",
            Site::Makerworld => "makerworld",
            Site::Thangs => "thangs",
            Site::Cults3d => "cults3d",
            Site::Myminifactory => "myminifactory",
        }
    }

    /// Actions really available (the others would return an error after a useless call).
    pub fn can_like(&self) -> bool {
        matches!(self, Site::Thingiverse | Site::Printables)
    }
    pub fn can_collect(&self) -> bool {
        matches!(self, Site::Thingiverse)
    }

    /// Sites where credentials (token or API key) can be saved. The others are search-only.
    pub fn supports_account(&self) -> bool {
        matches!(self, Site::Thingiverse | Site::Printables | Site::Makerworld | Site::Cults3d | Site::Myminifactory)
    }

    /// Sites that cannot be searched at all without credentials (official API only): they are skipped until set up.
    pub fn requires_key(&self) -> bool {
        matches!(self, Site::Cults3d | Site::Myminifactory)
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
    /// None = inconnu ; Some(0.0) = gratuit ; Some(x>0) = payant
    pub price: Option<f64>,
    pub likes: Option<u64>,
    pub downloads: Option<u64>,
}

#[async_trait]
pub trait Provider: Send + Sync {
    /// `limit` = results per page, `page` starts at 1.
    async fn search(&self, query: &str, limit: usize, page: usize, token: Option<&str>) -> Result<Vec<Model>>;
    async fn like(&self, _id: &str, _token: &str) -> Result<()> {
        bail!("Like is not available for this site")
    }
    async fn add_to_collection(&self, _collection: &str, _id: &str, _token: &str) -> Result<()> {
        bail!("Collections are not available for this site")
    }
}

pub fn provider(site: Site) -> Box<dyn Provider> {
    match site {
        Site::Thingiverse => Box::new(thingiverse::Thingiverse::new()),
        Site::Printables => Box::new(printables::Printables::new()),
        Site::Makerworld => Box::new(makerworld::MakerWorld::new()),
        Site::Thangs => Box::new(web::thangs()),
        Site::Cults3d => Box::new(cults::Cults::new()),
        Site::Myminifactory => Box::new(myminifactory::MyMiniFactory::new()),
    }
}

/// A single HTTP client for the whole program (connections are reused) with timeouts,
/// so that a site that does not answer never blocks a search.
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
