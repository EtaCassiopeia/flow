use std::path::{Path, PathBuf};
use std::process::Stdio;

use thiserror::Error;
use tokio::process::Command;

use crate::domain::{BranchName, RepoFullName, Worktree, WorktreeStatus};

#[allow(dead_code)]
#[derive(Error, Debug)]
pub enum GitError {
    #[error("git command failed: {cmd}\n  stderr: {stderr}")]
    Command { cmd: String, stderr: String },
    #[error("worktree path already exists: {0}")]
    PathExists(PathBuf),
    #[error("branch already exists: {0}")]
    BranchExists(String),
    #[error("worktree-list output could not be parsed")]
    Parse,
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

type Result<T> = std::result::Result<T, GitError>;

/// Run a git command in `cwd`. Returns stdout on success.
async fn run(cwd: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .output()
        .await?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        Err(GitError::Command {
            cmd: format!("git {}", args.join(" ")),
            stderr,
        })
    }
}

/// Create a worktree at `path` on `branch`, optionally creating it from `start_point`.
///
/// If `branch` already exists locally we just check it out into the new worktree
/// (no `-b`); otherwise we create it fresh from `start_point`.
pub async fn create_worktree(
    repo: &Path,
    path: &Path,
    branch: &BranchName,
    start_point: Option<&BranchName>,
) -> Result<()> {
    if path.exists() {
        return Err(GitError::PathExists(path.to_path_buf()));
    }
    let path_s = path.to_string_lossy();
    let branch_exists = run(repo, &["rev-parse", "--verify", branch.as_str()])
        .await
        .is_ok();
    let mut args: Vec<&str> = vec!["worktree", "add"];
    if branch_exists {
        args.push(&path_s);
        args.push(branch.as_str());
    } else {
        args.push("-b");
        args.push(branch.as_str());
        args.push(&path_s);
        if let Some(sp) = start_point {
            args.push(sp.as_str());
        }
    }
    run(repo, &args).await.map(|_| ())
}

pub async fn remove_worktree(repo: &Path, worktree_path: &Path, force: bool) -> Result<()> {
    let path_s = worktree_path.to_string_lossy();
    let mut args: Vec<&str> = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    args.push(&path_s);
    run(repo, &args).await.map(|_| ())
}

pub async fn delete_branch(repo: &Path, branch: &BranchName, force: bool) -> Result<()> {
    let flag = if force { "-D" } else { "-d" };
    run(repo, &["branch", flag, branch.as_str()])
        .await
        .map(|_| ())
}

#[allow(dead_code)]
pub async fn default_branch(repo: &Path) -> Result<BranchName> {
    let out = run(
        repo,
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    )
    .await
    .ok();
    let name = match out {
        Some(s) => s.trim().rsplit('/').next().unwrap_or("main").to_string(),
        None => "main".to_string(),
    };
    BranchName::new(name).map_err(|_| GitError::Parse)
}

/// Parse `git worktree list --porcelain`.
pub async fn list_worktrees(repo_path: &Path, repo: &RepoFullName) -> Result<Vec<Worktree>> {
    let raw = run(repo_path, &["worktree", "list", "--porcelain"]).await?;
    parse_worktree_list(&raw, repo)
}

fn parse_worktree_list(raw: &str, repo: &RepoFullName) -> Result<Vec<Worktree>> {
    let mut out = Vec::new();
    let mut path: Option<PathBuf> = None;
    let mut branch: Option<String> = None;
    let mut detached = false;
    for line in raw.lines() {
        if line.is_empty() {
            if let Some(p) = path.take() {
                let br = match branch.take() {
                    Some(b) => b
                        .strip_prefix("refs/heads/")
                        .unwrap_or(&b)
                        .to_string(),
                    None if detached => "(detached)".to_string(),
                    None => continue,
                };
                if let Ok(branch_name) = BranchName::new(br) {
                    out.push(Worktree {
                        repo: repo.clone(),
                        path: p,
                        branch: branch_name,
                        ticket: None,
                        status: WorktreeStatus::default(),
                        session: None,
                    });
                }
                detached = false;
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("worktree ") {
            path = Some(PathBuf::from(rest));
        } else if let Some(rest) = line.strip_prefix("branch ") {
            branch = Some(rest.to_string());
        } else if line == "detached" {
            detached = true;
        }
    }
    if let Some(p) = path {
        if let Some(b) = branch {
            let br = b.strip_prefix("refs/heads/").unwrap_or(&b).to_string();
            if let Ok(branch_name) = BranchName::new(br) {
                out.push(Worktree {
                    repo: repo.clone(),
                    path: p,
                    branch: branch_name,
                    ticket: None,
                    status: WorktreeStatus::default(),
                    session: None,
                });
            }
        }
    }
    Ok(out)
}

pub async fn worktree_status(worktree: &Path) -> Result<WorktreeStatus> {
    let porcelain = run(worktree, &["status", "--porcelain"]).await?;
    let dirty = !porcelain.trim().is_empty();
    let upstream = run(
        worktree,
        &["rev-parse", "--abbrev-ref", "@{upstream}"],
    )
    .await;
    let (ahead, behind, has_upstream) = match upstream {
        Ok(_) => {
            let counts = run(
                worktree,
                &["rev-list", "--left-right", "--count", "@{upstream}...HEAD"],
            )
            .await?;
            let mut parts = counts.split_whitespace();
            let behind: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            let ahead: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            (ahead, behind, true)
        }
        Err(_) => (0, 0, false),
    };
    Ok(WorktreeStatus {
        dirty,
        ahead,
        behind,
        has_upstream,
    })
}

#[allow(dead_code)]
pub async fn fetch(repo: &Path) -> Result<()> {
    run(repo, &["fetch", "--prune"]).await.map(|_| ())
}

/// Fetch a GitHub PR's head ref into `FETCH_HEAD`. Works for both same-repo
/// and fork PRs because GitHub mirrors fork PR refs to `origin/pull/<N>/head`.
pub async fn fetch_pr_head(repo: &Path, pr_number: u64) -> Result<()> {
    let refspec = format!("pull/{pr_number}/head");
    run(repo, &["fetch", "origin", &refspec]).await.map(|_| ())
}

/// Reset the current branch in `repo` to whatever `FETCH_HEAD` points at.
/// Used after `fetch_pr_head` to make the local branch match the PR head.
pub async fn reset_hard_to_fetch_head(repo: &Path) -> Result<()> {
    run(repo, &["reset", "--hard", "FETCH_HEAD"]).await.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> &'static str {
        "\
worktree /repos/widget
HEAD aaaa
branch refs/heads/main

worktree /repos/widget-PFC-1
HEAD bbbb
branch refs/heads/PFC-1-fix-login

"
    }

    #[test]
    fn parses_porcelain_listing() {
        let repo = RepoFullName::new("acme/widget").unwrap();
        let v = parse_worktree_list(fixture(), &repo).unwrap();
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].branch.as_str(), "main");
        assert_eq!(v[1].branch.as_str(), "PFC-1-fix-login");
    }

    #[tokio::test]
    async fn create_and_list_against_real_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let repo_path = tmp.path().join("repo");
        std::fs::create_dir(&repo_path).unwrap();
        // git init + initial commit
        Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(&repo_path)
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@test"])
            .current_dir(&repo_path)
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "test"])
            .current_dir(&repo_path)
            .output()
            .await
            .unwrap();
        std::fs::write(repo_path.join("README"), "hi").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_path)
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(&repo_path)
            .output()
            .await
            .unwrap();

        let wt_path = tmp.path().join("repo-PFC-1");
        let branch = BranchName::new("PFC-1-fix-login").unwrap();
        create_worktree(&repo_path, &wt_path, &branch, None)
            .await
            .unwrap();
        assert!(wt_path.exists());

        let repo = RepoFullName::new("acme/repo").unwrap();
        let listed = list_worktrees(&repo_path, &repo).await.unwrap();
        assert!(listed.iter().any(|w| w.branch.as_str() == "PFC-1-fix-login"));

        let status = worktree_status(&wt_path).await.unwrap();
        assert!(!status.dirty);
        assert!(!status.has_upstream);

        remove_worktree(&repo_path, &wt_path, true).await.unwrap();
        assert!(!wt_path.exists());
    }
}
