pub mod schema;

use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

pub use schema::Config;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("config file not found at {0}")]
    NotFound(PathBuf),
    #[error("could not read config: {0}")]
    Io(#[from] std::io::Error),
    #[error("could not parse config: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("could not serialize config: {0}")]
    Serialize(#[from] toml::ser::Error),
    #[error("could not expand path {0:?}: {1}")]
    Expand(String, shellexpand::LookupError<std::env::VarError>),
}

type Result<T> = std::result::Result<T, ConfigError>;

/// Default location: `~/.config/flow/config.toml`.
pub fn default_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            home.join(".config")
        });
    base.join("flow").join("config.toml")
}

pub fn load(path: &Path) -> Result<Config> {
    if !path.exists() {
        return Err(ConfigError::NotFound(path.to_path_buf()));
    }
    let raw = fs::read_to_string(path)?;
    let cfg: Config = toml::from_str(&raw)?;
    Ok(cfg)
}

pub fn save(path: &Path, cfg: &Config) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let raw = toml::to_string_pretty(cfg)?;
    fs::write(path, raw)?;
    Ok(())
}

/// Expand `~`/env vars in a path-like config value.
pub fn expand(s: &str) -> Result<PathBuf> {
    let expanded = shellexpand::full(s).map_err(|e| ConfigError::Expand(s.to_string(), e))?;
    Ok(PathBuf::from(expanded.into_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_roundtrip() {
        let cfg = Config::sample();
        let raw = toml::to_string_pretty(&cfg).unwrap();
        let parsed: Config = toml::from_str(&raw).unwrap();
        assert_eq!(parsed.code_root, cfg.code_root);
        assert_eq!(parsed.jira.host, cfg.jira.host);
    }
}
