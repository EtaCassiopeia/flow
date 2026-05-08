use std::path::{Path, PathBuf};
use std::process::Stdio;

use thiserror::Error;
use tokio::process::Command;

use crate::domain::{Session, SessionName};

#[allow(dead_code)]
#[derive(Error, Debug)]
pub enum TmuxError {
    #[error("tmux command failed: {cmd}\n  stderr: {stderr}")]
    Command { cmd: String, stderr: String },
    #[error("tmux output could not be parsed: {0:?}")]
    Parse(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

type Result<T> = std::result::Result<T, TmuxError>;

#[derive(Debug, Clone)]
pub struct Tmux {
    socket_name: Option<String>,
}

impl Default for Tmux {
    fn default() -> Self {
        Self { socket_name: None }
    }
}

impl Tmux {
    pub fn new() -> Self {
        Self::default()
    }

    /// For tests: pin tmux to a private socket via `-L`.
    #[allow(dead_code)]
    pub fn with_socket(name: impl Into<String>) -> Self {
        Self {
            socket_name: Some(name.into()),
        }
    }

    fn cmd(&self) -> Command {
        let mut c = Command::new("tmux");
        if let Some(s) = &self.socket_name {
            c.args(["-L", s]);
        }
        c
    }

    /// True iff `$TMUX` indicates we are inside a tmux client.
    pub fn inside_session() -> bool {
        std::env::var_os("TMUX").is_some()
    }

    pub async fn has_session(&self, name: &SessionName) -> Result<bool> {
        let output = self
            .cmd()
            .args(["has-session", "-t", name.as_str()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .await?;
        Ok(output.status.success())
    }

    pub async fn new_session_detached(&self, name: &SessionName, cwd: &Path) -> Result<()> {
        let output = self
            .cmd()
            .args([
                "new-session",
                "-d",
                "-s",
                name.as_str(),
                "-c",
                &cwd.to_string_lossy(),
            ])
            .output()
            .await?;
        if output.status.success() {
            Ok(())
        } else {
            Err(TmuxError::Command {
                cmd: format!("tmux new-session -d -s {name}"),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            })
        }
    }

    pub async fn kill_session(&self, name: &SessionName) -> Result<()> {
        let output = self
            .cmd()
            .args(["kill-session", "-t", name.as_str()])
            .output()
            .await?;
        if output.status.success() {
            Ok(())
        } else {
            Err(TmuxError::Command {
                cmd: format!("tmux kill-session -t {name}"),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            })
        }
    }

    #[allow(dead_code)]
    pub async fn list_sessions(&self) -> Result<Vec<Session>> {
        let output = self
            .cmd()
            .args([
                "list-sessions",
                "-F",
                "#{session_name}\t#{session_attached}\t#{session_path}\t#{session_windows}",
            ])
            .output()
            .await?;
        if !output.status.success() {
            // No server running = no sessions.
            return Ok(Vec::new());
        }
        let raw = String::from_utf8_lossy(&output.stdout);
        parse_sessions(&raw)
    }

    /// Switch the *currently attached* tmux client to `name`. Use this when we're
    /// already running inside tmux. Returns immediately.
    pub async fn switch_client(&self, name: &SessionName) -> Result<()> {
        let output = self
            .cmd()
            .args(["switch-client", "-t", name.as_str()])
            .output()
            .await?;
        if output.status.success() {
            Ok(())
        } else {
            Err(TmuxError::Command {
                cmd: format!("tmux switch-client -t {name}"),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            })
        }
    }

    /// Build the synchronous `tmux attach` command — caller is responsible for
    /// running it on the foreground terminal (after dropping raw mode etc).
    pub fn attach_command(&self, name: &SessionName) -> std::process::Command {
        let mut c = std::process::Command::new("tmux");
        if let Some(s) = &self.socket_name {
            c.args(["-L", s]);
        }
        c.args(["attach-session", "-t", name.as_str()]);
        c
    }
}

fn parse_sessions(raw: &str) -> Result<Vec<Session>> {
    let mut out = Vec::new();
    for line in raw.lines() {
        if line.is_empty() {
            continue;
        }
        let mut fields = line.split('\t');
        let name = fields.next().ok_or_else(|| TmuxError::Parse(line.into()))?;
        let attached_raw = fields.next().ok_or_else(|| TmuxError::Parse(line.into()))?;
        let cwd = fields.next().ok_or_else(|| TmuxError::Parse(line.into()))?;
        let windows = fields.next().ok_or_else(|| TmuxError::Parse(line.into()))?;
        let session_name = SessionName::new(name).map_err(|_| TmuxError::Parse(line.into()))?;
        out.push(Session {
            name: session_name,
            attached: attached_raw != "0",
            cwd: PathBuf::from(cwd),
            windows: windows.parse().unwrap_or(0),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_session_lines() {
        let raw = "alpha\t1\t/home/u/repo\t3\nbeta\t0\t/tmp\t1\n";
        let v = parse_sessions(raw).unwrap();
        assert_eq!(v.len(), 2);
        assert!(v[0].attached);
        assert_eq!(v[0].windows, 3);
        assert!(!v[1].attached);
    }

    #[tokio::test]
    #[ignore = "requires tmux on PATH; run with `cargo test -- --ignored`"]
    async fn create_kill_roundtrip() {
        let socket = format!("flow-test-{}", std::process::id());
        let tmux = Tmux::with_socket(&socket);
        let tmp = tempfile::tempdir().unwrap();
        let name = SessionName::new("flow_it_test").unwrap();

        assert!(!tmux.has_session(&name).await.unwrap());
        tmux.new_session_detached(&name, tmp.path()).await.unwrap();
        assert!(tmux.has_session(&name).await.unwrap());

        let listed = tmux.list_sessions().await.unwrap();
        assert!(listed.iter().any(|s| s.name == name));

        tmux.kill_session(&name).await.unwrap();
        assert!(!tmux.has_session(&name).await.unwrap());
    }
}
