mod checksum;
mod cli;
mod config;
mod error;
mod git;
mod installer;
mod lockfile;
mod manifest;
mod resolver;

use std::path::Path;

use cli::Commands;
use config::GlobalConfig;
use error::Result;
use manifest::{DependencySpec, Manifest, Package};

fn main() {
    let cli = cli::parse();

    if let Err(e) = run(cli.command) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run(command: Commands) -> Result<()> {
    let cwd = std::env::current_dir()?;

    match command {
        Commands::Init {
            name,
            version,
            description,
        } => cmd_init(&cwd, name, version, description),
        Commands::Install { global, frozen } => {
            if global {
                cmd_install_global(frozen)
            } else {
                cmd_install(&cwd, frozen)
            }
        }
        Commands::Update => cmd_update(&cwd),
        Commands::Add {
            global,
            url,
            tag,
            rev,
            branch,
        } => {
            if global {
                cmd_add_global(url, tag, rev, branch)
            } else {
                cmd_add(&cwd, url, tag, rev, branch)
            }
        }
        Commands::Remove { global, name } => {
            if global {
                cmd_remove_global(name)
            } else {
                cmd_remove(&cwd, name)
            }
        }
    }
}

fn cmd_init(
    dir: &Path,
    name: Option<String>,
    version: String,
    description: Option<String>,
) -> Result<()> {
    let mod_toml = dir.join("mod.toml");
    if mod_toml.exists() {
        return Err(error::NuanceError::Manifest(
            "mod.toml already exists in this directory".to_string(),
        ));
    }

    // Default name to directory name
    let pkg_name = name.unwrap_or_else(|| {
        dir.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("my-module")
            .to_string()
    });

    let manifest = Manifest {
        package: Package {
            name: pkg_name.clone(),
            version,
            description,
            license: None,
            authors: None,
            nu_version: None,
        },
        dependencies: Default::default(),
    };

    let content = manifest.to_toml_string()?;
    std::fs::write(&mod_toml, content)?;
    eprintln!("Created mod.toml for '{pkg_name}'");

    // Also create mod.nu if it doesn't exist
    let mod_nu = dir.join("mod.nu");
    if !mod_nu.exists() {
        std::fs::write(
            &mod_nu,
            "# Module entry point\n# Export your commands here with: export use <submodule>\n",
        )?;
        eprintln!("Created mod.nu");
    }

    Ok(())
}

fn cmd_install(dir: &Path, frozen: bool) -> Result<()> {
    installer::install(dir, frozen)
}

fn cmd_install_global(frozen: bool) -> Result<()> {
    installer::install_global(frozen)
}

fn cmd_update(dir: &Path) -> Result<()> {
    installer::update(dir)
}

fn cmd_add(
    dir: &Path,
    url: String,
    tag: Option<String>,
    rev: Option<String>,
    branch: Option<String>,
) -> Result<()> {
    // Load existing manifest (or error if none)
    let mut manifest = Manifest::from_dir(dir)?;

    // Derive package name from URL
    let pkg_name = git::repo_name_from_url(&url).ok_or_else(|| {
        error::NuanceError::Other(format!("could not determine package name from URL: {url}"))
    })?;

    // Check if already added
    if manifest.dependencies.contains_key(&pkg_name) {
        return Err(error::NuanceError::Manifest(format!(
            "dependency '{pkg_name}' already exists in mod.toml"
        )));
    }

    // If no ref spec given, auto-detect: try latest tag, fall back to default branch
    let dep_spec = auto_detect_dep_spec(&url, tag, rev, branch)?;

    dep_spec.validate(&pkg_name)?;

    // Add to manifest and write back
    manifest.dependencies.insert(pkg_name.clone(), dep_spec);
    let content = manifest.to_toml_string()?;
    std::fs::write(dir.join("mod.toml"), content)?;

    eprintln!("Added '{pkg_name}' to mod.toml");

    // Run install
    installer::install(dir, false)
}

fn cmd_add_global(
    url: String,
    tag: Option<String>,
    rev: Option<String>,
    branch: Option<String>,
) -> Result<()> {
    let mut config = GlobalConfig::load()?;

    // Derive package name from URL
    let pkg_name = git::repo_name_from_url(&url).ok_or_else(|| {
        error::NuanceError::Other(format!("could not determine package name from URL: {url}"))
    })?;

    // Check if already added
    if config.dependencies.contains_key(&pkg_name) {
        return Err(error::NuanceError::Config(format!(
            "dependency '{pkg_name}' already exists in global config"
        )));
    }

    let dep_spec = auto_detect_dep_spec(&url, tag, rev, branch)?;

    dep_spec.validate(&pkg_name)?;

    // Add to global config and save
    config.dependencies.insert(pkg_name.clone(), dep_spec);
    config.save()?;

    eprintln!("Added '{pkg_name}' to global config");

    // Run global install
    installer::install_global(false)
}

fn cmd_remove(dir: &Path, name: String) -> Result<()> {
    // Load existing manifest
    let mut manifest = Manifest::from_dir(dir)?;

    // Check the dep exists
    if manifest.dependencies.remove(&name).is_none() {
        return Err(error::NuanceError::Manifest(format!(
            "dependency '{name}' not found in mod.toml"
        )));
    }

    // Write updated manifest
    let content = manifest.to_toml_string()?;
    std::fs::write(dir.join("mod.toml"), content)?;
    eprintln!("Removed '{name}' from mod.toml");

    // Remove from .nu_modules/
    let module_dir = dir.join(".nu_modules").join(&name);
    if module_dir.exists() {
        std::fs::remove_dir_all(&module_dir)?;
        eprintln!("Removed .nu_modules/{name}/");
    }

    // Update lockfile: remove the package entry
    let lock_path = dir.join("mod.lock");
    if lock_path.exists() {
        let mut lockfile = lockfile::Lockfile::from_path(&lock_path)?;
        lockfile.packages.retain(|p| p.name != name);
        lockfile.write_to(&lock_path)?;
        eprintln!("Updated mod.lock");
    }

    Ok(())
}

fn cmd_remove_global(name: String) -> Result<()> {
    let mut config = GlobalConfig::load()?;

    // Check the dep exists
    if config.dependencies.remove(&name).is_none() {
        return Err(error::NuanceError::Config(format!(
            "dependency '{name}' not found in global config"
        )));
    }

    // Save updated config
    config.save()?;
    eprintln!("Removed '{name}' from global config");

    // Remove from global modules dir
    let modules_dir = config.modules_dir()?;
    let module_dir = modules_dir.join(&name);
    if module_dir.exists() {
        std::fs::remove_dir_all(&module_dir)?;
        eprintln!("Removed {}/", module_dir.display());
    }

    // Update global lockfile
    let lock_path = config::global_lock_path()?;
    if lock_path.exists() {
        let mut lockfile = lockfile::Lockfile::from_path(&lock_path)?;
        lockfile.packages.retain(|p| p.name != name);
        lockfile.write_to(&lock_path)?;
        eprintln!("Updated global lockfile");
    }

    Ok(())
}

/// Auto-detect the dependency spec from a URL, optionally with an explicit ref.
///
/// If no tag/rev/branch is given, tries the latest tag first, then falls back
/// to the default branch.
fn auto_detect_dep_spec(
    url: &str,
    tag: Option<String>,
    rev: Option<String>,
    branch: Option<String>,
) -> Result<DependencySpec> {
    if tag.is_none() && rev.is_none() && branch.is_none() {
        eprintln!("Fetching {url} to detect version...");
        let repo_path = git::clone_or_fetch(url)?;

        if let Some(latest) = git::latest_tag(&repo_path)? {
            eprintln!("  Found latest tag: {latest}");
            Ok(DependencySpec {
                git: url.to_string(),
                tag: Some(latest),
                rev: None,
                branch: None,
            })
        } else {
            let default_br = git::default_branch(&repo_path)?;
            eprintln!("  No tags found, using branch: {default_br}");
            Ok(DependencySpec {
                git: url.to_string(),
                tag: None,
                rev: None,
                branch: Some(default_br),
            })
        }
    } else {
        Ok(DependencySpec {
            git: url.to_string(),
            tag,
            rev,
            branch,
        })
    }
}
