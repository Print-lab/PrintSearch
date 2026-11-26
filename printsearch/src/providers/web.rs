//! Sites without a public API usable without an account (Thangs, Cults, MyMiniFactory): their results page is
//! read with the background browser (see browser.rs), as for MakerWorld.
//! Only the title, the picture and the link are read (no author, likes or price).
//! ⚠ Search addresses and link shapes are unofficial: they can change without notice.
use super::*;
use crate::browser;

pub struct WebSite {
    site: Site,
    /// Address of the results page for a search (already encoded)
    search_url: fn(&str) -> String,
    /// Recognises a model link; the 1st capture group is the identifier
    id_re: &'static str,
}

pub fn thangs() -> WebSite {
    WebSite {
        site: Site::Thangs,
        search_url: |q| format!("https://thangs.com/search/{}?scope=thangs", urlencoding::encode(q)),
        // https://thangs.com/designer/<auteur>/3d-model/<nom>-<id>   ou   https://thangs.com/m/<id>
        id_re: r"(?:/designer/[^/]+/3d-model/[^/?#]*-|/m/)(\d+)(?:[/?#]|$)",
    }
}

pub fn cults3d() -> WebSite {
    WebSite {
        site: Site::Cults3d,
        search_url: |q| format!("https://cults3d.com/en/search?q={}", urlencoding::encode(q)),
        // https://cults3d.com/en/3d-model/<category>/<name> : the identifier is "category/name"
        id_re: r"/3d-model/([^/?#]+/[^/?#]+)",
    }
}

pub fn myminifactory() -> WebSite {
    WebSite {
        site: Site::Myminifactory,
        search_url: |q| format!("https://www.myminifactory.com/search/?query={}", urlencoding::encode(q)),
        // https://www.myminifactory.com/object/3d-print-<nom>-<id>
        id_re: r"/object/3d-print-[^/?#]*?-(\d+)(?:[/?#]|$)",
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
    fn search_urls_are_encoded() {
        assert_eq!((thangs().search_url)("phone stand"), "https://thangs.com/search/phone%20stand?scope=thangs");
        assert_eq!((cults3d().search_url)("a&b"), "https://cults3d.com/en/search?q=a%26b");
        assert_eq!((myminifactory().search_url)("dragon"), "https://www.myminifactory.com/search/?query=dragon");
    }

    #[test]
    fn sites_are_distinct() {
        assert_eq!(thangs().site, Site::Thangs);
        assert_eq!(cults3d().site, Site::Cults3d);
        assert_eq!(myminifactory().site, Site::Myminifactory);
    }
}
