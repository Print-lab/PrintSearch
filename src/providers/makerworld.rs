//! MakerWorld has no public API and blocks plain HTTP requests (bot protection).
//! Its search page is therefore read with a driven Chromium-based browser (see browser.rs): a compatible
//! browser must be installed. Likes/downloads/price are not read in this mode.
use super::*;
use crate::browser;

pub struct MakerWorld;

impl MakerWorld {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Provider for MakerWorld {
    async fn search(&self, query: &str, limit: usize, page: usize, _token: Option<&str>) -> Result<Vec<Model>> {
        let url = format!("https://makerworld.com/en/search/models?keyword={}", urlencoding::encode(query));
        let cards = browser::scrape_cards(url, r"/models/(\d+)", page * limit).await?;
        Ok(browser::page_slice(cards, limit, page)
            .into_iter()
            .map(|c| Model {
                site: Site::Makerworld,
                id: c.id,
                title: c.title,
                author: String::new(),
                url: c.href,
                thumbnail: c.img,
                price: None,
                likes: None,
                downloads: None,
            })
            .collect())
    }
    // like / collections: to implement after inspecting the network calls of the signed-in site.
}
