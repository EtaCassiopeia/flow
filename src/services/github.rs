use std::sync::Arc;
use std::sync::Mutex;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use thiserror::Error;
use tokio::process::Command;
use url::Url;

use crate::domain::{BranchName, Pr, PrNumber, PrState, RepoFullName};

#[allow(dead_code)]
#[derive(Error, Debug)]
pub enum GithubError {
    #[error("`gh` CLI not found on PATH")]
    GhNotInstalled,
    #[error("`gh` is not authenticated; run `gh auth login` first")]
    NotAuthenticated,
    #[error("gh failed: {cmd}\n  stderr: {stderr}")]
    Command { cmd: String, stderr: String },
    #[error("could not parse gh output: {0}")]
    Parse(String),
    #[error("octocrab: {0}")]
    Octocrab(String),
    #[error("git error during PR checkout: {0}")]
    Git(#[from] crate::services::git::GitError),
    #[error("github access not configured: {0}")]
    NotConfigured(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

type Result<T> = std::result::Result<T, GithubError>;

#[derive(Debug, Clone)]
pub struct RepoSummary {
    pub full_name: RepoFullName,
    pub default_branch: BranchName,
    pub pushed_at: DateTime<Utc>,
}

#[async_trait]
pub trait GithubClient: Send + Sync {
    async fn list_my_repos(&self, limit: usize) -> Result<Vec<RepoSummary>>;
    async fn list_open_prs(&self, repo: &RepoFullName) -> Result<Vec<Pr>>;
    /// Run `gh pr checkout <num>` inside the given worktree; returns when done.
    async fn checkout_pr(&self, worktree: &std::path::Path, number: u64) -> Result<()>;
}

pub struct GhCli;

impl GhCli {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GhCli {
    fn default() -> Self {
        Self
    }
}

async fn run_gh(args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("gh").args(args).output().await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            GithubError::GhNotInstalled
        } else {
            GithubError::Io(e)
        }
    })?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        if stderr.contains("not logged into") || stderr.contains("authentication") {
            return Err(GithubError::NotAuthenticated);
        }
        Err(GithubError::Command {
            cmd: format!("gh {}", args.join(" ")),
            stderr,
        })
    }
}

#[async_trait]
impl GithubClient for GhCli {
    async fn list_my_repos(&self, limit: usize) -> Result<Vec<RepoSummary>> {
        let limit_s = limit.to_string();
        let stdout = run_gh(&[
            "repo",
            "list",
            "--limit",
            &limit_s,
            "--json",
            "nameWithOwner,defaultBranchRef,pushedAt",
        ])
        .await?;
        let raw: Vec<RepoRaw> =
            serde_json::from_slice(&stdout).map_err(|e| GithubError::Parse(e.to_string()))?;
        let mut out = Vec::with_capacity(raw.len());
        for r in raw {
            let Ok(full) = RepoFullName::new(&r.name_with_owner) else {
                continue;
            };
            let branch_str = r
                .default_branch_ref
                .map(|b| b.name)
                .unwrap_or_else(|| "main".into());
            let Ok(branch) = BranchName::new(branch_str) else {
                continue;
            };
            let pushed = r
                .pushed_at
                .as_deref()
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(Utc::now);
            out.push(RepoSummary {
                full_name: full,
                default_branch: branch,
                pushed_at: pushed,
            });
        }
        out.sort_by(|a, b| b.pushed_at.cmp(&a.pushed_at));
        Ok(out)
    }

    async fn list_open_prs(&self, repo: &RepoFullName) -> Result<Vec<Pr>> {
        let repo_arg = repo.as_str().to_string();
        let stdout = run_gh(&[
            "pr",
            "list",
            "--repo",
            &repo_arg,
            "--state",
            "open",
            "--json",
            "number,title,headRefName,author,isDraft,state,url",
            "--limit",
            "100",
        ])
        .await?;
        let raw: Vec<PrRaw> =
            serde_json::from_slice(&stdout).map_err(|e| GithubError::Parse(e.to_string()))?;
        let mut out = Vec::with_capacity(raw.len());
        for p in raw {
            let Ok(branch) = BranchName::new(&p.head_ref_name) else {
                continue;
            };
            let Ok(url) = Url::parse(&p.url) else { continue };
            out.push(Pr {
                repo: repo.clone(),
                number: PrNumber(p.number),
                title: p.title,
                head_ref: branch,
                author: p.author.map(|a| a.login).unwrap_or_default(),
                draft: p.is_draft,
                state: parse_state(&p.state),
                url,
            });
        }
        Ok(out)
    }

    async fn checkout_pr(&self, worktree: &std::path::Path, number: u64) -> Result<()> {
        let n = number.to_string();
        let output = Command::new("gh")
            .args(["pr", "checkout", &n])
            .current_dir(worktree)
            .output()
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    GithubError::GhNotInstalled
                } else {
                    GithubError::Io(e)
                }
            })?;
        if !output.status.success() {
            return Err(GithubError::Command {
                cmd: format!("gh pr checkout {n}"),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        Ok(())
    }
}

fn parse_state(s: &str) -> PrState {
    match s.to_ascii_uppercase().as_str() {
        "OPEN" => PrState::Open,
        "MERGED" => PrState::Merged,
        _ => PrState::Closed,
    }
}

#[derive(Deserialize)]
struct RepoRaw {
    #[serde(rename = "nameWithOwner")]
    name_with_owner: String,
    #[serde(default, rename = "defaultBranchRef")]
    default_branch_ref: Option<DefaultBranchRef>,
    #[serde(rename = "pushedAt")]
    pushed_at: Option<String>,
}

#[derive(Deserialize)]
struct DefaultBranchRef {
    name: String,
}

#[derive(Deserialize)]
struct PrRaw {
    number: u64,
    title: String,
    #[serde(rename = "headRefName")]
    head_ref_name: String,
    #[serde(default)]
    author: Option<AuthorRaw>,
    #[serde(rename = "isDraft")]
    is_draft: bool,
    state: String,
    url: String,
}

#[derive(Deserialize)]
struct AuthorRaw {
    #[serde(default)]
    login: String,
}

#[derive(Default, Clone)]
pub struct MockGithub {
    pub repos: Arc<Mutex<Vec<RepoSummary>>>,
    pub prs: Arc<Mutex<Vec<Pr>>>,
}

#[async_trait]
impl GithubClient for MockGithub {
    async fn list_my_repos(&self, limit: usize) -> Result<Vec<RepoSummary>> {
        Ok(self
            .repos
            .lock()
            .expect("mock gh lock")
            .iter()
            .take(limit)
            .cloned()
            .collect())
    }

    async fn list_open_prs(&self, repo: &RepoFullName) -> Result<Vec<Pr>> {
        Ok(self
            .prs
            .lock()
            .expect("mock gh lock")
            .iter()
            .filter(|p| p.repo == *repo)
            .cloned()
            .collect())
    }

    async fn checkout_pr(&self, _worktree: &std::path::Path, _number: u64) -> Result<()> {
        Ok(())
    }
}

// =====================================================================
// Octocrab fallback — used when `gh` isn't installed or isn't authenticated.
// =====================================================================

pub struct OctocrabClient {
    client: octocrab::Octocrab,
}

impl OctocrabClient {
    pub fn new(token: String) -> Result<Self> {
        let client = octocrab::Octocrab::builder()
            .personal_token(token)
            .build()
            .map_err(|e| GithubError::Octocrab(e.to_string()))?;
        Ok(Self { client })
    }
}

#[async_trait]
impl GithubClient for OctocrabClient {
    async fn list_my_repos(&self, limit: usize) -> Result<Vec<RepoSummary>> {
        let per_page = limit.min(100) as u8;
        let page = self
            .client
            .current()
            .list_repos_for_authenticated_user()
            .sort("pushed")
            .per_page(per_page)
            .send()
            .await
            .map_err(|e| GithubError::Octocrab(e.to_string()))?;
        let mut out = Vec::with_capacity(page.items.len());
        for r in page.items {
            let Some(full) = r.full_name else { continue };
            let Ok(full_name) = RepoFullName::new(full) else {
                continue;
            };
            let branch_str = r.default_branch.unwrap_or_else(|| "main".into());
            let Ok(default_branch) = BranchName::new(branch_str) else {
                continue;
            };
            let pushed_at = r.pushed_at.unwrap_or_else(Utc::now);
            out.push(RepoSummary {
                full_name,
                default_branch,
                pushed_at,
            });
        }
        out.sort_by(|a, b| b.pushed_at.cmp(&a.pushed_at));
        Ok(out)
    }

    async fn list_open_prs(&self, repo: &RepoFullName) -> Result<Vec<Pr>> {
        let page = self
            .client
            .pulls(repo.owner(), repo.repo())
            .list()
            .state(octocrab::params::State::Open)
            .per_page(100)
            .send()
            .await
            .map_err(|e| GithubError::Octocrab(e.to_string()))?;
        let mut out = Vec::with_capacity(page.items.len());
        for p in page.items {
            let Ok(head_ref) = BranchName::new(p.head.ref_field.clone()) else {
                continue;
            };
            let state = match p.state {
                octocrab::models::IssueState::Open => PrState::Open,
                octocrab::models::IssueState::Closed if p.merged => PrState::Merged,
                octocrab::models::IssueState::Closed => PrState::Closed,
                _ => PrState::Open,
            };
            out.push(Pr {
                repo: repo.clone(),
                number: PrNumber(p.number),
                title: p.title,
                head_ref,
                author: p.user.login.clone(),
                draft: p.draft.unwrap_or(false),
                state,
                url: p.html_url,
            });
        }
        Ok(out)
    }

    /// Without `gh`, replicate `gh pr checkout` with two git invocations:
    /// fetch `pull/<N>/head` (works for both same-repo and fork PRs because
    /// GitHub mirrors fork PR refs to origin) and `git reset --hard FETCH_HEAD`
    /// to point the freshly-created worktree branch at the PR head.
    async fn checkout_pr(&self, worktree: &std::path::Path, number: u64) -> Result<()> {
        crate::services::git::fetch_pr_head(worktree, number).await?;
        crate::services::git::reset_hard_to_fetch_head(worktree).await?;
        Ok(())
    }
}

// =====================================================================
// Disabled stub — used when neither backend is configured. All methods
// return a clear "how to fix" error that the UI surfaces as a toast.
// =====================================================================

pub struct Disabled {
    pub reason: String,
}

#[async_trait]
impl GithubClient for Disabled {
    async fn list_my_repos(&self, _limit: usize) -> Result<Vec<RepoSummary>> {
        Err(GithubError::NotConfigured(self.reason.clone()))
    }
    async fn list_open_prs(&self, _repo: &RepoFullName) -> Result<Vec<Pr>> {
        Err(GithubError::NotConfigured(self.reason.clone()))
    }
    async fn checkout_pr(&self, _worktree: &std::path::Path, _number: u64) -> Result<()> {
        Err(GithubError::NotConfigured(self.reason.clone()))
    }
}

// =====================================================================
// Backend detection.
// =====================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Gh,
    Octocrab,
    Disabled,
}

impl Backend {
    pub fn label(&self) -> &'static str {
        match self {
            Backend::Gh => "gh",
            Backend::Octocrab => "octocrab",
            Backend::Disabled => "disabled",
        }
    }
}

/// Pick the best available GitHub backend:
/// 1. `gh` if it's on PATH and `gh auth status` succeeds — preferred because
///    it handles fork PRs and refresh tokens transparently.
/// 2. Octocrab with a personal token from the keychain.
/// 3. A `Disabled` stub that surfaces a "how to fix" error on every call.
pub async fn detect() -> (Backend, Arc<dyn GithubClient>) {
    if probe_gh_auth().await {
        return (Backend::Gh, Arc::new(GhCli::new()));
    }
    match crate::services::keychain::get(crate::services::keychain::Slot::GithubToken) {
        Ok(token) if !token.is_empty() => match OctocrabClient::new(token) {
            Ok(c) => return (Backend::Octocrab, Arc::new(c)),
            Err(e) => {
                tracing::warn!(error=%e, "octocrab construction failed");
            }
        },
        _ => {}
    }
    (
        Backend::Disabled,
        Arc::new(Disabled {
            reason: "no `gh` auth found and no GitHub token in keychain — \
                     run `gh auth login` or save a token via setup"
                .into(),
        }),
    )
}

async fn probe_gh_auth() -> bool {
    let result = tokio::process::Command::new("gh")
        .args(["auth", "status"])
        .output()
        .await;
    matches!(result, Ok(o) if o.status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn disabled_backend_surfaces_not_configured() {
        let d = Disabled {
            reason: "test reason".into(),
        };
        let err = d.list_my_repos(10).await.unwrap_err();
        assert!(matches!(err, GithubError::NotConfigured(_)));
        assert!(format!("{err}").contains("test reason"));
    }

    #[tokio::test]
    async fn octocrab_client_constructs_with_arbitrary_token() {
        // Doesn't make any network call; just verifies the builder accepts a
        // token. Octocrab's underlying tower::Buffer needs a tokio reactor.
        let c = OctocrabClient::new("ghp_dummy".into()).expect("octocrab build");
        let _: &octocrab::Octocrab = &c.client;
    }

    #[test]
    fn backend_label_strings() {
        assert_eq!(Backend::Gh.label(), "gh");
        assert_eq!(Backend::Octocrab.label(), "octocrab");
        assert_eq!(Backend::Disabled.label(), "disabled");
    }
}
