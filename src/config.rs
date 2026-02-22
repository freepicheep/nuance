use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::error::{NuanceError, Result};
use crate::manifest::DependencySpec;

/// The global nuance config file: `~/.config/nuance/config.toml`.
///
/// Tracks globally-installed modules and optional path overrides.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modules_dir: Option<String>,

    #[serde(default)]
    pub dependencies: HashMap<String, DependencySpec>,
}

impl GlobalConfig {
    /// Load the global config, creating it with defaults if it doesn't exist.
    pub fn load() -> Result<Self> {
        let path = global_config_path()?;

        if !path.exists() {
            let config = GlobalConfig {
                modules_dir: None,
                dependencies: HashMap::new(),
            };
            config.save()?;
            return Ok(config);
        }

        let content = std::fs::read_to_string(&path)?;
        let config: GlobalConfig = toml::from_str(&content)
            .map_err(|e| NuanceError::Config(format!("failed to parse {}: {e}", path.display())))?;
        Ok(config)
    }

    /// Save the global config back to disk.
    pub fn save(&self) -> Result<()> {
        let path = global_config_path()?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)
            .map_err(|e| NuanceError::Config(format!("failed to serialize config: {e}")))?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Returns the directory where global modules should be installed.
    ///
    /// Uses the `modules_dir` override if set, otherwise falls back to
    /// `~/.config/nushell/vendor/nuance_modules/`.
    pub fn modules_dir(&self) -> Result<PathBuf> {
        if let Some(ref custom) = self.modules_dir {
            Ok(PathBuf::from(custom))
        } else {
            global_modules_dir()
        }
    }
}

/// Returns the global config directory: `~/.config/nuance/`.
pub fn global_config_dir() -> Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| NuanceError::Config("could not determine home directory".to_string()))?;
    Ok(home.join(".config").join("nuance"))
}

/// Returns the path to the global config file: `~/.config/nuance/config.toml`.
pub fn global_config_path() -> Result<PathBuf> {
    Ok(global_config_dir()?.join("config.toml"))
}

/// Returns the path to the global lockfile: `~/.config/nuance/config.lock`.
pub fn global_lock_path() -> Result<PathBuf> {
    Ok(global_config_dir()?.join("config.lock"))
}

/// Returns the default global modules directory, using the platform config
/// directory (where Nushell stores its config) + `vendor/nuance_modules/`.
///
/// e.g. `~/Library/Application Support/nushell/vendor/nuance_modules/` on macOS,
///      `~/.config/nushell/vendor/nuance_modules/` on Linux.
pub fn global_modules_dir() -> Result<PathBuf> {
    let config = dirs::config_dir()
        .ok_or_else(|| NuanceError::Config("could not determine config directory".to_string()))?;
    Ok(config.join("nushell").join("vendor").join("nuance_modules"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let config = GlobalConfig {
            modules_dir: None,
            dependencies: HashMap::from([(
                "nu-utils".to_string(),
                DependencySpec {
                    git: "https://github.com/user/nu-utils".to_string(),
                    tag: Some("v1.0.0".to_string()),
                    rev: None,
                    branch: None,
                },
            )]),
        };

        let serialized = toml::to_string_pretty(&config).unwrap();
        let parsed: GlobalConfig = toml::from_str(&serialized).unwrap();

        assert_eq!(parsed.dependencies.len(), 1);
        assert!(parsed.dependencies.contains_key("nu-utils"));
        assert!(parsed.modules_dir.is_none());
    }

    #[test]
    fn round_trip_with_override() {
        let config = GlobalConfig {
            modules_dir: Some("/custom/path".to_string()),
            dependencies: HashMap::new(),
        };

        let serialized = toml::to_string_pretty(&config).unwrap();
        let parsed: GlobalConfig = toml::from_str(&serialized).unwrap();

        assert_eq!(parsed.modules_dir.as_deref(), Some("/custom/path"));
    }

    #[test]
    fn modules_dir_custom() {
        let config = GlobalConfig {
            modules_dir: Some("/custom/modules".to_string()),
            dependencies: HashMap::new(),
        };
        assert_eq!(
            config.modules_dir().unwrap(),
            PathBuf::from("/custom/modules")
        );
    }

    #[test]
    fn modules_dir_default() {
        let config = GlobalConfig {
            modules_dir: None,
            dependencies: HashMap::new(),
        };
        let dir = config.modules_dir().unwrap();
        // Should end with nushell/vendor/nuance_modules
        assert!(dir.ends_with("nushell/vendor/nuance_modules"));
    }

    #[test]
    fn config_dir_paths() {
        // These should not error on any platform with a home directory
        let dir = global_config_dir().unwrap();
        assert!(dir.ends_with("nuance"));

        let path = global_config_path().unwrap();
        assert!(path.ends_with("nuance/config.toml"));

        let lock = global_lock_path().unwrap();
        assert!(lock.ends_with("nuance/config.lock"));
    }
}
