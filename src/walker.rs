use anyhow::{Context, Result};
use ignore::overrides::OverrideBuilder;
use ignore::{Match, WalkBuilder};
use std::path::{Path, PathBuf};
use tracing::debug;

/// Collect files and directories in `source` that match the given patterns.
///
/// Patterns follow gitignore syntax. Directories that match are returned as-is
/// (we do NOT descend into them — they will be symlinked as a whole).
pub fn collect_targets(source: &Path, patterns: &[String]) -> Result<Vec<PathBuf>> {
    let mut builder = OverrideBuilder::new(source);
    for pattern in patterns {
        builder
            .add(pattern)
            .with_context(|| format!("Invalid pattern: {pattern}"))?;
    }
    let overrides = builder.build().with_context(|| "Failed to build overrides")?;

    let mut targets: Vec<PathBuf> = Vec::new();

    // Walk the source directory without any built-in filtering.
    // We apply the override matcher manually:
    //   Match::Whitelist = pattern matched → include (link this file)
    //   Match::Ignore    = no pattern matched → skip
    //   Match::None      = no overrides relevant → skip
    let walker = WalkBuilder::new(source)
        .hidden(false)
        .git_ignore(false)
        .git_global(false)
        .git_exclude(false)
        .filter_entry(|entry| entry.file_name() != ".git")
        .build();

    for entry in walker {
        let entry = entry.with_context(|| "Error walking directory")?;
        let path = entry.path();

        // Skip the source root itself
        if path == source {
            continue;
        }

        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);

        match overrides.matched(path, is_dir) {
            Match::Whitelist(_) => {
                debug!("matched: {}", path.display());
                targets.push(path.to_path_buf());
            }
            _ => {}
        }
    }

    // Sort for deterministic output
    targets.sort();

    Ok(targets)
}

/// Given the full list of matched paths, collapse them so that if a directory
/// is matched, none of its children appear in the result. This way we symlink
/// the directory as a unit.
pub fn collapse_directories(targets: Vec<PathBuf>) -> Vec<PathBuf> {
    if targets.is_empty() {
        return targets;
    }

    let mut collapsed: Vec<PathBuf> = Vec::new();

    for path in &targets {
        // If the last collapsed entry is an ancestor of this path, skip it.
        if let Some(last) = collapsed.last() {
            if path.starts_with(last) {
                continue;
            }
        }
        collapsed.push(path.clone());
    }

    collapsed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapse_removes_children() {
        let input = vec![
            PathBuf::from("/a/node_modules"),
            PathBuf::from("/a/node_modules/foo"),
            PathBuf::from("/a/node_modules/foo/bar"),
            PathBuf::from("/a/src"),
        ];
        let result = collapse_directories(input);
        assert_eq!(
            result,
            vec![PathBuf::from("/a/node_modules"), PathBuf::from("/a/src")]
        );
    }

    #[test]
    fn collapse_empty() {
        assert!(collapse_directories(vec![]).is_empty());
    }
}
