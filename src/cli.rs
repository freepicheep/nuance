use clap::{Parser, Subcommand};

/// nuance — A module manager for Nushell
#[derive(Parser, Debug)]
#[command(name = "nuance", version, about = "A module manager for Nushell")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Create a new mod.toml in the current directory
    Init {
        /// Package name (defaults to current directory name)
        #[arg(long)]
        name: Option<String>,

        /// Package version
        #[arg(long, default_value = "0.1.0")]
        version: String,

        /// Package description
        #[arg(long)]
        description: Option<String>,
    },

    /// Resolve and install dependencies from mod.toml
    Install {
        /// Install global modules (from ~/.config/nuance/config.toml)
        #[arg(short = 'g', long)]
        global: bool,

        /// Use lockfile only; error if missing or stale
        #[arg(long)]
        frozen: bool,
    },

    /// Re-resolve all dependencies (ignore existing lockfile)
    Update,

    /// Add a package from a git repository URL
    Add {
        /// Add to global config instead of local mod.toml
        #[arg(short = 'g', long)]
        global: bool,

        /// Git repository URL (e.g. https://github.com/user/nu-module)
        url: String,

        /// Pin to a specific tag
        #[arg(long)]
        tag: Option<String>,

        /// Pin to a specific commit SHA
        #[arg(long)]
        rev: Option<String>,

        /// Track a branch
        #[arg(long)]
        branch: Option<String>,
    },

    /// Remove a package from mod.toml and .nu_modules/
    Remove {
        /// Remove from global config instead of local mod.toml
        #[arg(short = 'g', long)]
        global: bool,

        /// Package name to remove
        name: String,
    },
}

pub fn parse() -> Cli {
    Cli::parse()
}
