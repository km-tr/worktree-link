# Agents

Instructions for AI agents working on this repository.

## PR Guidelines

This repository uses squash merge. The PR title becomes the final commit message and is used to generate the changelog via release-plz.

### Title format

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>: <description>
```

Types:

- `feat` — New feature
- `fix` — Bug fix
- `docs` — Documentation only
- `refactor` — Code restructuring without behavior change
- `test` — Adding or updating tests
- `chore` — Maintenance, tooling, CI, dependencies

Examples:

```
feat: add wtl short alias via bin-aliases
fix: canonicalize symlink targets before prefix matching
docs: update Homebrew installation instructions
chore: add dist profile with thin LTO
```
