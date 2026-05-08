use std::io;
use thiserror::Error;

use crate::config::ConfigError;
use crate::services::git::GitError;
use crate::services::github::GithubError;
use crate::services::jira::JiraError;
use crate::services::tmux::TmuxError;

pub type Result<T> = std::result::Result<T, Error>;

#[allow(dead_code)]
#[derive(Error, Debug)]
pub enum Error {
    #[error("config: {0}")]
    Config(#[from] ConfigError),
    #[error("jira: {0}")]
    Jira(#[from] JiraError),
    #[error("github: {0}")]
    Github(#[from] GithubError),
    #[error("git: {0}")]
    Git(#[from] GitError),
    #[error("tmux: {0}")]
    Tmux(#[from] TmuxError),
    #[error("keychain: {0}")]
    Keychain(#[from] keyring::Error),
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("cancelled")]
    Cancelled,
}
