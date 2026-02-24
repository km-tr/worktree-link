use anyhow::{Context, Result};
use ignore::overrides::{Override, OverrideBuilder};
use ignore::{Match, WalkBuilder};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::debug;

/// Build an `Override` matcher from the given patterns.
pub fn build_overrides(source: &Path, patterns: &[String]) -> Result<Override> {
    let mut builder = OverrideBuilder::new(source);
    for pattern in patterns {
        builder
            .add(pattern)
            .with_context(|| format!("Invalid pattern: {pattern}"))?;
    }
    builder.build().with_context(|| "Failed to build overrides")
}

/// Collect files and directories in `source` that match the given patterns.
///
/// Patterns follow gitignore syntax. When a directory matches, we include it
/// but do NOT descend into it — it will be symlinked as a whole.
pub fn collect_targets(source: &Path, patterns: &[String]) -> Result<Vec<PathBuf>> {
    let overrides = Arc::new(build_overrides(source, patterns)?);

    let mut targets: Vec<PathBuf> = Vec::new();

    // We use filter_entry to both skip .git and to prune matched directories
    // (avoid descending into them). A matched directory is still yielded as
    // an entry before filter_entry decides not to recurse into it, so we
    // collect it from filter_entry via a shared Vec.
    let matched_dirs: Arc<std::sync::Mutex<Vec<PathBuf>>> =
        Arc::new(std::sync::Mutex::new(Vec::new()));

    let overrides_clone = Arc::clone(&overrides);
    let matched_dirs_clone = Arc::clone(&matched_dirs);
    let source_owned = source.to_path_buf();

    let walker = WalkBuilder::new(source)
        .hidden(false)
        .git_ignore(false)
        .git_global(false)
        .git_exclude(false)
        .filter_entry(move |entry| {
            // Always skip .git
            if entry.file_name() == ".git" {
                return false;
            }

            let path = entry.path();

            // Always allow the root itself
            if path == source_owned {
                return true;
            }

            let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);

            if is_dir {
                if let Match::Whitelist(_) = overrides_clone.matched(path, true) {
                    // This directory matches — record it and stop descent
                    matched_dirs_clone.lock().unwrap().push(path.to_path_buf());
                    return false;
                }
            }

            true
        })
        .build();

    for entry in walker {
        let entry = entry.with_context(|| "Error walking directory")?;
        let path = entry.path();

        // Skip the source root itself
        if path == source {
            continue;
        }

        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);

        // Files matched by pattern
        if let Match::Whitelist(_) = overrides.matched(path, is_dir) {
            debug!("matched: {}", path.display());
            targets.push(path.to_path_buf());
        }
    }

    // Add matched directories that were pruned by filter_entry
    let dirs = matched_dirs.lock().unwrap();
    for dir in dirs.iter() {
        debug!("matched dir: {}", dir.display());
        targets.push(dir.clone());
    }

    // Sort for deterministic output
    targets.sort();
    targets.dedup();

    Ok(targets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn collect_targets_skips_unmatched() {
        let dir = tempdir("collect_skip");
        fs::write(dir.join(".env"), "SECRET=1").unwrap();
        fs::write(dir.join("main.rs"), "fn main(){}").unwrap();
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(dir.join("src/lib.rs"), "").unwrap();

        let targets = collect_targets(&dir, &[".env".into()]).unwrap();
        let rel: Vec<_> = targets
            .iter()
            .map(|p| p.strip_prefix(&dir).unwrap())
            .collect();
        assert_eq!(rel, vec![Path::new(".env")]);
    }

    #[test]
    fn collect_targets_matches_directory_without_descending() {
        let dir = tempdir("collect_dir");
        let nm = dir.join("node_modules/pkg");
        fs::create_dir_all(&nm).unwrap();
        fs::write(nm.join("index.js"), "").unwrap();
        fs::write(dir.join("app.js"), "").unwrap();

        let targets = collect_targets(&dir, &["node_modules".into()]).unwrap();
        let rel: Vec<_> = targets
            .iter()
            .map(|p| p.strip_prefix(&dir).unwrap())
            .collect();
        // Should only contain the directory itself, not its children
        assert_eq!(rel, vec![Path::new("node_modules")]);
    }

    #[test]
    fn collect_targets_negation_pattern() {
        let dir = tempdir("collect_neg");
        fs::write(dir.join(".env"), "A=1").unwrap();
        fs::write(dir.join(".env.local"), "B=2").unwrap();
        fs::write(dir.join(".env.production"), "C=3").unwrap();

        let targets = collect_targets(
            &dir,
            &[".env".into(), ".env.*".into(), "!.env.production".into()],
        )
        .unwrap();
        let rel: Vec<_> = targets
            .iter()
            .map(|p| p.strip_prefix(&dir).unwrap())
            .collect();
        assert_eq!(rel, vec![Path::new(".env"), Path::new(".env.local")]);
    }

    fn tempdir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("worktree-link-test-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        // Return canonical path so comparisons work
        fs::canonicalize(&dir).unwrap()
    }
}
