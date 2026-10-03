mod config;
mod github;
mod gitops;
mod scan;
mod upload;

use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use inquire::{Password, PasswordDisplayMode, Select};
use std::{io::IsTerminal, path::PathBuf};

#[derive(Parser)]
#[command(name = "trevtool", version, about = "Мульти-утилита: GitHub и не только — без браузера")]
struct Cli {
    /// Загрузить проект на GitHub (без значения — текущая папка)
    #[arg(short = 'g', long = "github", value_name = "PATH", num_args = 0..=1, default_missing_value = ".")]
    github: Option<PathBuf>,

    /// Имя репозитория (по умолчанию — имя папки)
    #[arg(short, long, requires = "github")]
    name: Option<String>,

    /// Описание репозитория
    #[arg(short, long, requires = "github")]
    description: Option<String>,

    /// Публичный репозиторий (по умолчанию приватный)
    #[arg(long, requires = "github")]
    public: bool,

    /// Не задавать вопросов
    #[arg(short = 'y', long, requires = "github")]
    yes: bool,

    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Сохранить токен GitHub
    Auth {
        #[arg(long)]
        token: Option<String>,
    },
    /// Удалить сохранённый токен
    Logout,
}

fn auth(token: Option<String>) -> Result<()> {
    let token = match token {
        Some(t) => t,
        None => {
            println!("Создайте токен: https://github.com/settings/tokens");
            println!("(classic — scope `repo`; fine-grained — доступ к созданию репозиториев и Contents: write)");
            Password::new("Токен GitHub:")
                .without_confirmation()
                .with_display_mode(PasswordDisplayMode::Masked)
                .prompt()?
        }
    };
    let gh = github::GitHub::new(token.trim().to_string())?;
    let me = gh.me()?;
    let mut cfg = config::Config::load()?;
    cfg.github_token = Some(gh.token.clone());
    cfg.save()?;
    println!("✔ вход выполнен как {}", me.login);
    Ok(())
}

fn menu() -> Result<()> {
    loop {
        let items = vec!["Загрузить проект на GitHub", "Авторизация GitHub", "Выход"];
        match Select::new("trevtool — что делаем?", items).prompt()? {
            "Загрузить проект на GitHub" => {
                if let Err(e) = upload::run(upload::Opts { ask: true, ..Default::default() }) {
                    eprintln!("✘ {e:#}");
                }
            }
            "Авторизация GitHub" => {
                if let Err(e) = auth(None) {
                    eprintln!("✘ {e:#}");
                }
            }
            _ => return Ok(()),
        }
        println!();
    }
}

fn main() {
    let cli = Cli::parse();
    let res = match (cli.cmd, cli.github) {
        (Some(Cmd::Auth { token }), _) => auth(token),
        (Some(Cmd::Logout), _) => config::Config::default().save().map(|_| println!("✔ токен удалён")),
        (None, Some(path)) => upload::run(upload::Opts {
            path: Some(path),
            name: cli.name,
            description: cli.description,
            public: cli.public,
            yes: cli.yes,
            ask: false,
        }),
        (None, None) if std::io::stdin().is_terminal() => menu(),
        (None, None) => Cli::command().print_help().map_err(Into::into),
    };
    if let Err(e) = res {
        eprintln!("✘ {e:#}");
        std::process::exit(1);
    }
}
