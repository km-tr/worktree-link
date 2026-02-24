use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Describes what happened when attempting to create a link.
#[derive(Debug, PartialEq)]
pub enum LinkAction {
    Created { source: PathBuf, target: PathBuf },
    Skipped { target: PathBuf, reason: String },
    Overwritten { source: PathBuf, target: PathBuf },
}

/// Describes what happened when attempting to unlink.
#[derive(Debug, PartialEq)]
pub enum UnlinkAction {
    Removed(PathBuf),
    Skipped { target: PathBuf, reason: String },
}

impl std::fmt::Display for LinkAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinkAction::Created { source, target } => {
                write!(
                    f,
                    "{} {} -> {}",
                    "LINK".green().bold(),
                    target.display(),
                    source.display()
                )
            }
            LinkAction::Skipped { target, reason } => {
                write!(
                    f,
                    "{} {} ({})",
                    "SKIP".yellow().bold(),
                    target.display(),
                    reason
                )
            }
            LinkAction::Overwritten { source, target } => {
                write!(
                    f,
                    "{} {} -> {}",
                    "OVERWRITE".magenta().bold(),
                    target.display(),
                    source.display()
                )
            }
        }
    }
}

impl std::fmt::Display for UnlinkAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnlinkAction::Removed(path) => {
                write!(f, "{} {}", "UNLINK".red().bold(), path.display())
            }
            UnlinkAction::Skipped { target, reason } => {
                write!(
                    f,
                    "{} {} ({})",
                    "SKIP".yellow().bold(),
                    target.display(),
                    reason
                )
            }
        }
    }
}

/// Create a symlink from `source_path` to `target_path`.
///
/// `source_path` must be an absolute path (under the canonical source root).
/// We intentionally do NOT call `fs::canonicalize` on it so that symlinks
/// within the source tree are preserved as-is, keeping the link/unlink
/// round-trip consistent.
///
/// `target_path` is where the symlink will be created (in the new worktree).
pub fn create_link(
    source_path: &Path,
    target_path: &Path,
    force: bool,
    dry_run: bool,
) -> Result<LinkAction> {
    debug!(
        "create_link: {} -> {}",
        target_path.display(),
        source_path.display()
    );

    if target_path.exists() || target_path.is_symlink() {
        if !force {
            return Ok(LinkAction::Skipped {
                target: target_path.to_path_buf(),
                reason: "already exists (use --force to overwrite)".into(),
            });
        }

        // Guard: if any parent of target_path is a symlink, removing it
        // would delete through the symlink into the real source data.
        if has_symlink_parent(target_path) {
            return Ok(LinkAction::Skipped {
                target: target_path.to_path_buf(),
                reason: "parent directory is a symlink (remove it first)".into(),
            });
        }

        if !dry_run {
            remove_entry(target_path)
                .with_context(|| format!("Failed to remove: {}", target_path.display()))?;
            create_parent_dirs(target_path)?;
            symlink(source_path, target_path)?;
        }

        info!("overwritten: {}", target_path.display());
        return Ok(LinkAction::Overwritten {
            source: source_path.to_path_buf(),
            target: target_path.to_path_buf(),
        });
    }

    if !dry_run {
        create_parent_dirs(target_path)?;
        symlink(source_path, target_path)?;
    }

    info!("linked: {}", target_path.display());
    Ok(LinkAction::Created {
        source: source_path.to_path_buf(),
        target: target_path.to_path_buf(),
    })
}

/// Walk `target_dir` and remove any symlinks that point into `source_dir`.
///
/// This walks the target side (not the source), so it also catches stale
/// symlinks whose source-side originals have been deleted or renamed.
/// Errors on individual entries are logged as warnings and skipped so that
/// the walk continues (best-effort).
pub fn unlink_targets(
    source_dir: &Path,
    target_dir: &Path,
    dry_run: bool,
) -> Result<Vec<UnlinkAction>> {
    let mut actions = Vec::new();

    walk_symlinks(target_dir, &mut |entry_path| {
        if !entry_path.is_symlink() {
            return Ok(());
        }

        let link_dest = match fs::read_link(&entry_path) {
            Ok(dest) => dest,
            Err(e) => {
                warn!("Skipping {}: {e}", entry_path.display());
                actions.push(UnlinkAction::Skipped {
                    target: entry_path,
                    reason: format!("cannot read symlink: {e}"),
                });
                return Ok(());
            }
        };

        // Resolve relative symlink targets to absolute paths for comparison.
        // fs::read_link can return relative paths, while source_dir is canonical.
        let resolved = if link_dest.is_absolute() {
            link_dest
        } else {
            match entry_path.parent() {
                Some(parent) => parent.join(&link_dest),
                None => link_dest,
            }
        };

        // Only remove symlinks that point into the source directory
        if !resolved.starts_with(source_dir) {
            return Ok(());
        }

        if !dry_run {
            if let Err(e) = remove_entry(&entry_path) {
                warn!("Failed to remove {}: {e}", entry_path.display());
                actions.push(UnlinkAction::Skipped {
                    target: entry_path,
                    reason: format!("removal failed: {e}"),
                });
                return Ok(());
            }
        }

        info!("unlinked: {}", entry_path.display());
        actions.push(UnlinkAction::Removed(entry_path));
        Ok(())
    })?;

    actions.sort_by(|a, b| {
        let path_a = match a {
            UnlinkAction::Removed(p) | UnlinkAction::Skipped { target: p, .. } => p,
        };
        let path_b = match b {
            UnlinkAction::Removed(p) | UnlinkAction::Skipped { target: p, .. } => p,
        };
        path_a.cmp(path_b)
    });

    Ok(actions)
}

/// Recursively walk a directory, calling `visitor` on each symlink found.
/// Does not follow symlinks (so symlinked directories are visited but not descended into).
/// Errors on individual entries are warned and skipped (best-effort).
fn walk_symlinks(dir: &Path, visitor: &mut dyn FnMut(PathBuf) -> Result<()>) -> Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            warn!("Skipping directory {}: {e}", dir.display());
            return Ok(());
        }
        Err(e) => return Err(e).with_context(|| format!("Failed to read dir: {}", dir.display())),
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                warn!("Skipping entry: {e}");
                continue;
            }
        };
        let path = entry.path();
        let meta = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) => {
                warn!("Skipping {}: {e}", path.display());
                continue;
            }
        };

        if meta.file_type().is_symlink() {
            visitor(path)?;
        } else if meta.is_dir() {
            walk_symlinks(&path, visitor)?;
        }
    }

    Ok(())
}

/// Check if any parent component of `path` is a symlink.
fn has_symlink_parent(path: &Path) -> bool {
    let mut current = path.to_path_buf();
    while let Some(parent) = current.parent() {
        if parent.as_os_str().is_empty() {
            break;
        }
        if parent.is_symlink() {
            return true;
        }
        current = parent.to_path_buf();
    }
    false
}

fn create_parent_dirs(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create parent directory: {}", parent.display()))?;
    }
    Ok(())
}

fn remove_entry(path: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if meta.is_dir() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }
    Ok(())
}

#[cfg(unix)]
fn symlink(source: &Path, target: &Path) -> Result<()> {
    std::os::unix::fs::symlink(source, target).with_context(|| {
        format!(
            "Failed to create symlink: {} -> {}",
            target.display(),
            source.display()
        )
    })
}

#[cfg(not(unix))]
fn symlink(source: &Path, target: &Path) -> Result<()> {
    anyhow::bail!("Symlink creation is only supported on Unix systems")
}
