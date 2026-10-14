//! A small on-disk cache for API responses and downloaded files, so repeat
//! searches are instant and we stay well under Wikimedia's rate limits.
//! Entries live in the OS cache dir (`~/Library/Caches/magpie` on macOS).

use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

/// Search/API responses: results change rarely, but new logos do get uploaded.
pub const API_TTL: Duration = Duration::from_secs(7 * 24 * 3600);
/// Downloaded logo and icon files.
pub const FILE_TTL: Duration = Duration::from_secs(30 * 24 * 3600);

fn dir() -> Option<&'static PathBuf> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = directories::ProjectDirs::from("", "", "magpie")?
            .cache_dir()
            .to_path_buf();
        // Carry over the cache from when the app was called Logo Finder.
        if let Some(old) = directories::ProjectDirs::from("", "", "logo-finder")
            && !dir.exists()
            && old.cache_dir().exists()
        {
            let _ = fs::rename(old.cache_dir(), &dir);
        }
        fs::create_dir_all(&dir).ok()?;
        Some(dir)
    })
    .as_ref()
}

/// FNV-1a: stable across Rust versions, unlike `DefaultHasher`.
fn hash(key: &str) -> u64 {
    key.bytes()
        .fold(0xcbf29ce484222325, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}

fn path(key: &str) -> Option<PathBuf> {
    Some(dir()?.join(format!("{:016x}", hash(key))))
}

pub fn get(key: &str, ttl: Duration) -> Option<Vec<u8>> {
    let path = path(key)?;
    let age = fs::metadata(&path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())?;
    if age > ttl {
        return None;
    }
    fs::read(path).ok()
}

/// Best-effort: a failed write just means a cache miss next time.
pub fn put(key: &str, data: &[u8]) {
    let Some(path) = path(key) else { return };
    // Write then rename so a crash never leaves a half-written entry.
    let tmp = path.with_extension("tmp");
    if fs::write(&tmp, data).is_ok() {
        let _ = fs::rename(&tmp, &path);
    }
}

/// Total size of the cache in bytes.
pub fn size() -> u64 {
    let Some(dir) = dir() else { return 0 };
    fs::read_dir(dir)
        .map(|entries| entries.filter_map(|e| e.ok()?.metadata().ok()).map(|m| m.len()).sum())
        .unwrap_or(0)
}

pub fn clear() -> Result<(), String> {
    let Some(dir) = dir() else { return Ok(()) };
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        fs::remove_file(entry.path()).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_stable() {
        assert_eq!(hash(""), 0xcbf29ce484222325);
        assert_eq!(hash("a"), 0xaf63dc4c8601ec8c);
    }
}
