//! Обёртка над системным `git`. Токен НЕ попадает ни в remote URL, ни в аргументы процесса —
//! только в переменные окружения одного вызова push.
use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::{path::Path, process::Command};

fn run(dir: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .context("не удалось запустить git — он установлен?")?;
    if !out.status.success() {
        bail!("git {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn ok(dir: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn check_git() -> Result<()> {
    run(Path::new("."), &["--version"]).map(|_| ())
}

pub fn is_repo_root(dir: &Path) -> bool {
    dir.join(".git").exists()
}

pub fn init(dir: &Path) -> Result<()> {
    run(dir, &["init", "-b", "main"]).map(|_| ())
}

pub fn has_commits(dir: &Path) -> bool {
    ok(dir, &["rev-parse", "--verify", "HEAD"])
}

/// Коммитит всё, что изменено. Возвращает false, если коммитить нечего.
pub fn commit_all(dir: &Path, msg: &str, login: &str) -> Result<bool> {
    run(dir, &["add", "-A"])?;
    if run(dir, &["status", "--porcelain"])?.is_empty() {
        return Ok(false);
    }
    // Если git-идентичность не настроена — подставляем noreply-адрес GitHub.
    let has_name = !run(dir, &["config", "user.name"]).unwrap_or_default().is_empty();
    let has_mail = !run(dir, &["config", "user.email"]).unwrap_or_default().is_empty();
    let (n, e) = (format!("user.name={login}"), format!("user.email={login}@users.noreply.github.com"));
    let mut args: Vec<&str> = vec![];
    if !has_name {
        args.extend(["-c", n.as_str()]);
    }
    if !has_mail {
        args.extend(["-c", e.as_str()]);
    }
    args.extend(["commit", "-m", msg]);
    run(dir, &args)?;
    Ok(true)
}

pub fn current_branch(dir: &Path) -> Result<String> {
    let b = run(dir, &["branch", "--show-current"])?;
    if b.is_empty() {
        bail!("detached HEAD — переключитесь на ветку (git switch main)");
    }
    Ok(b)
}

pub fn push(dir: &Path, clone_url: &str, branch: &str, token: &str) -> Result<()> {
    if ok(dir, &["remote", "get-url", "origin"]) {
        run(dir, &["remote", "set-url", "origin", clone_url])?;
    } else {
        run(dir, &["remote", "add", "origin", clone_url])?;
    }
    let header = format!("Authorization: Basic {}", STANDARD.encode(format!("x-access-token:{token}")));
    let out = Command::new("git")
        .current_dir(dir)
        .args(["push", "-u", "origin", branch])
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_CONFIG_COUNT", "2")
        .env("GIT_CONFIG_KEY_0", "http.extraheader")
        .env("GIT_CONFIG_VALUE_0", header)
        .env("GIT_CONFIG_KEY_1", "credential.helper")
        .env("GIT_CONFIG_VALUE_1", "") // отключаем чужие credential-helper'ы
        .output()
        .context("не удалось запустить git push")?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        bail!("push не удался:\n{}", err.trim());
    }
    Ok(())
}
