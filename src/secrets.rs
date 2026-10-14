//! API keys live in the OS credential store (macOS Keychain, Windows Credential
//! Manager, or the Secret Service on Linux), never in the settings file.

use crate::photos::Keys;

const SERVICE: &str = "magpie";
/// Keychain service used when the app was called Logo Finder; keys are moved out of it on load.
const LEGACY_SERVICE: &str = "logo-finder";
pub const PROVIDERS: [&str; 3] = ["Pexels", "Unsplash", "Pixabay"];

fn entry(provider: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, provider).map_err(|e| e.to_string())
}

pub fn get(provider: &str) -> Result<String, String> {
    match entry(provider)?.get_password() {
        Ok(v) => Ok(v),
        Err(keyring::Error::NoEntry) => migrate_legacy(provider),
        Err(e) => Err(e.to_string()),
    }
}

/// Move a key saved under the old "logo-finder" service to "magpie". Empty if there is none.
fn migrate_legacy(provider: &str) -> Result<String, String> {
    let legacy = keyring::Entry::new(LEGACY_SERVICE, provider).map_err(|e| e.to_string())?;
    let value = match legacy.get_password() {
        Ok(v) => v,
        Err(keyring::Error::NoEntry) => return Ok(String::new()),
        Err(e) => return Err(e.to_string()),
    };
    set(provider, &value)?;
    let _ = legacy.delete_credential();
    Ok(value)
}

/// Store a key; an empty value deletes it.
pub fn set(provider: &str, value: &str) -> Result<(), String> {
    let entry = entry(provider)?;
    let value = value.trim();
    if value.is_empty() {
        return match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        };
    }
    entry.set_password(value).map_err(|e| e.to_string())
}

pub fn key_of<'a>(keys: &'a Keys, provider: &str) -> &'a str {
    match provider {
        "Pexels" => &keys.pexels,
        "Unsplash" => &keys.unsplash,
        _ => &keys.pixabay,
    }
}

/// Load every key. Errors (e.g. the user denied Keychain access) leave that key empty.
pub fn load() -> (Keys, Vec<String>) {
    let mut errors = Vec::new();
    let mut read = |p: &str| {
        get(p).unwrap_or_else(|e| {
            errors.push(format!("{p}: {e}"));
            String::new()
        })
    };
    let keys = Keys {
        pexels: read("Pexels"),
        unsplash: read("Unsplash"),
        pixabay: read("Pixabay"),
    };
    (keys, errors)
}

/// Write every key, returning a message per failure.
pub fn save(keys: &Keys) -> Vec<String> {
    PROVIDERS
        .iter()
        .filter_map(|p| set(p, key_of(keys, p)).err().map(|e| format!("{p}: {e}")))
        .collect()
}

#[cfg(test)]
mod tests {
    /// Touches the real OS keychain, so it's opt-in: `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn keychain_round_trip() {
        let entry = keyring::Entry::new("magpie-selftest", "probe").unwrap();
        entry.set_password("s3cret").unwrap();
        assert_eq!(entry.get_password().unwrap(), "s3cret");
        entry.delete_credential().unwrap();
        assert!(matches!(entry.get_password(), Err(keyring::Error::NoEntry)));
    }
}
