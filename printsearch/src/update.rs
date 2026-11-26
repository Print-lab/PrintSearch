//! Update check against GitHub (public API, no authentication).
//! The latest published *release* is looked at first; if there is none, the highest version *tag*.
//! Nothing is downloaded or installed: the version numbers are simply compared.
use crate::log::{log_info, log_warn};
use anyhow::{bail, Result};
use serde::Serialize;
use serde_json::Value;
use std::{
    cmp::Ordering,
    sync::Mutex,
    time::{Duration, Instant},
};

const REPO: &str = "Print-lab/PrintSearch";
const CACHE_TTL: Duration = Duration::from_secs(3600);
static CACHE: Mutex<Option<(Instant, UpdateInfo)>> = Mutex::new(None);

#[derive(Serialize, Clone, Debug)]
pub struct UpdateInfo {
    pub current: String,
    /// Latest published version (None = no release or tag on GitHub)
    pub latest: Option<String>,
    pub update_available: bool,
    /// Release page (built here, never taken as-is from the GitHub response)
    pub url: String,
    pub notes: Option<String>,
    pub published_at: Option<String>,
}

pub fn current() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// "v1.2.3-rc1+build" -> ([1,2,3], true)   (true = pre-release)
fn parse(v: &str) -> Option<(Vec<u64>, bool)> {
    let v = v.trim().trim_start_matches(['v', 'V']);
    let v = v.split('+').next()?;
    let (core, pre) = match v.split_once('-') {
        Some((c, _)) => (c, true),
        None => (v, false),
    };
    let nums: Option<Vec<u64>> = core.split('.').map(|p| p.parse().ok()).collect();
    let nums = nums?;
    if nums.is_empty() {
        return None;
    }
    Some((nums, pre))
}

fn cmp_versions(a: &str, b: &str) -> Ordering {
    match (parse(a), parse(b)) {
        (Some((mut x, xp)), Some((mut y, yp))) => {
            let n = x.len().max(y.len()).max(3);
            x.resize(n, 0);
            y.resize(n, 0);
            x.cmp(&y).then_with(|| yp.cmp(&xp)) // with an equal number, a final version > a pre-release
        }
        _ => Ordering::Equal,
    }
}

pub fn is_newer(candidate: &str, current: &str) -> bool {
    parse(candidate).is_some() && parse(current).is_some() && cmp_versions(candidate, current) == Ordering::Greater
}

async fn get(path: &str) -> Result<(u16, Value)> {
    let resp = crate::providers::client()
        .get(format!("https://api.github.com/repos/{REPO}/{path}"))
        .header("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(10))
        .send()
        .await?;
    let status = resp.status().as_u16();
    if status == 403 || status == 429 {
        bail!("GitHub rate limit reached, try again later");
    }
    let body = resp.json::<Value>().await.unwrap_or(Value::Null);
    Ok((status, body))
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max).collect::<String>())
    }
}

/// Via the GitHub API: version number, release notes, publication date.
async fn fetch_api() -> Result<(Option<String>, Option<String>, Option<String>)> {
    let (status, v) = get("releases/latest").await?;
    Ok(match status {
        200 => (
            v["tag_name"].as_str().map(String::from),
            v["body"].as_str().filter(|b| !b.trim().is_empty()).map(|b| truncate(b, 1500)),
            v["published_at"].as_str().map(String::from),
        ),
        404 => {
            // No release published: take the highest version tag.
            let (st, tags) = get("tags?per_page=50").await?;
            if st != 200 {
                bail!("GitHub answered HTTP {st}");
            }
            let best = tags
                .as_array()
                .and_then(|a| a.iter().filter_map(|t| t["name"].as_str()).filter(|n| parse(n).is_some()).max_by(|a, b| cmp_versions(a, b)).map(String::from));
            (best, None, None)
        }
        s => bail!("GitHub answered HTTP {s}"),
    })
}

/// Fallback without the API (so without its rate limit): the web page `releases/latest` redirects to
/// `.../releases/tag/<version>`. Only that redirect is read, without following it.
async fn latest_from_redirect() -> Result<Option<String>> {
    let client = reqwest::Client::builder()
        .user_agent(concat!("printsearch/", env!("CARGO_PKG_VERSION"), " (+personal tool)"))
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(10))
        .build()?;
    let resp = client.get(format!("https://github.com/{REPO}/releases/latest")).send().await?;
    if !resp.status().is_redirection() {
        return Ok(None);
    }
    let loc = resp.headers().get(reqwest::header::LOCATION).and_then(|l| l.to_str().ok()).unwrap_or("");
    Ok(loc.rsplit_once("/releases/tag/").and_then(|(_, t)| {
        let t = urlencoding::decode(t.split(['?', '#']).next().unwrap_or(t)).ok()?.into_owned();
        parse(&t).map(|_| t)
    }))
}

async fn fetch() -> Result<UpdateInfo> {
    let (tag, notes, published_at) = match fetch_api().await {
        Ok(r) => r,
        // The API failed (rate limit, filtered network…): try the web page.
        Err(api_err) => {
            log_warn!("update check: GitHub API failed ({api_err:#}); trying the public releases page");
            match latest_from_redirect().await {
                Ok(tag) => (tag, None, None),
                Err(e) => {
                    log_warn!("update check failed: {e:#}");
                    return Err(api_err);
                }
            }
        }
    };
    let current = current().to_string();
    let url = match &tag {
        Some(t) => format!("https://github.com/{REPO}/releases/tag/{}", urlencoding::encode(t)),
        None => format!("https://github.com/{REPO}/releases"),
    };
    let update_available = tag.as_deref().map_or(false, |t| is_newer(t, &current));
    log_info!("update check: current {current}, latest {}, update available: {update_available}", tag.as_deref().unwrap_or("none"));
    Ok(UpdateInfo {
        current,
        latest: tag.map(|t| t.trim_start_matches(['v', 'V']).to_string()),
        update_available,
        url,
        notes: if update_available { notes } else { None },
        published_at,
    })
}

/// `force` = ignore the one-hour cache ("Check for updates" button).
pub async fn check(force: bool) -> Result<UpdateInfo> {
    if !force {
        if let Some((t, info)) = CACHE.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            if t.elapsed() < CACHE_TTL {
                return Ok(info.clone());
            }
        }
    }
    let info = fetch().await?;
    *CACHE.lock().unwrap_or_else(|e| e.into_inner()) = Some((Instant::now(), info.clone()));
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_versions() {
        assert!(is_newer("v0.3.0", "0.2.0"));
        assert!(is_newer("0.10.0", "0.9.5"));
        assert!(is_newer("1.0", "0.9.9"));
        assert!(is_newer("0.2.0", "0.2.0-beta"));
        assert!(is_newer("v0.2.1", "0.2.0"));
    }

    #[test]
    fn not_newer() {
        assert!(!is_newer("0.2.0", "0.2.0"));
        assert!(!is_newer("v0.2.0", "0.2.0"));
        assert!(!is_newer("0.1.9", "0.2.0"));
        assert!(!is_newer("0.2.0-beta", "0.2.0"));
        assert!(!is_newer("garbage", "0.2.0"));
        assert!(!is_newer("0.3.0", "garbage"));
        assert!(!is_newer("", "0.2.0"));
    }

    #[test]
    fn build_metadata_is_ignored() {
        assert!(!is_newer("0.2.0+build5", "0.2.0"));
        assert!(is_newer("0.2.1+build5", "0.2.0"));
    }

    #[test]
    fn picks_highest_tag() {
        let tags = ["v0.1.0", "v0.10.0", "v0.9.0", "nightly", "v0.2.0-rc1"];
        let best = tags.iter().filter(|t| parse(t).is_some()).max_by(|a, b| cmp_versions(a, b)).unwrap();
        assert_eq!(*best, "v0.10.0");
    }

    #[test]
    fn truncates_on_char_boundary() {
        let s = "é".repeat(2000);
        let t = truncate(&s, 1500);
        assert_eq!(t.chars().count(), 1501);
    }
}
