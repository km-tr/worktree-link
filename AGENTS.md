# Agents

Instructions for AI agents working on this repository.

## PR Guidelines

This repository uses squash merge. The PR title becomes the final commit message and is used to generate the changelog via release-plz.

Write all PR content (title, description, comments) in English.

### Title format

Follow Conventional Commits:

```
<type>: <description>
```

Types:

- `feat` — New feature
- `fix` — Bug fix
- `docs` — Documentation only
- `refactor` — Code restructuring without behavior change
- `test` — Adding or updating tests
- `perf` — Performance improvement
- `style` — Formatting, whitespace (no logic change)
- `ci` — CI configuration changes
- `build` — Build system or external dependencies
- `revert` — Revert a previous commit
- `chore` — Maintenance, tooling, dependencies

Examples:

```
feat: add wtl short alias via bin-aliases
fix: canonicalize symlink targets before prefix matching
docs: update Homebrew installation instructions
chore: add dist profile with thin LTO
```
