use std::path::Path;

use crate::config::{self, GlobalConfig};
use crate::error::Result;
use crate::git;
use crate::lockfile::{LockedPackage, Lockfile};
use crate::manifest::Manifest;
use crate::resolver::{self, ResolvedDep};

/// The name of the directory where local dependencies are installed.
const MODULES_DIR: &str = ".nu_modules";

/// Run a full local install: resolve → fetch → checksum → place → lock.
pub fn install(project_dir: &Path, frozen: bool) -> Result<()> {
    let manifest = Manifest::from_dir(project_dir)?;
    let lock_path = project_dir.join("mod.lock");
    let modules_dir = project_dir.join(MODULES_DIR);

    if manifest.dependencies.is_empty() {
        eprintln!("No dependencies declared in mod.toml.");
        write_activate_overlay(&modules_dir, MODULES_DIR, std::iter::empty::<&str>())?;
        return Ok(());
    }

    // Determine whether to re-resolve or use the lockfile
    let resolved = if frozen {
        // --frozen: use lockfile only
        if !lock_path.exists() {
            return Err(crate::error::NuanceError::Lockfile(
                "mod.lock not found (required with --frozen)".to_string(),
            ));
        }
        let lockfile = Lockfile::from_path(&lock_path)?;
        eprintln!("Using locked dependencies (--frozen).");
        resolver::resolve_from_lock(&lockfile.packages)
    } else if lock_path.exists() && !is_lockfile_stale(project_dir)? {
        // Lockfile exists and is up-to-date
        let lockfile = Lockfile::from_path(&lock_path)?;
        eprintln!("Using existing lockfile.");
        resolver::resolve_from_lock(&lockfile.packages)
    } else {
        // Resolve fresh
        eprintln!("Resolving dependencies...");
        resolver::resolve(project_dir)?
    };

    // Install each dependency
    install_resolved(&resolved, &modules_dir, &lock_path, MODULES_DIR)
}

/// Run an update: always re-resolve, ignoring existing lockfile.
pub fn update(project_dir: &Path) -> Result<()> {
    let lock_path = project_dir.join("mod.lock");
    // Remove existing lockfile to force re-resolution
    if lock_path.exists() {
        std::fs::remove_file(&lock_path)?;
    }
    install(project_dir, false)
}

/// Run a global install: resolve from `~/.config/nuance/config.toml` and install
/// modules to the global modules directory.
pub fn install_global(frozen: bool) -> Result<()> {
    let config = GlobalConfig::load()?;
    let modules_dir = config.modules_dir()?;
    let lock_path = config::global_lock_path()?;
    let display_dir = modules_dir.display().to_string();

    if config.dependencies.is_empty() {
        eprintln!("No dependencies declared in global config.");
        write_activate_overlay(&modules_dir, &display_dir, std::iter::empty::<&str>())?;
        return Ok(());
    }

    let resolved = if frozen {
        if !lock_path.exists() {
            return Err(crate::error::NuanceError::Lockfile(
                "config.lock not found (required with --frozen)".to_string(),
            ));
        }
        let lockfile = Lockfile::from_path(&lock_path)?;
        eprintln!("Using locked global dependencies (--frozen).");
        resolver::resolve_from_lock(&lockfile.packages)
    } else if lock_path.exists() && !is_global_lockfile_stale(&config, &lock_path)? {
        let lockfile = Lockfile::from_path(&lock_path)?;
        eprintln!("Using existing global lockfile.");
        resolver::resolve_from_lock(&lockfile.packages)
    } else {
        eprintln!("Resolving global dependencies...");
        resolver::resolve_from_deps(&config.dependencies)?
    };

    install_resolved(&resolved, &modules_dir, &lock_path, &display_dir)
}

/// Install a list of resolved dependencies into a target directory and write the lockfile.
fn install_resolved(
    resolved: &[ResolvedDep],
    modules_dir: &Path,
    lock_path: &Path,
    display_name: &str,
) -> Result<()> {
    std::fs::create_dir_all(modules_dir)?;
    let mut locked_packages = Vec::new();

    for dep in resolved {
        eprintln!(
            "  Installing {}@{}...",
            dep.name,
            &dep.rev[..12.min(dep.rev.len())]
        );
        install_dep(dep, modules_dir)?;

        let dest = modules_dir.join(&dep.name);
        let sha256 = resolver::compute_checksum(&dest)?;

        locked_packages.push(LockedPackage {
            name: dep.name.clone(),
            git: dep.git.clone(),
            tag: dep.tag.clone(),
            rev: dep.rev.clone(),
            sha256,
        });
    }

    // Write lockfile
    let lockfile = Lockfile {
        version: 1,
        packages: locked_packages,
    };
    lockfile.write_to(lock_path)?;

    eprintln!(
        "\nInstalled {} package{} into {}/",
        resolved.len(),
        if resolved.len() == 1 { "" } else { "s" },
        display_name
    );

    write_activate_overlay(
        modules_dir,
        display_name,
        resolved.iter().map(|dep| dep.name.as_str()),
    )?;

    Ok(())
}

fn write_activate_overlay<I, S>(
    modules_dir: &Path,
    display_name: &str,
    module_names: I,
) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    std::fs::create_dir_all(modules_dir)?;

    let activate_path = modules_dir.join("activate.nu");
    let mut activate_script = String::from(
        "# Generated by nuance — do not edit\nexport-env {\n    let modules_dir = ($env.FILE_PWD | path join)\n    $env.NU_LIB_DIRS = ($env.NU_LIB_DIRS | default [] | append $modules_dir)\n}\n\n",
    );

    for module_name in module_names {
        activate_script.push_str("export use ");
        activate_script.push_str(module_name.as_ref());
        activate_script.push_str(" *\n");
    }

    activate_script.push_str("\nexport alias deactivate = overlay hide activate\n");

    std::fs::write(&activate_path, activate_script)?;
    eprintln!("Generated {}/activate.nu", display_name);
    Ok(())
}

/// Install a single resolved dependency into the modules directory.
fn install_dep(dep: &ResolvedDep, modules_dir: &Path) -> Result<()> {
    let repo_path = git::clone_or_fetch(&dep.git)?;
    let dest = modules_dir.join(&dep.name);
    git::export_to(&repo_path, &dep.rev, &dest)?;
    Ok(())
}

/// Check if the lockfile is stale relative to mod.toml.
///
/// A simple heuristic: if the dependency names in mod.toml don't match
/// the locked package names, the lockfile is stale.
fn is_lockfile_stale(project_dir: &Path) -> Result<bool> {
    let manifest = Manifest::from_dir(project_dir)?;
    let lock_path = project_dir.join("mod.lock");

    if !lock_path.exists() {
        return Ok(true);
    }

    let lockfile = Lockfile::from_path(&lock_path)?;

    // Check if all manifest deps are in the lockfile
    for name in manifest.dependencies.keys() {
        if lockfile.find_package(name).is_none() {
            return Ok(true); // New dep not in lockfile
        }
    }

    // Check if lockfile has deps not in manifest
    for pkg in &lockfile.packages {
        if !manifest.dependencies.contains_key(&pkg.name) {
            return Ok(true); // Removed dep still in lockfile
        }
    }

    Ok(false)
}

/// Check if the global lockfile is stale relative to the global config.
fn is_global_lockfile_stale(config: &GlobalConfig, lock_path: &Path) -> Result<bool> {
    if !lock_path.exists() {
        return Ok(true);
    }

    let lockfile = Lockfile::from_path(lock_path)?;

    for name in config.dependencies.keys() {
        if lockfile.find_package(name).is_none() {
            return Ok(true);
        }
    }

    for pkg in &lockfile.packages {
        if !config.dependencies.contains_key(&pkg.name) {
            return Ok(true);
        }
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_temp_dir(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "nuance_installer_test_{}_{}_{}",
            label,
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn writes_activate_overlay_with_modules() {
        let modules_dir = make_temp_dir("with_modules");

        write_activate_overlay(&modules_dir, ".nu_modules", ["nu-foo", "nu-bar"]).unwrap();

        let activate = std::fs::read_to_string(modules_dir.join("activate.nu")).unwrap();
        assert!(activate.contains("export use nu-foo *"));
        assert!(activate.contains("export use nu-bar *"));
        assert!(activate.contains("export alias deactivate = overlay hide activate"));

        let _ = std::fs::remove_dir_all(modules_dir);
    }

    #[test]
    fn writes_activate_overlay_without_modules() {
        let modules_dir = make_temp_dir("without_modules");

        write_activate_overlay(&modules_dir, ".nu_modules", std::iter::empty::<&str>()).unwrap();

        let activate = std::fs::read_to_string(modules_dir.join("activate.nu")).unwrap();
        assert!(!activate.lines().any(|line| line.starts_with("export use ")));
        assert!(activate.contains("export alias deactivate = overlay hide activate"));

        let _ = std::fs::remove_dir_all(modules_dir);
    }

    #[test]
    fn frozen_install_without_dependencies_writes_activate_overlay() {
        let project_dir = make_temp_dir("empty_manifest");
        std::fs::write(
            project_dir.join("mod.toml"),
            r#"[package]
name = "demo"
version = "0.1.0"
"#,
        )
        .unwrap();

        install(&project_dir, true).unwrap();

        let activate =
            std::fs::read_to_string(project_dir.join(".nu_modules").join("activate.nu")).unwrap();
        assert!(!activate.lines().any(|line| line.starts_with("export use ")));
        assert!(activate.contains("export alias deactivate = overlay hide activate"));

        let _ = std::fs::remove_dir_all(project_dir);
    }
}
