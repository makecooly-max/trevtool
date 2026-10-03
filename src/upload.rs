use anyhow::{bail, Context, Result};
use inquire::{Confirm, Select, Text};
use std::{io::IsTerminal, path::PathBuf};

use crate::{config, github::GitHub, gitops, scan};

#[derive(Default)]
pub struct Opts {
    pub path: Option<PathBuf>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub public: bool,
    pub yes: bool,
    /// true — вызвано из меню: спрашиваем все поля. false — из флагов: берём умолчания.
    pub ask: bool,
}

fn sanitize(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '-' })
        .collect();
    let s = s.trim_matches(|c| c == '-' || c == '.').to_string();
    if s.is_empty() { "project".into() } else { s }
}

pub fn run(o: Opts) -> Result<()> {
    let can_ask = std::io::stdin().is_terminal() && !o.yes;

    gitops::check_git()?;
    let gh = GitHub::new(config::github_token()?)?;
    let me = gh.me()?;
    println!("✔ GitHub: {}", me.login);

    // 1. Путь
    let path = match o.path {
        Some(p) => p,
        None => PathBuf::from(Text::new("Путь к проекту:").with_default(".").prompt()?),
    };
    let root = path.canonicalize().with_context(|| format!("путь не найден: {}", path.display()))?;
    if !root.is_dir() {
        bail!("{} — не папка", root.display());
    }

    // 2. .gitignore
    if let Some(gi) = scan::suggest_gitignore(&root) {
        let create = if can_ask {
            Confirm::new("В проекте нет .gitignore. Создать подходящий (target, node_modules, .env…)?")
                .with_default(true)
                .prompt()?
        } else {
            o.yes
        };
        if create {
            scan::write_gitignore(&root, &gi)?;
            println!("✔ создан .gitignore");
        }
    }

    // 3. Проверка содержимого
    let rep = scan::scan(&root)?;
    println!("• файлов к отправке: {} ({})", rep.files, scan::human(rep.total_bytes));
    if rep.files == 0 {
        bail!("в папке нечего отправлять");
    }
    if !rep.too_big.is_empty() {
        eprintln!("✘ файлы ≥100 МБ — GitHub их отклонит:");
        for (p, s) in &rep.too_big {
            eprintln!("    {} ({})", p.display(), scan::human(*s));
        }
        bail!("добавьте их в .gitignore (или используйте Git LFS) и повторите");
    }
    for (p, s) in &rep.big {
        println!("⚠ крупный файл: {} ({})", p.display(), scan::human(*s));
    }
    if !rep.secrets.is_empty() {
        eprintln!("⚠ похоже на секреты:");
        for p in &rep.secrets {
            eprintln!("    {}", p.display());
        }
        if !can_ask {
            bail!("добавьте эти файлы в .gitignore: без подтверждения в терминале секреты не отправляются");
        }
        if !Confirm::new("Всё равно отправить?").with_default(false).prompt()? {
            bail!("отменено");
        }
    }

    // 4. Параметры репозитория
    let default_name = sanitize(&root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
    let name = match o.name {
        Some(n) => sanitize(&n),
        None if o.ask && can_ask => sanitize(&Text::new("Имя репозитория:").with_default(&default_name).prompt()?),
        None => default_name,
    };
    let description = match o.description {
        Some(d) => Some(d),
        None if o.ask && can_ask => {
            let d = Text::new("Описание (Enter — пропустить):").prompt()?;
            (!d.trim().is_empty()).then_some(d)
        }
        None => None,
    };
    let public = if o.public {
        true
    } else if o.ask && can_ask {
        Select::new("Видимость:", vec!["Приватный", "Публичный"]).prompt()? == "Публичный"
    } else {
        false
    };

    if can_ask
        && !Confirm::new(&format!(
            "Отправить в {}/{} ({})?",
            me.login,
            name,
            if public { "публичный" } else { "приватный" }
        ))
        .with_default(true)
        .prompt()?
    {
        bail!("отменено");
    }

    // 5. Репозиторий на GitHub
    let repo = match gh.get_repo(&me.login, &name)? {
        Some(r) => {
            if !can_ask {
                bail!("репозиторий {}/{} уже существует — укажите другое имя: --name", me.login, name);
            }
            if !Confirm::new("Такой репозиторий уже есть. Отправить в него?").with_default(false).prompt()? {
                bail!("отменено");
            }
            r
        }
        None => gh.create_repo(&name, description.as_deref(), !public)?,
    };
    println!("✔ репозиторий: {}", repo.html_url);

    // 6. Git
    if !gitops::is_repo_root(&root) {
        gitops::init(&root)?;
    }
    let msg = if gitops::has_commits(&root) { "Update via trevtool" } else { "Initial commit" };
    gitops::commit_all(&root, msg, &me.login)?;
    if !gitops::has_commits(&root) {
        bail!("нечего коммитить");
    }
    let branch = gitops::current_branch(&root)?;
    println!("… отправка ({branch})");
    gitops::push(&root, &repo.clone_url, &branch, &gh.token)?;

    println!("✔ готово: {}", repo.html_url);
    Ok(())
}
