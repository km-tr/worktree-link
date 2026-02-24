# worktree-link

A CLI tool that automatically creates symlinks from a main git worktree to a new worktree based on glob patterns.

Short alias `wtl` is also available.

## Use Cases

- Share `node_modules` across worktrees (avoid installing dependencies per worktree)
- Share environment files like `.env` / `.env.local`
- Share build caches and artifacts like `.next/`, `tmp/`, `dist/`
- Share IDE settings (`.idea/`, `.vscode/`)

## Installation

### Homebrew (macOS / Linux)

```bash
brew install km-tr/tap/worktree-link
```

### Shell script (macOS / Linux)

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/km-tr/worktree-link/releases/latest/download/worktree-link-installer.sh | sh
```

### From source

```bash
cargo install --path .
```

> **Note:** The `wtl` alias is only available when installed via Homebrew or the shell script installer. When building from source, only the `worktree-link` binary is installed.

## Usage

```text
worktree-link [OPTIONS]
wtl [OPTIONS]
```

### Options

| Option | Description | Default |
|--------|-------------|---------|
| `-s, --source <DIR>` | Source directory (main worktree) | Auto-detected via `git worktree list` |
| `-t, --target <DIR>` | Target directory (new worktree) | `.` (current directory) |
| `-c, --config <FILE>` | Path to config file | `<SOURCE>/.worktreelinks` |
| `-n, --dry-run` | Show what would be done without making changes | `false` |
| `-f, --force` | Overwrite existing files/symlinks | `false` |
| `-v, --verbose` | Enable verbose logging | `false` |
| `--unlink` | Remove symlinks previously created by worktree-link | `false` |
| `--no-ignore` | Do not respect .gitignore rules | `false` |

### Examples

```bash
# Create symlinks (auto-detect source from git, target is current directory)
wtl

# Specify the source directory explicitly
wtl -s /path/to/main

# Specify the target directory explicitly
wtl -t /path/to/feature-branch

# Specify both source and target
wtl -s /path/to/main -t ./feature-branch

# Preview with dry-run before creating links
wtl --dry-run

# Overwrite existing files/symlinks
wtl --force

# Remove previously created symlinks
wtl --unlink

# Disable .gitignore filtering
wtl --no-ignore
```

## Configuration (`.worktreelinks`)

Create a `.worktreelinks` file in your project root and list the files/directories to link using gitignore-compatible glob patterns.

```gitignore
# Dependencies
node_modules

# Environment variables
.env
.env.*

# Build artifacts and caches
.next/
tmp/
dist/

# IDE settings
.idea/
.vscode/settings.json

# Monorepo packages
packages/*/node_modules
```

### Pattern Rules

- Lines starting with `#` are comments
- Blank lines are ignored
- Patterns ending with `/` match directories only
- `*` matches any character except `/`
- `**` matches across directory boundaries
- Patterns starting with `!` are negation (exclusion) patterns

## Behavior

### Directory Linking

When a pattern matches a directory (e.g. `node_modules`), the entire directory is symlinked as a single unit rather than linking individual files inside it.

### Absolute Paths

Symlinks are created using absolute paths, making them resilient to worktree relocation.

### Safety

- The `.git/` directory is always excluded
- Existing files, symlinks, and directories are never overwritten unless `--force` is specified (directories are removed recursively)
- `--unlink` only removes symlinks that point into the source directory

## Platform Support

`worktree-link` uses Unix symlink APIs (`#[cfg(unix)]`). Non-Unix platforms (e.g. native Windows) are not supported. On Windows, use WSL or a similar Unix-like environment.

Currently only tested on macOS. Linux should work but is not regularly tested.

## Development

```bash
# Build
cargo build

# Test
cargo test

# Debug run
cargo run -- --dry-run -s /path/to/source -t /path/to/target
```

## License

MIT
