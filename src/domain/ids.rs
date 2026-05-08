use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IdError {
    #[error("ticket key must look like ABC-123, got {0:?}")]
    InvalidTicketKey(String),
    #[error("repo full name must be 'owner/repo', got {0:?}")]
    InvalidRepoFullName(String),
    #[error("branch name is empty or contains forbidden characters")]
    InvalidBranchName,
    #[error("tmux session name is empty or contains '.' / ':' / whitespace")]
    InvalidSessionName,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TicketKey(String);

impl TicketKey {
    pub fn new(s: impl Into<String>) -> Result<Self, IdError> {
        let s = s.into();
        if Self::is_valid(&s) {
            Ok(Self(s))
        } else {
            Err(IdError::InvalidTicketKey(s))
        }
    }

    fn is_valid(s: &str) -> bool {
        let Some((proj, num)) = s.split_once('-') else {
            return false;
        };
        if proj.is_empty() || num.is_empty() {
            return false;
        }
        let mut chars = proj.chars();
        let first = chars.next().unwrap_or(' ');
        if !first.is_ascii_uppercase() {
            return false;
        }
        if !chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) {
            return false;
        }
        num.chars().all(|c| c.is_ascii_digit())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TicketKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RepoFullName(String);

impl RepoFullName {
    pub fn new(s: impl Into<String>) -> Result<Self, IdError> {
        let s = s.into();
        let (owner, repo) = s
            .split_once('/')
            .ok_or_else(|| IdError::InvalidRepoFullName(s.clone()))?;
        if owner.is_empty()
            || repo.is_empty()
            || owner.contains('/')
            || repo.contains('/')
            || owner.chars().any(char::is_whitespace)
            || repo.chars().any(char::is_whitespace)
        {
            return Err(IdError::InvalidRepoFullName(s));
        }
        Ok(Self(s))
    }

    pub fn owner(&self) -> &str {
        self.0.split_once('/').map(|(o, _)| o).unwrap_or("")
    }

    pub fn repo(&self) -> &str {
        self.0.split_once('/').map(|(_, r)| r).unwrap_or("")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RepoFullName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BranchName(String);

impl BranchName {
    pub fn new(s: impl Into<String>) -> Result<Self, IdError> {
        let s = s.into();
        if s.is_empty()
            || s.starts_with('-')
            || s.contains("..")
            || s.contains(' ')
            || s.contains('~')
            || s.contains('^')
            || s.contains(':')
            || s.contains('?')
            || s.contains('*')
            || s.contains('[')
            || s.ends_with('/')
            || s.ends_with('.')
        {
            return Err(IdError::InvalidBranchName);
        }
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BranchName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionName(String);

impl SessionName {
    pub fn new(s: impl Into<String>) -> Result<Self, IdError> {
        let s = s.into();
        if s.is_empty() || s.contains('.') || s.contains(':') || s.chars().any(char::is_whitespace)
        {
            return Err(IdError::InvalidSessionName);
        }
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SessionName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PrNumber(pub u64);

impl fmt::Display for PrNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticket_key_accepts_canonical() {
        assert!(TicketKey::new("PFC-1234").is_ok());
        assert!(TicketKey::new("ABC1-9").is_ok());
    }

    #[test]
    fn ticket_key_rejects_garbage() {
        assert!(TicketKey::new("").is_err());
        assert!(TicketKey::new("pfc-123").is_err());
        assert!(TicketKey::new("PFC-").is_err());
        assert!(TicketKey::new("-123").is_err());
        assert!(TicketKey::new("PFC-12a").is_err());
    }

    #[test]
    fn repo_full_name_parses_owner_repo() {
        let r = RepoFullName::new("acme/widget").unwrap();
        assert_eq!(r.owner(), "acme");
        assert_eq!(r.repo(), "widget");
    }

    #[test]
    fn repo_full_name_rejects_garbage() {
        assert!(RepoFullName::new("acme").is_err());
        assert!(RepoFullName::new("/repo").is_err());
        assert!(RepoFullName::new("acme/").is_err());
        assert!(RepoFullName::new("acme/wid get").is_err());
    }

    #[test]
    fn branch_name_rejects_forbidden_chars() {
        assert!(BranchName::new("PFC-1-fix").is_ok());
        assert!(BranchName::new("").is_err());
        assert!(BranchName::new("foo bar").is_err());
        assert!(BranchName::new("foo..bar").is_err());
        assert!(BranchName::new("foo:bar").is_err());
        assert!(BranchName::new("foo/").is_err());
    }

    #[test]
    fn session_name_rejects_dot_colon_space() {
        assert!(SessionName::new("PFC-1234").is_ok());
        assert!(SessionName::new("hello.world").is_err());
        assert!(SessionName::new("a b").is_err());
        assert!(SessionName::new("x:y").is_err());
    }
}
