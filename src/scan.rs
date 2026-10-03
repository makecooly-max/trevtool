//! Предполётная проверка проекта: размеры файлов, секреты, отсутствие .gitignore.
use anyhow::Result;
use ignore::WalkBuilder;
use std::{fs, path::{Path, PathBuf}};

pub const WARN_BYTES: u64 = 50 * 1024 * 1024;
pub const HARD_LIMIT_BYTES: u64 = 100 * 1024 * 1024; // GitHub отклоняет push с такими файлами

#[derive(Default)]
pub struct Report {
    pub files: usize,
    pub total_bytes: u64,
    pub too_big: Vec<(PathBuf, u64)>,
    pub big: Vec<(PathBuf, u64)>,
    pub secrets: Vec<PathBuf>,
}

fn looks_secret(name: &str) -> bool {
    let n = name.to_lowercase();
    n == ".env" || n.starts_with(".env.") && !n.ends_with(".example") && !n.ends_with(".sample")
        || n.starts_with("id_rsa") || n.starts_with("id_ed25519")
        || n.ends_with(".pem") || n.ends_with(".key") || n.ends_with(".p12") || n.ends_with(".pfx")
        || n == "credentials.json" || n == ".npmrc" || n == ".netrc"
}

/// Обход с учётом .gitignore (даже если репозиторий ещё не создан).
pub fn scan(root: &Path) -> Result<Report> {
    let mut r = Report::default();
    let walker = WalkBuilder::new(root)
        .hidden(false)
        .require_git(false)
        .filter_entry(|e| e.file_name() != ".git")
        .build();
    for entry in walker.flatten() {
        let Ok(md) = entry.metadata() else { continue };
        if !md.is_file() {
            continue;
        }
        let rel = entry.path().strip_prefix(root).unwrap_or(entry.path()).to_path_buf();
        r.files += 1;
        r.total_bytes += md.len();
        if md.len() >= HARD_LIMIT_BYTES {
            r.too_big.push((rel.clone(), md.len()));
        } else if md.len() >= WARN_BYTES {
            r.big.push((rel.clone(), md.len()));
        }
        if looks_secret(&entry.file_name().to_string_lossy()) {
            r.secrets.push(rel);
        }
    }
    Ok(r)
}

/// Если .gitignore нет — предлагаем разумный по типу проекта. Возвращает содержимое.
pub fn suggest_gitignore(root: &Path) -> Option<String> {
    if root.join(".gitignore").exists() {
        return None;
    }
    let mut lines: Vec<&str> = vec![".DS_Store", "Thumbs.db", "*.swp", ".idea/", ".vscode/", ".env"];
    let has = |f: &str| root.join(f).exists();
    if has("Cargo.toml") { lines.push("/target"); }
    if has("package.json") { lines.extend(["node_modules/", "dist/"]); }
    if has("requirements.txt") || has("pyproject.toml") { lines.extend(["__pycache__/", "*.pyc", ".venv/"]); }
    if has("CMakeLists.txt") { lines.extend(["build/", "cmake-build-*/"]); }
    if has("go.mod") { lines.push("/bin/"); }
    Some(lines.join("\n") + "\n")
}

pub fn write_gitignore(root: &Path, content: &str) -> Result<()> {
    fs::write(root.join(".gitignore"), content)?;
    Ok(())
}

pub fn human(b: u64) -> String {
    let (kb, mb, gb) = (1024.0, 1024.0 * 1024.0, 1024.0 * 1024.0 * 1024.0);
    let f = b as f64;
    if f >= gb { format!("{:.1} ГБ", f / gb) } else if f >= mb { format!("{:.1} МБ", f / mb) } else { format!("{:.0} КБ", f / kb) }
}
