use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

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
/// `source_path` is the file/dir that already exists (in the main worktree).
/// `target_path` is where the symlink will be created (in the new worktree).
pub fn create_link(
    source_path: &Path,
    target_path: &Path,
    force: bool,
    dry_run: bool,
) -> Result<LinkAction> {
    let source_abs = fs::canonicalize(source_path)
        .with_context(|| format!("Cannot resolve source path: {}", source_path.display()))?;

    debug!(
        "create_link: {} -> {}",
        target_path.display(),
        source_abs.display()
    );

    if target_path.exists() || target_path.is_symlink() {
        if !force {
            return Ok(LinkAction::Skipped {
                target: target_path.to_path_buf(),
                reason: "already exists (use --force to overwrite)".into(),
            });
        }

        if !dry_run {
            remove_entry(target_path)
                .with_context(|| format!("Failed to remove: {}", target_path.display()))?;
        }

        if !dry_run {
            create_parent_dirs(target_path)?;
            symlink(&source_abs, target_path)?;
        }

        info!("overwritten: {}", target_path.display());
        return Ok(LinkAction::Overwritten {
            source: source_abs,
            target: target_path.to_path_buf(),
        });
    }

    if !dry_run {
        create_parent_dirs(target_path)?;
        symlink(&source_abs, target_path)?;
    }

    info!("linked: {}", target_path.display());
    Ok(LinkAction::Created {
        source: source_abs,
        target: target_path.to_path_buf(),
    })
}

/// Remove symlinks in `target_dir` that point into `source_dir` and match the given patterns.
pub fn unlink_targets(
    source_dir: &Path,
    target_dir: &Path,
    matched_targets: &[PathBuf],
    dry_run: bool,
) -> Result<Vec<UnlinkAction>> {
    let source_abs = fs::canonicalize(source_dir)
        .with_context(|| format!("Cannot resolve source path: {}", source_dir.display()))?;

    let mut actions = Vec::new();

    for source_path in matched_targets {
        let rel = source_path
            .strip_prefix(source_dir)
            .with_context(|| "Path is not relative to source")?;
        let target_path = target_dir.join(rel);

        if !target_path.is_symlink() {
            actions.push(UnlinkAction::Skipped {
                target: target_path,
                reason: "not a symlink".into(),
            });
            continue;
        }

        // Verify the symlink points into the source directory
        let link_dest = fs::read_link(&target_path)
            .with_context(|| format!("Failed to read symlink: {}", target_path.display()))?;
        if !link_dest.starts_with(&source_abs) {
            actions.push(UnlinkAction::Skipped {
                target: target_path,
                reason: format!("points to {} (not in source)", link_dest.display()),
            });
            continue;
        }

        if !dry_run {
            remove_entry(&target_path)
                .with_context(|| format!("Failed to remove symlink: {}", target_path.display()))?;
        }

        info!("unlinked: {}", target_path.display());
        actions.push(UnlinkAction::Removed(target_path));
    }

    Ok(actions)
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
    std::os::unix::fs::symlink(source, target)
        .with_context(|| format!("Failed to create symlink: {} -> {}", target.display(), source.display()))
}

#[cfg(not(unix))]
fn symlink(source: &Path, target: &Path) -> Result<()> {
    anyhow::bail!("Symlink creation is only supported on Unix systems")
}
