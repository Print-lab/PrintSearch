//! Cults (cults3d.com) through its official GraphQL API.
//!
//! The website is protected by a bot check (Cloudflare) that blocks automated browsing, so reading its pages
//! with the background browser does not work. The API needs a free key: create one in your Cults account
//! (cults3d.com/en/api/keys). Credentials are stored as "username:api_key" and sent with HTTP Basic auth.
use super::*;
use anyhow::anyhow;
use serde_json::json;

pub struct Cults {
    http: reqwest::Client,
}

impl Cults {
    pub fn new() -> Self {
        Self { http: client() }
    }
}

const ENDPOINT: &str = "https://cults3d.com/graphql";
const FULL_FIELDS: &str = "likesCount downloadsCount price(currency: USD) { cents } creator { nick }";

fn query(extra_fields: &str, with_offset: bool) -> String {
    let (decl, arg) = if with_offset { (", $offset: Int", ", offset: $offset") } else { ("", "") };
    format!(
        "query Search($q: String!, $limit: Int{decl}) {{ creationsSearchBatch(query: $q, limit: $limit{arg}) {{ \
         results {{ name(locale: EN) url(locale: EN) shortUrl illustrationImageUrl {extra_fields} }} }} }}"
    )
}

/// "username:api_key" -> (username, api_key)
pub fn split_credentials(token: &str) -> Option<(&str, &str)> {
    let (user, key) = token.split_once(':')?;
    (!user.trim().is_empty() && !key.trim().is_empty()).then_some((user.trim(), key.trim()))
}

impl Cults {
    async fn run(&self, user: &str, key: &str, fields: &str, vars: &Value, with_offset: bool) -> Result<Vec<Value>> {
        let resp = self
            .http
            .post(ENDPOINT)
            .basic_auth(user, Some(key))
            .json(&json!({ "query": query(fields, with_offset), "variables": vars }))
            .send()
            .await?;
        let status = resp.status();
        if status == 401 || status == 403 {
            bail!("Cults refused the credentials (HTTP {status}): check your username and API key in Settings");
        }
        if !status.is_success() {
            let body: String = resp.text().await.unwrap_or_default().chars().take(200).collect();
            bail!("Cults API: HTTP {status} {body}");
        }
        let v: Value = resp.json().await?;
        if let Some(errors) = v.get("errors") {
            let first = errors.get(0).and_then(|e| e["message"].as_str()).unwrap_or("unknown error");
            bail!("GraphQL error from Cults: {first}");
        }
        Ok(v["data"]["creationsSearchBatch"]["results"].as_array().cloned().unwrap_or_default())
    }
}

#[async_trait]
impl Provider for Cults {
    async fn search(&self, query: &str, limit: usize, page: usize, token: Option<&str>) -> Result<Vec<Model>> {
        let (user, key) = token
            .and_then(split_credentials)
            .ok_or_else(|| anyhow!("Cults needs your username and a free API key (Settings → Cults)"))?;
        let mut vars = json!({ "q": query, "limit": limit });
        if page > 1 {
            vars["offset"] = json!((page - 1) * limit);
        }
        // Some field might not exist on the API side: retry with the essential ones before giving up.
        let rows = match self.run(user, key, FULL_FIELDS, &vars, page > 1).await {
            Ok(r) => r,
            Err(e) if e.to_string().starts_with("GraphQL error") => self.run(user, key, "", &vars, page > 1).await.map_err(|_| e)?,
            Err(e) => return Err(e),
        };
        Ok(rows.iter().map(to_model).collect())
    }
}

/// One `results` entry of the GraphQL answer -> a model.
fn to_model(r: &Value) -> Model {
    let short = text(r, "shortUrl").unwrap_or_default();
    let url = text(r, "url").filter(|u| !u.is_empty()).unwrap_or_else(|| short.clone());
    let id = short.trim_end_matches('/').rsplit('/').next().unwrap_or("").trim_start_matches(':').to_string();
    Model {
        site: Site::Cults3d,
        id,
        title: text(r, "name").unwrap_or_default(),
        author: r["creator"]["nick"].as_str().unwrap_or_default().to_string(),
        url,
        thumbnail: text(r, "illustrationImageUrl"),
        price: r["price"]["cents"].as_f64().map(|c| c / 100.0),
        likes: r["likesCount"].as_u64(),
        downloads: r["downloadsCount"].as_u64(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_are_split_on_the_first_colon() {
        assert_eq!(split_credentials("alice:abc123"), Some(("alice", "abc123")));
        assert_eq!(split_credentials(" alice : abc:123 "), Some(("alice", "abc:123")));
        assert_eq!(split_credentials("alice"), None);
        assert_eq!(split_credentials(":key"), None);
        assert_eq!(split_credentials("user:"), None);
    }

    #[test]
    fn full_answer_is_read() {
        let r = serde_json::json!({
            "name": "Benchy the boat", "url": "https://cults3d.com/en/3d-model/various/benchy", "shortUrl": "https://cults3d.com/:1234567",
            "illustrationImageUrl": "https://images.cults3d.com/x.jpg", "likesCount": 42, "downloadsCount": 1500,
            "price": {"cents": 1250}, "creator": {"nick": "alice"}
        });
        let m = to_model(&r);
        assert_eq!((m.id.as_str(), m.title.as_str(), m.author.as_str()), ("1234567", "Benchy the boat", "alice"));
        assert_eq!(m.url, "https://cults3d.com/en/3d-model/various/benchy");
        assert_eq!((m.likes, m.downloads, m.price), (Some(42), Some(1500), Some(12.5)));
        assert_eq!(m.thumbnail.as_deref(), Some("https://images.cults3d.com/x.jpg"));
        assert_eq!(m.site, Site::Cults3d);
    }

    #[test]
    fn minimal_answer_and_free_model() {
        let m = to_model(&serde_json::json!({"name": "Free thing", "shortUrl": "https://cults3d.com/:9/", "price": {"cents": 0}}));
        assert_eq!(m.id, "9");
        assert_eq!(m.url, "https://cults3d.com/:9/"); // no `url` field: falls back to the short address
        assert_eq!(m.price, Some(0.0));
        assert_eq!((m.likes, m.downloads, m.author.as_str()), (None, None, ""));
        assert_eq!(to_model(&serde_json::json!({"name": "x", "shortUrl": "https://cults3d.com/:5"})).price, None);
    }

    #[test]
    fn query_has_offset_only_when_asked() {
        assert!(!query("", false).contains("offset"));
        let q = query(FULL_FIELDS, true);
        assert!(q.contains("$offset: Int") && q.contains("offset: $offset") && q.contains("creator { nick }"));
        assert!(q.matches('{').count() == q.matches('}').count(), "braces must balance");
    }
}
