use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::error::{NuanceError, Result};

/// The top-level `mod.toml` manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub package: Package,
    #[serde(default)]
    pub dependencies: HashMap<String, DependencySpec>,
}

/// The `[package]` section of a manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authors: Option<Vec<String>>,
    #[serde(rename = "nu-version", skip_serializing_if = "Option::is_none")]
    pub nu_version: Option<String>,
}

/// A single dependency specification from `[dependencies]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencySpec {
    pub git: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rev: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
}

impl DependencySpec {
    /// Validate that exactly one of tag/rev/branch is specified.
    pub fn validate(&self, name: &str) -> Result<()> {
        let count = [&self.tag, &self.rev, &self.branch]
            .iter()
            .filter(|v| v.is_some())
            .count();

        if count == 0 {
            return Err(NuanceError::Manifest(format!(
                "dependency '{name}': must specify one of 'tag', 'rev', or 'branch'"
            )));
        }
        if count > 1 {
            return Err(NuanceError::Manifest(format!(
                "dependency '{name}': specify only one of 'tag', 'rev', or 'branch'"
            )));
        }
        Ok(())
    }

    /// Returns the git ref string (tag, rev, or branch value).
    pub fn ref_spec(&self) -> &str {
        self.rev
            .as_deref()
            .or(self.tag.as_deref())
            .or(self.branch.as_deref())
            .expect("validated: one of tag/rev/branch is set")
    }
}

impl Manifest {
    /// Read and parse a `mod.toml` from the given directory.
    pub fn from_dir(dir: &Path) -> Result<Self> {
        let path = dir.join("mod.toml");
        if !path.exists() {
            return Err(NuanceError::NoManifest(dir.to_path_buf()));
        }
        let content = std::fs::read_to_string(&path)?;
        Self::from_str(&content)
    }

    /// Parse a manifest from a TOML string.
    pub fn from_str(s: &str) -> Result<Self> {
        let manifest: Manifest = toml::from_str(s)?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Validate the manifest contents.
    fn validate(&self) -> Result<()> {
        if self.package.name.is_empty() {
            return Err(NuanceError::Manifest(
                "package name cannot be empty".to_string(),
            ));
        }
        if self.package.version.is_empty() {
            return Err(NuanceError::Manifest(
                "package version cannot be empty".to_string(),
            ));
        }
        for (name, spec) in &self.dependencies {
            spec.validate(name)?;
        }
        Ok(())
    }

    /// Serialize this manifest to a TOML string.
    pub fn to_toml_string(&self) -> Result<String> {
        Ok(toml::to_string_pretty(self)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_manifest() {
        let toml = r#"
[package]
name = "my-module"
version = "0.3.1"
description = "Useful utilities"
license = "MIT"
authors = ["Test Author"]
nu-version = ">=0.101.0"

[dependencies]
nu-git-utils = { git = "https://github.com/someuser/nu-git-utils", tag = "v0.2.0" }
nu-str-extras = { git = "https://github.com/someuser/nu-str-extras", branch = "main" }
"#;
        let manifest = Manifest::from_str(toml).unwrap();
        assert_eq!(manifest.package.name, "my-module");
        assert_eq!(manifest.package.version, "0.3.1");
        assert_eq!(manifest.dependencies.len(), 2);
        assert!(manifest.dependencies.contains_key("nu-git-utils"));
        assert!(manifest.dependencies.contains_key("nu-str-extras"));
    }

    #[test]
    fn parse_minimal_manifest() {
        let toml = r#"
[package]
name = "minimal"
version = "0.1.0"
"#;
        let manifest = Manifest::from_str(toml).unwrap();
        assert_eq!(manifest.package.name, "minimal");
        assert!(manifest.dependencies.is_empty());
    }

    #[test]
    fn reject_no_ref_spec() {
        let toml = r#"
[package]
name = "bad"
version = "0.1.0"

[dependencies]
broken = { git = "https://github.com/user/broken" }
"#;
        let err = Manifest::from_str(toml).unwrap_err();
        assert!(err.to_string().contains("must specify one of"));
    }

    #[test]
    fn reject_multiple_ref_specs() {
        let toml = r#"
[package]
name = "bad"
version = "0.1.0"

[dependencies]
broken = { git = "https://github.com/user/broken", tag = "v1", branch = "main" }
"#;
        let err = Manifest::from_str(toml).unwrap_err();
        assert!(err.to_string().contains("specify only one of"));
    }

    #[test]
    fn reject_empty_name() {
        let toml = r#"
[package]
name = ""
version = "0.1.0"
"#;
        let err = Manifest::from_str(toml).unwrap_err();
        assert!(err.to_string().contains("name cannot be empty"));
    }
}
