mod checksum;
mod cli;
mod error;
mod git;
mod installer;
mod lockfile;
mod manifest;
mod resolver;

use std::path::Path;

use cli::Commands;
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
        Commands::Install { frozen } => cmd_install(&cwd, frozen),
        Commands::Update => cmd_update(&cwd),
        Commands::Add {
            url,
            tag,
            rev,
            branch,
        } => cmd_add(&cwd, url, tag, rev, branch),
        Commands::Remove { name } => cmd_remove(&cwd, name),
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
    let dep_spec = if tag.is_none() && rev.is_none() && branch.is_none() {
        eprintln!("Fetching {url} to detect version...");
        let repo_path = git::clone_or_fetch(&url)?;

        if let Some(latest) = git::latest_tag(&repo_path)? {
            eprintln!("  Found latest tag: {latest}");
            DependencySpec {
                git: url.clone(),
                tag: Some(latest),
                rev: None,
                branch: None,
            }
        } else {
            let default_br = git::default_branch(&repo_path)?;
            eprintln!("  No tags found, using branch: {default_br}");
            DependencySpec {
                git: url.clone(),
                tag: None,
                rev: None,
                branch: Some(default_br),
            }
        }
    } else {
        DependencySpec {
            git: url.clone(),
            tag,
            rev,
            branch,
        }
    };

    dep_spec.validate(&pkg_name)?;

    // Add to manifest and write back
    manifest.dependencies.insert(pkg_name.clone(), dep_spec);
    let content = manifest.to_toml_string()?;
    std::fs::write(dir.join("mod.toml"), content)?;

    eprintln!("Added '{pkg_name}' to mod.toml");

    // Run install
    installer::install(dir, false)
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
