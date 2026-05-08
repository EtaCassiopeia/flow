use keyring::Entry;

pub const SERVICE: &str = "flow";

pub enum Slot<'a> {
    JiraToken { email: &'a str },
    GithubToken,
}

impl<'a> Slot<'a> {
    fn account(&self) -> String {
        match self {
            Slot::JiraToken { email } => format!("jira:{email}"),
            Slot::GithubToken => "github".to_string(),
        }
    }
}

fn entry(slot: Slot<'_>) -> Result<Entry, keyring::Error> {
    Entry::new(SERVICE, &slot.account())
}

pub fn get(slot: Slot<'_>) -> Result<String, keyring::Error> {
    entry(slot)?.get_password()
}

pub fn set(slot: Slot<'_>, value: &str) -> Result<(), keyring::Error> {
    entry(slot)?.set_password(value)
}

#[allow(dead_code)]
pub fn delete(slot: Slot<'_>) -> Result<(), keyring::Error> {
    entry(slot)?.delete_credential()
}

/// Best-effort presence check (returns false on any error).
pub fn has(slot: Slot<'_>) -> bool {
    get(slot).is_ok()
}
