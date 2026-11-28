use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::PathBuf};

/// Saved settings: credentials per site ("thingiverse", "printables", "makerworld", "cults3d" as "user:key", "myminifactory")
/// and the browser used to read pages ("auto", an identifier or a path).
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct Config {
    #[serde(default)]
    pub tokens: BTreeMap<String, String>,
    #[serde(default)]
    pub browser: Option<String>,
    /// Interface language ("en", "fr" or "de"), used for the tray icon menu
    #[serde(default)]
    pub lang: Option<String>,
}

fn path() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("dev", "printsearch", "printsearch").context("dossier de config introuvable")?;
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

    /// Loads the configuration. An unreadable or corrupted file is set aside (`config.json.bad`)
    /// and the application starts with an empty configuration instead of refusing to start.
    /// Replaces every stored secret found in `msg` by `***`. Error messages must never reveal a token or an API key,
    /// for example one that a library copied from a request address into its own error text.
    pub fn scrub(&self, msg: &str) -> String {
        let mut out = msg.to_string();
        let env = std::env::var("THINGIVERSE_APP_TOKEN").unwrap_or_default();
        for token in self.tokens.values().map(String::as_str).chain(std::iter::once(env.as_str())) {
            // the whole value, then its parts ("user:key" for Cults): each part is a secret too
            for secret in std::iter::once(token).chain(token.split(':')) {
                if secret.len() >= 4 {
                    out = out.replace(secret, "***");
                }
            }
        }
        out
    }

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

    /// Atomic write (temporary file then rename): no half-written config.
    pub fn save(&self) -> Result<()> {
        let p = path()?;
        let tmp = p.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))?; // tokens are secrets
        }
        fs::rename(&tmp, &p)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrub_hides_every_secret_and_its_parts() {
        let mut c = Config::default();
        c.tokens.insert("myminifactory".into(), "abcd1234".into());
        c.tokens.insert("cults3d".into(), "alice:KEYVALUE".into());
        c.tokens.insert("printables".into(), "xy".into()); // too short to be replaced safely
        let msg = "error sending request for url (https://x/api?q=a&key=abcd1234&page=1) user alice key KEYVALUE full alice:KEYVALUE xy";
        let out = c.scrub(msg);
        assert!(!out.contains("abcd1234") && !out.contains("KEYVALUE") && !out.contains("alice"), "{out}");
        assert!(out.contains("key=***&page=1") && out.ends_with(" xy"), "{out}");
        assert_eq!(Config::default().scrub("nothing to hide"), "nothing to hide");
    }
}
