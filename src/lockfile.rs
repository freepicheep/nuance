use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::Result;

/// The `mod.lock` lockfile.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Lockfile {
    pub version: u32,
    #[serde(rename = "package")]
    pub packages: Vec<LockedPackage>,
}

/// A single locked package entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LockedPackage {
    pub name: String,
    pub git: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    pub rev: String,
    pub sha256: String,
}

impl Lockfile {
    /// Read a lockfile from disk.
    pub fn from_path(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::from_str(&content)
    }

    /// Parse a lockfile from a TOML string.
    pub fn from_str(s: &str) -> Result<Self> {
        Ok(toml::from_str(s)?)
    }

    /// Serialize the lockfile to a TOML string with the header comment.
    pub fn to_toml_string(&self) -> Result<String> {
        let body = toml::to_string_pretty(self)?;
        Ok(format!("# This file is generated automatically. Do not edit.\n{body}"))
    }

    /// Write the lockfile to disk.
    pub fn write_to(&self, path: &Path) -> Result<()> {
        let content = self.to_toml_string()?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Look up a locked package by name.
    pub fn find_package(&self, name: &str) -> Option<&LockedPackage> {
        self.packages.iter().find(|p| p.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_lockfile() -> Lockfile {
        Lockfile {
            version: 1,
            packages: vec![
                LockedPackage {
                    name: "nu-git-utils".to_string(),
                    git: "https://github.com/someuser/nu-git-utils".to_string(),
                    tag: Some("v0.2.0".to_string()),
                    rev: "d4e8f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8".to_string(),
                    sha256: "abc123".to_string(),
                },
                LockedPackage {
                    name: "nu-str-extras".to_string(),
                    git: "https://github.com/someuser/nu-str-extras".to_string(),
                    tag: Some("v1.0.0".to_string()),
                    rev: "1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b".to_string(),
                    sha256: "def456".to_string(),
                },
            ],
        }
    }

    #[test]
    fn round_trip() {
        let lock = sample_lockfile();
        let serialized = lock.to_toml_string().unwrap();

        // The header comment is not part of the TOML data, strip it for parsing
        let parsed = Lockfile::from_str(&serialized).unwrap();
        assert_eq!(lock, parsed);
    }

    #[test]
    fn find_package_by_name() {
        let lock = sample_lockfile();
        let pkg = lock.find_package("nu-git-utils").unwrap();
        assert_eq!(pkg.rev, "d4e8f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8");
        assert!(lock.find_package("nonexistent").is_none());
    }

    #[test]
    fn parse_spec_format() {
        let toml = r#"
# This file is generated automatically. Do not edit.
version = 1

[[package]]
name = "nu-git-utils"
git = "https://github.com/someuser/nu-git-utils"
tag = "v0.2.0"
rev = "d4e8f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8"
sha256 = "abc123"
"#;
        let lock = Lockfile::from_str(toml).unwrap();
        assert_eq!(lock.version, 1);
        assert_eq!(lock.packages.len(), 1);
        assert_eq!(lock.packages[0].name, "nu-git-utils");
    }
}
