use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub code_root: String,
    #[serde(default)]
    pub default_org: Option<String>,
    pub jira: JiraConfig,
    #[serde(default)]
    pub github: GithubConfig,
    #[serde(default)]
    pub tmux: TmuxConfig,
    #[serde(default)]
    pub ui: UiConfig,
}

impl Config {
    pub fn sample() -> Self {
        Self {
            code_root: "~/code".into(),
            default_org: Some("pfc".into()),
            jira: JiraConfig {
                host: "company.atlassian.net".into(),
                email: "you@example.com".into(),
                jql_my_open: default_jql(),
            },
            github: GithubConfig::default(),
            tmux: TmuxConfig::default(),
            ui: UiConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraConfig {
    pub host: String,
    pub email: String,
    #[serde(default = "default_jql")]
    pub jql_my_open: String,
}

fn default_jql() -> String {
    "assignee = currentUser() AND statusCategory != Done ORDER BY updated DESC".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GithubConfig {
    #[serde(default = "yes")]
    pub auto_discover: bool,
    #[serde(default)]
    pub watched: Vec<String>,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmuxConfig {
    #[serde(default = "default_session_template")]
    pub session_template: String,
    #[serde(default = "yes")]
    pub kill_on_remove_worktree: bool,
}

impl Default for TmuxConfig {
    fn default() -> Self {
        Self {
            session_template: default_session_template(),
            kill_on_remove_worktree: true,
        }
    }
}

fn default_session_template() -> String {
    "{ticket}".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default)]
    pub refresh_interval_secs: u64,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            refresh_interval_secs: 0,
        }
    }
}

fn default_theme() -> String {
    "dark".into()
}
