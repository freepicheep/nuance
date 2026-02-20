# nuance

A module manager for [Nushell](https://www.nushell.sh/).

nuance handles dependency resolution, fetching, and lockfile management for Nushell modules distributed as git repositories.

## Install

```bash
cargo install --git https://github.com/freepicheep/nuance
```

## Quick Start

```bash
# Initialize a new module project
nuance init

# Add a dependency
nuance add https://github.com/user/nu-some-module

# Install all dependencies from mod.toml
nuance install

# Re-resolve everything (ignore lockfile)
nuance update

# Remove a dependency
nuance remove nu-some-module
```

## How It Works

A nuance project is a directory containing:

- **`mod.toml`** — declares package metadata and dependencies
- **`mod.nu`** — the Nushell module entry point
- **`mod.lock`** — auto-generated lockfile pinning exact commits (commit this to version control)

Running `nuance install` fetches dependencies into `.nu_modules/`. Use them in your code by running `nuance use .nu_modules/module-name *`. 

## mod.toml

```toml
[package]
name = "my-module"
version = "0.1.0"
description = "Something useful"

[dependencies]
nu-utils = { git = "https://github.com/user/nu-utils", tag = "v1.0.0" }
other-lib = { git = "https://github.com/user/other-lib", branch = "main" }
pinned = { git = "https://github.com/user/pinned", rev = "a3f9c12" }
```

Each dependency must specify exactly one of `tag`, `branch`, or `rev`.

## Commands

| Command | Description |
|---------|-------------|
| `nuance init` | Create a new `mod.toml` in the current directory |
| `nuance add <url>` | Add a dependency (auto-detects latest tag) |
| `nuance install` | Install dependencies from `mod.toml` |
| `nuance install --frozen` | Install from lockfile only (CI-friendly) |
| `nuance update` | Re-resolve all dependencies |
| `nuance remove <name>` | Remove a dependency |

## License

MIT
