use std::{fs, path::Path};

/// An unreadable or missing file must never be considered current.
pub fn matches_file(path: &Path, expected: &[u8]) -> bool {
    fs::read(path).is_ok_and(|actual| actual == expected)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compares_content_and_treats_missing_files_as_outdated() {
        let path = std::env::temp_dir().join(format!("winrust-update-check-{}", std::process::id()));
        fs::write(&path, b"new payload").unwrap();
        assert!(matches_file(&path, b"new payload"));
        assert!(!matches_file(&path, b"old payload"));
        fs::remove_file(&path).unwrap();
        assert!(!matches_file(&path, b"new payload"));
    }
}
