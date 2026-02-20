use std::path::Path;

use crate::error::Result;
use crate::git;
use crate::lockfile::{LockedPackage, Lockfile};
use crate::manifest::Manifest;
use crate::resolver::{self, ResolvedDep};

/// The name of the directory where dependencies are installed.
const MODULES_DIR: &str = ".nu_modules";

/// Run a full install: resolve → fetch → checksum → place → lock.
pub fn install(project_dir: &Path, frozen: bool) -> Result<()> {
    let manifest = Manifest::from_dir(project_dir)?;
    let lock_path = project_dir.join("mod.lock");
    let modules_dir = project_dir.join(MODULES_DIR);

    if manifest.dependencies.is_empty() {
        eprintln!("No dependencies declared in mod.toml.");
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
    std::fs::create_dir_all(&modules_dir)?;
    let mut locked_packages = Vec::new();

    for dep in &resolved {
        eprintln!("  Installing {}@{}...", dep.name, &dep.rev[..12.min(dep.rev.len())]);
        install_dep(dep, &modules_dir)?;

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
    lockfile.write_to(&lock_path)?;

    eprintln!(
        "\nInstalled {} package{} into {}/",
        resolved.len(),
        if resolved.len() == 1 { "" } else { "s" },
        MODULES_DIR
    );

    Ok(())
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
