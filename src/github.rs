use anyhow::{bail, Context, Result};
use reqwest::{blocking::Client, header, StatusCode};
use serde::Deserialize;
use serde_json::json;

const API: &str = "https://api.github.com";

#[derive(Deserialize)]
pub struct User {
    pub login: String,
}

#[derive(Deserialize)]
pub struct Repo {
    pub clone_url: String,
    pub html_url: String,
}

pub struct GitHub {
    client: Client,
    pub token: String,
}

impl GitHub {
    pub fn new(token: String) -> Result<Self> {
        let mut h = header::HeaderMap::new();
        h.insert(header::ACCEPT, "application/vnd.github+json".parse()?);
        h.insert("X-GitHub-Api-Version", "2022-11-28".parse()?);
        let client = Client::builder()
            .user_agent(concat!("trevtool/", env!("CARGO_PKG_VERSION")))
            .default_headers(h)
            .build()?;
        Ok(Self { client, token })
    }

    pub fn me(&self) -> Result<User> {
        let r = self
            .client
            .get(format!("{API}/user"))
            .bearer_auth(&self.token)
            .send()
            .context("нет связи с api.github.com")?;
        if r.status() == StatusCode::UNAUTHORIZED {
            bail!("токен недействителен или отозван (trevtool auth)");
        }
        Ok(r.error_for_status()?.json()?)
    }

    pub fn get_repo(&self, owner: &str, name: &str) -> Result<Option<Repo>> {
        let r = self
            .client
            .get(format!("{API}/repos/{owner}/{name}"))
            .bearer_auth(&self.token)
            .send()?;
        if r.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        Ok(Some(r.error_for_status()?.json()?))
    }

    /// Создаёт ПУСТОЙ репозиторий (без README/лицензии), чтобы первый push прошёл без конфликтов.
    pub fn create_repo(&self, name: &str, desc: Option<&str>, private: bool) -> Result<Repo> {
        let r = self
            .client
            .post(format!("{API}/user/repos"))
            .bearer_auth(&self.token)
            .json(&json!({ "name": name, "description": desc, "private": private, "auto_init": false }))
            .send()?;
        let status = r.status();
        if status.is_success() {
            return Ok(r.json()?);
        }
        let body = r.text().unwrap_or_default();
        match status {
            StatusCode::FORBIDDEN | StatusCode::NOT_FOUND => bail!(
                "GitHub отказал ({status}). У токена нет права создавать репозитории \
                 (classic: scope `repo`; fine-grained: Administration → Read and write)"
            ),
            _ => bail!("GitHub вернул {status}: {body}"),
        }
    }
}
