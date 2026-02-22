# Unreleased

Nothing yet!

# Version 0.1.1 (2026-02-21)

## Added

- Added global module management via `--global`/`-g` for `nuance install`,
  `nuance add`, and `nuance remove`.
- Added generated `.nu_modules/activate.nu` output from `nuance init` and
  `nuance install` to make project module activation easier.
- Added `nuance hook` to print a Nushell env-change hook for automatic project
  activation.
- Added configurable default git provider support for `owner/repo` shorthand in
  `nuance add` via `default_git_provider`.

## Changed

- Updated README install docs to include Homebrew, shell script, and `mise`
  installation methods.
- Updated README with badges and general formatting improvements.

# Version 0.1.0 (2026-02-20)

## Added

- Initial public release.
