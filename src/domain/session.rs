use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::ids::SessionName;

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub name: SessionName,
    pub attached: bool,
    pub cwd: PathBuf,
    pub windows: u32,
}
