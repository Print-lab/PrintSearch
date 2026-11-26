use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::PathBuf};

/// Saved settings: tokens per site ("thingiverse", "printables", "makerworld"; the other sites have no account)
/// and the browser used to read pages ("auto", an identifier or a path).
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct Config {
    #[serde(default)]
    pub tokens: BTreeMap<String, String>,
    #[serde(default)]
    pub browser: Option<String>,
    /// Interface language ("en", "de" or "fr"), used for the tray icon menu
    #[serde(default)]
    pub lang: Option<String>,
}

fn path() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("dev", "printsearch", "printsearch").context("config folder not found")?;
    fs::create_dir_all(dirs.config_dir())?;
    Ok(dirs.config_dir().join("config.json"))
}

impl Config {
    pub fn token_for(&self, site: crate::providers::Site) -> Option<String> {
        use crate::providers::Site;
        self.tokens.get(site.key()).cloned().or_else(|| match site {
            Site::Thingiverse => std::env::var("THINGIVERSE_APP_TOKEN").ok(),
            _ => None,
        })
    }

    /// Loads the configuration. An unreadable or corrupt file is set aside (`config.json.bad`)
    /// and the application starts with an empty configuration instead of refusing to start.
    pub fn load() -> Self {
        let Ok(p) = path() else { return Self::default() };
        if !p.exists() {
            return Self::default();
        }
        match fs::read_to_string(&p).map_err(anyhow::Error::from).and_then(|t| serde_json::from_str::<Self>(&t).map_err(anyhow::Error::from)) {
            Ok(c) => c,
            Err(e) => {
                let bad = p.with_extension("json.bad");
                let _ = fs::rename(&p, &bad);
                crate::log::log_warn!("config.json unreadable ({e}); moved to {} and starting with defaults", bad.display());
                Self::default()
            }
        }
    }

    /// Atomic write (temporary file, then rename): no half-written config.
    pub fn save(&self) -> Result<()> {
        let p = path()?;
        let tmp = p.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))?; // tokens = secrets
        }
        fs::rename(&tmp, &p)?;
        Ok(())
    }
}
