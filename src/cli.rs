// ============================================================================
// cli.rs — コマンドライン引数の定義
// ============================================================================
//
// 【clap クレートと derive マクロ】
// clap は Rust で最も広く使われるコマンドライン引数パーサーです。
// `#[derive(Parser)]` マクロを使うと、構造体の定義から自動的にパーサーが生成されます。
// 各フィールドの `///` コメントはそのまま `--help` の説明文として使われます。
//
// 【derive マクロとは？】
// `#[derive(...)]` は Rust のプロシージャルマクロの一種で、
// コンパイル時にコードを自動生成します。ここでは Parser トレイトの
// 実装が自動生成され、コマンドライン引数の解析コードを手書きする必要がなくなります。
// ============================================================================

use clap::Parser;
use std::path::PathBuf;

/// Automatically create symlinks between git worktrees based on .worktreelinks patterns.
#[derive(Parser, Debug)]
#[command(name = "worktree-link", version, about)]
pub struct Cli {
    // 【Option<T> 型】
    // Option<PathBuf> は「値があるかもしれないし、ないかもしれない」を表します。
    // - Some(値) → 値がある
    // - None     → 値がない
    // CLI では --source が指定されなかった場合に None になります。
    /// Source directory (main worktree).
    /// Auto-detected via `git worktree list` if omitted.
    #[arg(short, long)]
    pub source: Option<PathBuf>,

    // 【default_value】
    // `default_value = "."` はフラグが省略された場合のデフォルト値です。
    // "." はカレントディレクトリを意味します。
    /// Target directory (new worktree)
    #[arg(short, long, default_value = ".")]
    pub target: PathBuf,

    /// Path to config file [default: <SOURCE>/.worktreelinks]
    #[arg(short, long = "config")]
    pub config: Option<PathBuf>,

    // 【bool フラグ】
    // bool 型のフィールドは「フラグ」として扱われます。
    // `--dry-run` のように指定すると true、省略すると false になります。
    // `short = 'n'` は `-n` という短いフラグ名を指定しています。
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

    /// Don't respect .gitignore rules.
    /// By default, files matched by .gitignore are excluded unless
    /// they also match a .worktreelinks pattern.
    #[arg(long)]
    pub no_ignore: bool,
}
