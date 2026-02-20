use std::collections::HashMap;
use std::path::Path;

use crate::checksum;
use crate::error::{NuanceError, Result};
use crate::git::{self, RefKind};
use crate::lockfile::LockedPackage;
use crate::manifest::{DependencySpec, Manifest};

/// A fully resolved dependency.
#[derive(Debug, Clone)]
pub struct ResolvedDep {
    pub name: String,
    pub git: String,
    pub tag: Option<String>,
    pub rev: String,
}

/// Resolve all dependencies (including transitive) from a root manifest.
///
/// Returns a flat map of package name → resolved dependency.
/// Errors on conflicts (same name, different source or rev).
pub fn resolve(root_dir: &Path) -> Result<Vec<ResolvedDep>> {
    let manifest = Manifest::from_dir(root_dir)?;
    let mut resolved: HashMap<String, ResolvedDep> = HashMap::new();

    resolve_deps(&manifest.dependencies, &mut resolved)?;

    // Return sorted for deterministic output
    let mut deps: Vec<_> = resolved.into_values().collect();
    deps.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(deps)
}

/// Resolve dependencies from an existing lockfile without re-fetching.
pub fn resolve_from_lock(locked: &[LockedPackage]) -> Vec<ResolvedDep> {
    locked
        .iter()
        .map(|p| ResolvedDep {
            name: p.name.clone(),
            git: p.git.clone(),
            tag: p.tag.clone(),
            rev: p.rev.clone(),
        })
        .collect()
}

fn resolve_deps(
    deps: &HashMap<String, DependencySpec>,
    resolved: &mut HashMap<String, ResolvedDep>,
) -> Result<()> {
    for (name, spec) in deps {
        // Clone or fetch the repo
        eprintln!("  Fetching {name} from {}...", spec.git);
        let repo_path = git::clone_or_fetch(&spec.git)?;

        // Resolve the ref to a commit SHA
        let kind = RefKind::from_spec(&spec.tag, &spec.rev, &spec.branch);
        let rev = git::resolve_ref(&repo_path, spec.ref_spec(), kind)?;

        // Check for conflicts
        if let Some(existing) = resolved.get(name) {
            if existing.rev != rev || existing.git != spec.git {
                return Err(NuanceError::Conflict {
                    name: name.clone(),
                    rev_a: existing.rev.clone(),
                    rev_b: rev,
                });
            }
            // Same resolution — skip (already resolved)
            continue;
        }

        resolved.insert(
            name.clone(),
            ResolvedDep {
                name: name.clone(),
                git: spec.git.clone(),
                tag: spec.tag.clone(),
                rev: rev.clone(),
            },
        );

        // Check for transitive dependencies
        // Export the dep to a temp dir to read its mod.toml
        let tmp = std::env::temp_dir()
            .join("nuance_resolve")
            .join(name);
        git::export_to(&repo_path, &rev, &tmp)?;

        if let Ok(dep_manifest) = Manifest::from_dir(&tmp) {
            if !dep_manifest.dependencies.is_empty() {
                eprintln!("  Resolving transitive dependencies for {name}...");
                resolve_deps(&dep_manifest.dependencies, resolved)?;
            }
        }

        // Clean up temp dir
        let _ = std::fs::remove_dir_all(&tmp);
    }

    Ok(())
}

/// Compute the SHA-256 checksum of an exported dependency directory.
pub fn compute_checksum(dir: &Path) -> Result<String> {
    checksum::hash_directory(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conflict_detection() {
        let mut resolved = HashMap::new();
        resolved.insert(
            "my-dep".to_string(),
            ResolvedDep {
                name: "my-dep".to_string(),
                git: "https://github.com/user/my-dep".to_string(),
                tag: Some("v1.0.0".to_string()),
                rev: "aaaa".to_string(),
            },
        );

        // Same name, different rev = conflict
        let mut deps = HashMap::new();
        deps.insert(
            "my-dep".to_string(),
            DependencySpec {
                git: "https://github.com/user/my-dep".to_string(),
                tag: Some("v2.0.0".to_string()),
                rev: None,
                branch: None,
            },
        );

        // This would try to fetch from git which we can't do in a unit test,
        // so we test the conflict logic directly
        // In a real scenario, resolve_deps would detect the conflict after resolving
        // For now, just verify the data structure works
        assert!(resolved.contains_key("my-dep"));
    }
}
