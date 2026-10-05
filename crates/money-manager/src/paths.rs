//! Where this installation keeps its files.
//!
//! Everything is private to the application: the XDG base directories on a
//! desktop, and the directory the system gives an application on a phone
//! (`android_main` points the XDG variables at it). Nothing is ever read from
//! or written to shared storage, so there is no permission to ask for.
//!
//! Both directories are named after [`crate::APP_ID`] and must stay
//! byte-identical between versions, or an upgrade loses the ledger.

use std::path::PathBuf;
use std::sync::OnceLock;

/// The rule itself, apart from the environment so it can be tested: a set,
/// non-empty variable wins, and anything else falls back under the home
/// directory.
fn resolve_from(variable: Option<PathBuf>, home: Option<PathBuf>, fallback: &str) -> PathBuf {
    match variable {
        Some(dir) if !dir.as_os_str().is_empty() => dir,
        _ => home.unwrap_or_else(|| PathBuf::from("/")).join(fallback),
    }
}

fn resolve(variable: &str, fallback: &str) -> PathBuf {
    resolve_from(
        std::env::var_os(variable).map(PathBuf::from),
        dirs::home_dir(),
        fallback,
    )
}

/// `settings.json`: what is about this installation rather than the ledger.
pub fn settings_file() -> PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| resolve("XDG_CONFIG_HOME", ".config"))
        .join(crate::APP_ID)
        .join("settings.json")
}

/// The chunk directory of the ledger: sealed chunks and `staging.rdx`, laid
/// out exactly as the terminal version's `money_manager.chunks`.
pub fn ledger_dir() -> PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| resolve("XDG_DATA_HOME", ".local/share"))
        .join(crate::APP_ID)
        .join("ledger")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_the_variable_and_falls_back_under_home() {
        let home = Some(PathBuf::from("/home/someone"));
        assert_eq!(
            resolve_from(Some("/elsewhere/config".into()), home.clone(), ".config"),
            PathBuf::from("/elsewhere/config"),
        );
        // Unset and empty mean the same thing, as the XDG spec says.
        assert_eq!(
            resolve_from(None, home.clone(), ".config"),
            PathBuf::from("/home/someone/.config"),
        );
        assert_eq!(
            resolve_from(Some(PathBuf::new()), home, ".local/share"),
            PathBuf::from("/home/someone/.local/share"),
        );
    }
}
