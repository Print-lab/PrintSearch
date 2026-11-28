//! Official API: https://www.thingiverse.com/developers (token required).
//! Without a token, or if the API fails, falls back to reading the website with a driven browser (see browser.rs).
//! ⚠ Check the like/collections paths in the official documentation: they have changed before.
use super::*;
use crate::browser;
use anyhow::Context;

pub struct Thingiverse {
    http: reqwest::Client,
}
impl Thingiverse {
    pub fn new() -> Self {
        Self { http: client() }
    }

    async fn search_api(&self, query: &str, limit: usize, page: usize, token: &str) -> Result<Vec<Model>> {
        let url = format!("{API}/search/{}", urlencoding::encode(query));
        let resp = self
            .http
            .get(url)
            .query(&[("per_page", limit.to_string()), ("page", page.to_string())])
            .bearer_auth(token)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body: String = resp.text().await.unwrap_or_default().chars().take(200).collect();
            bail!("API Thingiverse : HTTP {status} {body}");
        }
        let v: Value = resp.json().await?;
        let hits = v.get("hits").and_then(|h| h.as_array()).or_else(|| v.as_array()).cloned().unwrap_or_default();
        Ok(hits
            .iter()
            .take(limit)
            .map(|h| Model {
                site: Site::Thingiverse,
                id: text(h, "id").unwrap_or_default(),
                title: text(h, "name").unwrap_or_default(),
                author: h.get("creator").and_then(|c| text(c, "name")).unwrap_or_default(),
                url: text(h, "public_url").unwrap_or_default(),
                thumbnail: text(h, "thumbnail"),
                price: Some(0.0), // Thingiverse: everything is free
                likes: h.get("like_count").and_then(|x| x.as_u64()),
                downloads: h.get("download_count").and_then(|x| x.as_u64()),
            })
            .collect())
    }

    async fn search_web(&self, query: &str, limit: usize, page: usize) -> Result<Vec<Model>> {
        let url = format!("https://www.thingiverse.com/search?q={}&type=things&sort=relevant", urlencoding::encode(query));
        let cards = browser::scrape_cards(url, r"/thing:(\d+)", page * limit).await?;
        Ok(browser::page_slice(cards, limit, page)
            .into_iter()
            .map(|c| Model {
                site: Site::Thingiverse,
                id: c.id,
                title: c.title,
                author: String::new(),
                url: c.href,
                thumbnail: c.img,
                price: Some(0.0),
                likes: None,
                downloads: None,
            })
            .collect())
    }
}

const API: &str = "https://api.thingiverse.com";

#[async_trait]
impl Provider for Thingiverse {
    async fn search(&self, query: &str, limit: usize, page: usize, token: Option<&str>) -> Result<Vec<Model>> {
        match token {
            Some(t) => match self.search_api(query, limit, page, t).await {
                Ok(v) => Ok(v),
                Err(api) => self
                    .search_web(query, limit, page)
                    .await
                    .map_err(|web| anyhow::anyhow!("{api:#}; browser fallback: {web:#}")),
            },
            None => self.search_web(query, limit, page).await,
        }
    }

    async fn like(&self, id: &str, token: &str) -> Result<()> {
        self.http
            .post(format!("{API}/things/{id}/favorite"))
            .bearer_auth(token)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn add_to_collection(&self, collection: &str, id: &str, token: &str) -> Result<()> {
        self.http
            .post(format!("{API}/collections/{collection}/thing/{id}"))
            .bearer_auth(token)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}

pub const REDIRECT: &str = "http://localhost:8787/callback";

pub fn authorize_url() -> Result<String> {
    let id = std::env::var("THINGIVERSE_CLIENT_ID").context("THINGIVERSE_CLIENT_ID manquant")?;
    Ok(format!(
        "https://www.thingiverse.com/login/oauth/authorize?client_id={id}&redirect_uri={}&response_type=code",
        urlencoding::encode(REDIRECT)
    ))
}

pub async fn exchange_code(code: &str) -> Result<String> {
    let id = std::env::var("THINGIVERSE_CLIENT_ID").context("THINGIVERSE_CLIENT_ID manquant")?;
    let secret = std::env::var("THINGIVERSE_CLIENT_SECRET").context("THINGIVERSE_CLIENT_SECRET manquant")?;
    let body = client()
        .post("https://www.thingiverse.com/login/oauth/access_token")
        .form(&[("client_id", id.as_str()), ("client_secret", secret.as_str()), ("code", code)])
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    body.split('&')
        .find_map(|kv| kv.strip_prefix("access_token=").map(str::to_string))
        .context("access_token missing from the response")
}

/// Command-line login: short-lived callback server.
pub async fn oauth_login() -> Result<String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    println!("Open this URL in your browser:\n{}\n", authorize_url()?);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8787").await?;
    let (mut sock, _) = listener.accept().await?;
    let mut buf = [0u8; 4096];
    let n = sock.read(&mut buf).await?;
    let req = String::from_utf8_lossy(&buf[..n]).to_string();
    let code = req
        .split("code=")
        .nth(1)
        .and_then(|s| s.split(|c: char| c == '&' || c == ' ').next())
        .context("OAuth code missing from the redirect")?
        .to_string();
    sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\nConnexion reussie, vous pouvez fermer cet onglet.")
        .await?;
    exchange_code(&code).await
}
