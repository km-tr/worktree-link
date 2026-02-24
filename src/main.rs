mod cli;
mod config;
mod linker;
mod walker;

use anyhow::{bail, Context, Result};
use clap::Parser;
use colored::Colorize;
use std::fs;

use cli::Cli;
use config::Config;

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Set up tracing
    let level = if cli.verbose { "debug" } else { "warn" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(level)),
        )
        .without_time()
        .init();

    // Resolve source directory
    let source = fs::canonicalize(&cli.source)
        .with_context(|| format!("Source directory does not exist: {}", cli.source.display()))?;

    if !source.is_dir() {
        bail!("Source is not a directory: {}", source.display());
    }

    // Resolve target directory
    let target = fs::canonicalize(&cli.target)
        .with_context(|| format!("Target directory does not exist: {}", cli.target.display()))?;

    if !target.is_dir() {
        bail!("Target is not a directory: {}", target.display());
    }

    if source == target {
        bail!("Source and target cannot be the same directory");
    }

    if target.starts_with(&source) || source.starts_with(&target) {
        bail!("Source and target must not be nested");
    }

    if cli.dry_run {
        println!("{}", "DRY RUN — no changes will be made".cyan().bold());
    }

    if cli.unlink {
        // Unlink mode: walk the target directory looking for symlinks into source.
        // No config file needed — we scan target for any symlink pointing into source.
        let actions = linker::unlink_targets(&source, &target, cli.dry_run)?;

        let mut removed = 0;
        let mut skipped = 0;
        for action in &actions {
            println!("  {action}");
            match action {
                linker::UnlinkAction::Removed(_) => removed += 1,
                linker::UnlinkAction::Skipped { .. } => skipped += 1,
            }
        }

        if actions.is_empty() {
            println!(
                "  {} No symlinks pointing to source found",
                "INFO".cyan().bold()
            );
        }

        println!();
        println!(
            "{}",
            format!("Removed: {removed}, Skipped: {skipped}").bold()
        );
    } else {
        // Link mode: read config and collect matching files/directories from source
        let config_path = cli
            .config
            .clone()
            .unwrap_or_else(|| source.join(".worktreelinks"));
        let config = Config::from_file(&config_path)?;

        if config.patterns.is_empty() {
            println!(
                "{} No patterns found in {}",
                "WARN".yellow().bold(),
                config_path.display()
            );
            return Ok(());
        }

        let targets = walker::collect_targets(&source, &config.patterns)?;

        if targets.is_empty() {
            println!(
                "{} No files matched the patterns in {}",
                "WARN".yellow().bold(),
                config_path.display()
            );
            return Ok(());
        }

        if cli.verbose {
            println!("Found {} target(s) to link", targets.len());
        }

        let mut created = 0;
        let mut overwritten = 0;
        let mut skipped = 0;

        for source_path in &targets {
            let rel = source_path
                .strip_prefix(&source)
                .with_context(|| "Path is not relative to source")?;
            let target_path = target.join(rel);

            let action = linker::create_link(source_path, &target_path, cli.force, cli.dry_run)?;

            println!("  {action}");
            match action {
                linker::LinkAction::Created { .. } => created += 1,
                linker::LinkAction::Overwritten { .. } => overwritten += 1,
                linker::LinkAction::Skipped { .. } => skipped += 1,
            }
        }

        println!();
        println!(
            "{}",
            format!("Created: {created}, Overwritten: {overwritten}, Skipped: {skipped}").bold()
        );
    }

    Ok(())
}
