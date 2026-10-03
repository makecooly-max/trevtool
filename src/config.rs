use anyhow::{bail, Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Default, Serialize, Deserialize)]
pub struct Config {
    pub github_token: Option<String>,
}

fn config_path() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("", "", "trevtool").context("не найден домашний каталог")?;
    Ok(dirs.config_dir().join("config.toml"))
}

impl Config {
    pub fn load() -> Result<Self> {
        let p = config_path()?;
        if !p.exists() {
            return Ok(Self::default());
        }
        let s = fs::read_to_string(&p).with_context(|| format!("чтение {}", p.display()))?;
        Ok(toml::from_str(&s).unwrap_or_default())
    }

    pub fn save(&self) -> Result<()> {
        let p = config_path()?;
        fs::create_dir_all(p.parent().unwrap())?;
        fs::write(&p, toml::to_string_pretty(self)?)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&p, fs::Permissions::from_mode(0o600))?; // токен — только владельцу
        }
        Ok(())
    }
}

/// Приоритет: переменная окружения → конфиг.
pub fn github_token() -> Result<String> {
    for var in ["TREVTOOL_GITHUB_TOKEN", "GITHUB_TOKEN", "GH_TOKEN"] {
        if let Ok(t) = std::env::var(var) {
            if !t.trim().is_empty() {
                return Ok(t.trim().to_string());
            }
        }
    }
    match Config::load()?.github_token {
        Some(t) if !t.is_empty() => Ok(t),
        _ => bail!("нет токена GitHub. Выполните: trevtool auth"),
    }
}
