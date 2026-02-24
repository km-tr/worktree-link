use clap::Parser;
use std::path::PathBuf;

/// Automatically create symlinks between git worktrees based on .worktreelinks patterns.
#[derive(Parser, Debug)]
#[command(name = "worktree-link", version, about)]
pub struct Cli {
    /// Source directory (main worktree)
    pub source: PathBuf,

    /// Target directory (new worktree) [default: current directory]
    #[arg(default_value = ".")]
    pub target: PathBuf,

    /// Path to config file [default: <SOURCE>/.worktreelinks]
    #[arg(short, long = "config")]
    pub config: Option<PathBuf>,

    /// Show what would be done without making changes
    #[arg(short = 'n', long)]
    pub dry_run: bool,

    /// Overwrite existing files/symlinks
    #[arg(short, long)]
    pub force: bool,

    /// Enable verbose logging
    #[arg(short, long)]
    pub verbose: bool,

    /// Remove symlinks previously created by worktree-link
    #[arg(long)]
    pub unlink: bool,
}

