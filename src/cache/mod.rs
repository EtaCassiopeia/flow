//! Cross-screen cache for fetched data, with optional on-disk snapshot under
//! `~/.cache/flow/snapshot.json`. Singletons (tickets/repos/worktrees) live as
//! `Option<CacheEntry<V>>`; per-key caches (PRs by repo) live as plain
//! `HashMap<K, CacheEntry<V>>`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::{Pr, RepoFullName, Repo, Ticket, Worktree};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry<V> {
    pub value: V,
    pub fetched_at: DateTime<Utc>,
}

impl<V> CacheEntry<V> {
    pub fn new(value: V) -> Self {
        Self {
            value,
            fetched_at: Utc::now(),
        }
    }

    pub fn is_fresh(&self, ttl: Duration) -> bool {
        let age = Utc::now().signed_duration_since(self.fetched_at);
        match age.to_std() {
            Ok(d) => d < ttl,
            Err(_) => true, // negative age = clock skew; treat as fresh
        }
    }
}

/// Default TTLs from the implementation plan.
pub const TICKETS_TTL: Duration = Duration::from_secs(5 * 60);
pub const PRS_TTL: Duration = Duration::from_secs(2 * 60);
pub const REPOS_TTL: Duration = Duration::from_secs(24 * 60 * 60);
pub const WORKTREES_TTL: Duration = Duration::from_secs(30);

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Snapshot {
    #[serde(default)]
    pub tickets: Option<CacheEntry<Vec<Ticket>>>,
    #[serde(default)]
    pub repos: Option<CacheEntry<Vec<Repo>>>,
    #[serde(default)]
    pub worktrees: Option<CacheEntry<Vec<Worktree>>>,
    #[serde(default)]
    pub prs: HashMap<RepoFullName, CacheEntry<Vec<Pr>>>,
}

pub fn snapshot_path() -> PathBuf {
    cache_dir().join("snapshot.json")
}

pub fn cache_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            home.join(".cache")
        });
    base.join("flow")
}

/// Best-effort load. Missing file or malformed JSON yields `Snapshot::default()`.
pub fn load() -> Snapshot {
    let path = snapshot_path();
    let raw = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return Snapshot::default(),
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn save(snapshot: &Snapshot) -> std::io::Result<()> {
    let dir = cache_dir();
    std::fs::create_dir_all(&dir)?;
    let raw = serde_json::to_string(snapshot)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(snapshot_path(), raw)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{BranchName, PrNumber, PrState, RepoFullName};
    use chrono::Utc as ChronoUtc;
    use url::Url;

    #[test]
    fn entry_freshness() {
        let e = CacheEntry::new(42_u32);
        assert!(e.is_fresh(Duration::from_secs(60)));
    }

    #[test]
    fn entry_stale_after_ttl() {
        let mut e = CacheEntry::new(42_u32);
        e.fetched_at = Utc::now() - chrono::Duration::seconds(120);
        assert!(!e.is_fresh(Duration::from_secs(60)));
    }

    #[test]
    fn snapshot_roundtrip() {
        let snap = Snapshot {
            tickets: Some(CacheEntry::new(vec![])),
            ..Snapshot::default()
        };
        let raw = serde_json::to_string(&snap).unwrap();
        let back: Snapshot = serde_json::from_str(&raw).unwrap();
        assert!(back.tickets.is_some());
    }

    #[test]
    fn snapshot_roundtrip_with_pr_map() {
        let repo = RepoFullName::new("acme/widget").unwrap();
        let pr = Pr {
            repo: repo.clone(),
            number: PrNumber(7),
            title: "fix bug".into(),
            head_ref: BranchName::new("fix-bug").unwrap(),
            author: "alice".into(),
            draft: false,
            state: PrState::Open,
            url: Url::parse("https://github.com/acme/widget/pull/7").unwrap(),
        };
        let mut snap = Snapshot::default();
        snap.prs.insert(repo.clone(), CacheEntry::new(vec![pr]));
        let raw = serde_json::to_string(&snap).unwrap();
        let back: Snapshot = serde_json::from_str(&raw).unwrap();
        let entry = back.prs.get(&repo).expect("pr cache entry");
        assert_eq!(entry.value.len(), 1);
        assert_eq!(entry.value[0].number, PrNumber(7));
    }

    #[test]
    fn save_and_load_roundtrip_via_disk() {
        let tmp = tempfile::tempdir().unwrap();
        // Override XDG_CACHE_HOME so we don't touch the real ~/.cache.
        // SAFETY: this test is single-threaded relative to the cache module.
        unsafe {
            std::env::set_var("XDG_CACHE_HOME", tmp.path());
        }
        let snap_in = Snapshot {
            tickets: Some(CacheEntry {
                value: Vec::new(),
                fetched_at: ChronoUtc::now(),
            }),
            ..Snapshot::default()
        };
        save(&snap_in).expect("save");
        let snap_out = load();
        assert!(snap_out.tickets.is_some());
        unsafe {
            std::env::remove_var("XDG_CACHE_HOME");
        }
    }
}
