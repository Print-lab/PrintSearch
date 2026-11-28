//! MyMiniFactory through its official API (v2).
//!
//! MyMiniFactory blocks automated browsing (bot detection) and asks scrapers to use its API instead, so a free
//! API key is required: create an application in your MyMiniFactory account (myminifactory.com/pages/for-developers).
//! ⚠ This integration could not be tested against a live key when it was written: the response is read defensively
//! and any API error is shown as is, so that a wrong field name is easy to spot and fix.
use super::*;
use anyhow::anyhow;

pub struct MyMiniFactory {
    http: reqwest::Client,
}

impl MyMiniFactory {
    pub fn new() -> Self {
        Self { http: client() }
    }
}

const SEARCH: &str = "https://www.myminifactory.com/api/v2/search";

/// First non-empty string found among the given paths.
fn first_text(v: &Value, paths: &[&[&str]]) -> Option<String> {
    paths.iter().find_map(|p| {
        let mut cur = v;
        for k in *p {
            cur = cur.get(*k)?;
        }
        cur.as_str().filter(|s| !s.is_empty()).map(String::from)
    })
}

#[async_trait]
impl Provider for MyMiniFactory {
    async fn search(&self, query: &str, limit: usize, page: usize, token: Option<&str>) -> Result<Vec<Model>> {
        let key = token.map(str::trim).filter(|k| !k.is_empty()).ok_or_else(|| anyhow!("MyMiniFactory needs a free API key (Settings → MyMiniFactory)"))?;
        let resp = self
            .http
            .get(SEARCH)
            .query(&[("q", query), ("key", key), ("page", &page.to_string()), ("per_page", &limit.min(50).to_string())])
            .send()
            .await
            .map_err(|e| e.without_url())?; // the request address contains the API key: never keep it in an error
        let status = resp.status();
        if status == 401 || status == 403 {
            bail!("MyMiniFactory refused the API key (HTTP {status}): check it in Settings");
        }
        if !status.is_success() {
            let body: String = resp.text().await.unwrap_or_default().chars().take(200).collect();
            bail!("MyMiniFactory API: HTTP {status} {body}");
        }
        let v: Value = resp.json().await.map_err(|e| e.without_url())?;
        let items = v["items"].as_array().or_else(|| v["objects"].as_array()).or_else(|| v.as_array()).cloned().unwrap_or_default();
        Ok(items.iter().map(to_model).collect())
    }
}

/// One entry of the `items` array -> a model. Read defensively: the exact field names could not be checked live.
fn to_model(it: &Value) -> Model {
    let id = text(it, "id").unwrap_or_default();
    // `images` is an array of objects holding several sizes
    let thumbnail = it["images"].as_array().and_then(|a| a.first()).and_then(|img| first_text(img, &[&["thumbnail", "url"], &["standard", "url"], &["original", "url"]])).or_else(|| first_text(it, &[&["thumbnail"]]));
    Model {
        site: Site::Myminifactory,
        url: first_text(it, &[&["url"]]).unwrap_or_else(|| format!("https://www.myminifactory.com/object/{id}")),
        id,
        title: first_text(it, &[&["name"], &["title"]]).unwrap_or_default(),
        author: first_text(it, &[&["designer", "name"], &["designer", "username"], &["designer", "nickname"]]).unwrap_or_default(),
        thumbnail,
        price: it["price"].as_f64(),
        likes: it["likes"].as_u64().or_else(|| it["likes_count"].as_u64()),
        downloads: it["downloads"].as_u64().or_else(|| it["downloads_count"].as_u64()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn answer_is_read_defensively() {
        let full = json!({
            "id": 261257, "name": "Dragon", "url": "https://www.myminifactory.com/object/3d-print-dragon-261257",
            "designer": {"name": "", "username": "bob"}, "likes": 7,
            "images": [{"thumbnail": {"url": "https://cdn.mmf/t.jpg"}, "standard": {"url": "https://cdn.mmf/s.jpg"}}]
        });
        let m = to_model(&full);
        assert_eq!((m.id.as_str(), m.title.as_str(), m.author.as_str()), ("261257", "Dragon", "bob"));
        assert_eq!(m.thumbnail.as_deref(), Some("https://cdn.mmf/t.jpg"));
        assert_eq!((m.likes, m.price, m.site), (Some(7), None, Site::Myminifactory));
        // missing fields never break anything, and the page address is rebuilt from the id
        let bare = to_model(&json!({"id": "12", "title": "Bare"}));
        assert_eq!((bare.title.as_str(), bare.url.as_str(), bare.thumbnail), ("Bare", "https://www.myminifactory.com/object/12", None));
        let only_standard = to_model(&json!({"id": 1, "name": "x", "images": [{"standard": {"url": "https://cdn.mmf/s.jpg"}}]}));
        assert_eq!(only_standard.thumbnail.as_deref(), Some("https://cdn.mmf/s.jpg"));
    }

    #[test]
    fn first_text_follows_paths_and_skips_empty() {
        let v = json!({"designer": {"name": "", "username": "bob"}, "name": "Dragon"});
        assert_eq!(first_text(&v, &[&["designer", "name"], &["designer", "username"]]), Some("bob".into()));
        assert_eq!(first_text(&v, &[&["name"]]), Some("Dragon".into()));
        assert_eq!(first_text(&v, &[&["missing"]]), None);
    }
}
