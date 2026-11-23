//! Printables has no documented public API: we use the GraphQL API used by their website.
//! ⚠ Unofficial: field/mutation names to verify with the browser's Network tab,
//! they may change without notice. The (Bearer) token is obtained from the logged-in browser.
use super::*;

pub struct Printables {
    http: reqwest::Client,
}
impl Printables {
    pub fn new() -> Self {
        Self { http: client() }
    }
    async fn gql(&self, query: &str, vars: Value, token: Option<&str>) -> Result<Value> {
        let mut req = self
            .http
            .post("https://api.printables.com/graphql/")
            .json(&serde_json::json!({ "query": query, "variables": vars }));
        if let Some(t) = token {
            req = req.bearer_auth(t);
        }
        let v: Value = req.send().await?.error_for_status()?.json().await?;
        if let Some(e) = v.get("errors") {
            bail!("Printables GraphQL error: {e}");
        }
        Ok(v)
    }
}

#[async_trait]
impl Provider for Printables {
    async fn search(&self, query: &str, limit: usize, page: usize, token: Option<&str>) -> Result<Vec<Model>> {
        // `offset` is only added from page 2 onwards; `price` is removed if it is rejected.
        let (decl, arg) = if page > 1 { (", $offset: Int", ", offset: $offset") } else { ("", "") };
        let build = |extra: &str| {
            format!(
                "query Search($q: String!, $limit: Int{decl}) {{ result: searchPrints2(query: $q, limit: $limit{arg}) {{
                   items {{ id name slug likesCount downloadCount image {{ filePath }} user {{ publicUsername }} {extra} }} }} }}"
            )
        };
        let mut vars = serde_json::json!({ "q": query, "limit": limit });
        if page > 1 {
            vars["offset"] = serde_json::json!((page - 1) * limit);
        }
        let v = match self.gql(&build("price"), vars.clone(), token).await {
            Ok(v) => v,
            Err(_) => self.gql(&build(""), vars, token).await?,
        };
        let items = v["data"]["result"]["items"].as_array().cloned().unwrap_or_default();
        Ok(items
            .iter()
            .map(|i| {
                let id = text(i, "id").unwrap_or_default();
                let slug = text(i, "slug").unwrap_or_default();
                Model {
                    site: Site::Printables,
                    url: format!("https://www.printables.com/model/{id}-{slug}"),
                    thumbnail: i["image"]["filePath"].as_str().map(|p| format!("https://media.printables.com/{p}")),
                    price: i["price"].as_f64(),
                    id,
                    title: text(i, "name").unwrap_or_default(),
                    author: i["user"]["publicUsername"].as_str().unwrap_or_default().to_string(),
                    likes: i["likesCount"].as_u64(),
                    downloads: i["downloadCount"].as_u64(),
                }
            })
            .collect())
    }

    async fn like(&self, id: &str, token: &str) -> Result<()> {
        const M: &str = "mutation Like($id: ID!) { likePrint(printId: $id) { ok } }";
        self.gql(M, serde_json::json!({ "id": id }), Some(token)).await?;
        Ok(())
    }
}
