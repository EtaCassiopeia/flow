mod app;
mod cache;
mod config;
mod domain;
mod error;
mod fixtures;
mod msg;
mod runtime;
mod services;
mod ui;

use std::sync::Arc;

use anyhow::Context;
use clap::{Parser, Subcommand};

use crate::app::AppState;
use crate::services::github::GithubClient;
use crate::services::jira::JiraClient;
use crate::ui::Tui;
use crate::ui::screens::Screen;

#[derive(Parser, Debug)]
#[command(name = "flow", version, about = "your worktree-and-tmux remote control")]
struct Cli {
    /// Run with in-memory mocks (no network or filesystem side-effects).
    #[arg(long)]
    mock: bool,

    /// Logging level: error|warn|info|debug|trace.
    #[arg(long, default_value = "info")]
    log_level: String,

    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Print resolved config, keychain entry presence, and tool versions.
    Doctor,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    init_tracing(&cli.log_level);

    if let Some(Cmd::Doctor) = cli.cmd {
        return doctor().await;
    }

    let cfg_path = config::default_path();
    let (config, screen) = if cli.mock {
        (config::Config::sample(), Screen::Dashboard)
    } else {
        match config::load(&cfg_path) {
            Ok(cfg) => (cfg, Screen::Dashboard),
            Err(config::ConfigError::NotFound(_)) => (
                config::Config::sample(),
                Screen::Setup(Box::new(ui::screens::setup::initial())),
            ),
            Err(e) => return Err(anyhow::anyhow!(e)).context("loading config"),
        }
    };

    let (jira, github): (Arc<dyn JiraClient>, Arc<dyn GithubClient>) = if cli.mock {
        let (j, g) = fixtures::mocks();
        (Arc::new(j), Arc::new(g))
    } else {
        let token = services::keychain::get(services::keychain::Slot::JiraToken {
            email: &config.jira.email,
        })
        .unwrap_or_default();
        let jira = services::jira::ReqwestJira::new(
            &config.jira.host,
            config.jira.email.clone(),
            token,
        )
        .context("building jira client")?;
        let (backend, github) = services::github::detect().await;
        tracing::info!(backend = backend.label(), "github backend selected");
        (Arc::new(jira), github)
    };

    ui::install_panic_hook();
    let tui = Tui::enter().context("entering alt screen")?;
    let app_state = AppState::new(config, jira, github, screen);
    let result = app::run(tui, app_state).await;
    result.map_err(anyhow::Error::from)
}

fn init_tracing(level: &str) {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(format!("flow={level}")))
        .unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}

async fn doctor() -> anyhow::Result<()> {
    println!("flow doctor");
    let cfg_path = config::default_path();
    println!("  config path : {}", cfg_path.display());
    println!("  config found: {}", cfg_path.exists());
    if let Ok(cfg) = config::load(&cfg_path) {
        println!("  jira host   : {}", cfg.jira.host);
        println!("  jira email  : {}", cfg.jira.email);
        println!("  code root   : {}", cfg.code_root);
        let jira_present = services::keychain::has(services::keychain::Slot::JiraToken {
            email: &cfg.jira.email,
        });
        let gh_present = services::keychain::has(services::keychain::Slot::GithubToken);
        println!(
            "  jira token  : {}",
            if jira_present { "present" } else { "missing" }
        );
        println!(
            "  github token: {}",
            if gh_present { "present" } else { "missing" }
        );
    }
    let git = std::process::Command::new("git").arg("--version").output();
    match git {
        Ok(o) => println!(
            "  git         : {}",
            String::from_utf8_lossy(&o.stdout).trim()
        ),
        Err(_) => println!("  git         : not found"),
    }
    let tmux = std::process::Command::new("tmux").arg("-V").output();
    match tmux {
        Ok(o) => println!(
            "  tmux        : {}",
            String::from_utf8_lossy(&o.stdout).trim()
        ),
        Err(_) => println!("  tmux        : not found"),
    }
    let gh = std::process::Command::new("gh").arg("--version").output();
    match gh {
        Ok(o) => println!(
            "  gh          : {}",
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .next()
                .unwrap_or("?")
        ),
        Err(_) => println!("  gh          : not found"),
    }
    let (backend, _client) = services::github::detect().await;
    println!("  github      : backend = {}", backend.label());
    Ok(())
}
