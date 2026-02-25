# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.3](https://github.com/km-tr/worktree-link/compare/v0.1.2...v0.1.3) - 2026-02-25

### Other

- remove cargo-release from mise.toml ([#12](https://github.com/km-tr/worktree-link/pull/12))

## [0.1.2](https://github.com/km-tr/worktree-link/compare/v0.1.1...v0.1.2) - 2026-02-24

### Added

- make source optional with --source/-s and --target/-t flags ([#8](https://github.com/km-tr/worktree-link/pull/8))

## [0.1.1](https://github.com/km-tr/worktree-link/compare/v0.1.0...v0.1.1) - 2026-02-24

### Added

- respect .gitignore with .worktreelinks override ([#7](https://github.com/km-tr/worktree-link/pull/7))

### Fixed

- *(ci)* add git_only option to release-plz config for unpublished packages ([#9](https://github.com/km-tr/worktree-link/pull/9))
- *(ci)* use PAT for release-plz PR creation ([#5](https://github.com/km-tr/worktree-link/pull/5))

### Other

- release v0.1.0 ([#6](https://github.com/km-tr/worktree-link/pull/6))

## [0.1.0](https://github.com/km-tr/worktree-link/releases/tag/v0.1.0) - 2026-02-24

### Added

- integrate release-plz with git-cliff for automated changelog generation ([#4](https://github.com/km-tr/worktree-link/pull/4))

### Other

- add dist profile with thin LTO for cargo-dist ([#3](https://github.com/km-tr/worktree-link/pull/3))
- *(dist)* switch to separate homebrew-tap repository ([#2](https://github.com/km-tr/worktree-link/pull/2))
- Implement worktree-link CLI tool ([#1](https://github.com/km-tr/worktree-link/pull/1))
- Initial commit
