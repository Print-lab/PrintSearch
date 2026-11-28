//! Sites without a usable public API and without bot protection (Thangs): their results page is read with the
//! background browser (see browser.rs), like MakerWorld.
//! Only the title, picture and link are read (no author, likes or price).
//! ⚠ Search addresses and link shapes are unofficial: they can change without notice.
use super::*;
use crate::browser;

pub struct WebSite {
    site: Site,
    /// Address of the results page for a search (already encoded)
    search_url: fn(&str) -> String,
    /// Recognizes a model link; the 1st capture group is the identifier
    id_re: &'static str,
}

pub fn thangs() -> WebSite {
    WebSite {
        site: Site::Thangs,
        search_url: |q| format!("https://thangs.com/search/{}?scope=thangs", urlencoding::encode(q)),
        // https://thangs.com/designer/<author>/3d-model/<name>-<id>   or   https://thangs.com/m/<id>
        id_re: r"(?:/designer/[^/]+/3d-model/[^/?#]*-|/m/)(\d+)(?:[/?#]|$)",
    }
}

#[async_trait]
impl Provider for WebSite {
    async fn search(&self, query: &str, limit: usize, page: usize, _token: Option<&str>) -> Result<Vec<Model>> {
        let cards = browser::scrape_cards((self.search_url)(query), self.id_re, page * limit).await?;
        Ok(browser::page_slice(cards, limit, page)
            .into_iter()
            .map(|c| Model {
                site: self.site,
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_url_is_encoded() {
        assert_eq!((thangs().search_url)("phone stand"), "https://thangs.com/search/phone%20stand?scope=thangs");
        assert_eq!((thangs().search_url)("a&b"), "https://thangs.com/search/a%26b?scope=thangs");
    }

    #[test]
    fn site_is_thangs() {
        assert_eq!(thangs().site, Site::Thangs);
    }
}
