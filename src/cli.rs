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

// 【/// ドキュメントコメントと clap の連携】
// `///` で始まるコメントは「ドキュメントコメント（doc comment）」と呼ばれ、
// Rust の公式ドキュメント生成ツール（rustdoc）で HTML ドキュメントに変換されます。
// clap の derive マクロはこの doc comment をそのまま `--help` の説明文として使います。
// そのため、ここでは英語のまま残しています（CLIのヘルプ表示に影響するため）。

/// .worktreelinks のパターンに基づいて、git worktree 間のシンボリックリンクを自動作成します。
///
// 【#[derive(Parser, Debug)]】
// 複数のトレイトを同時に derive できます。Parser は clap 用、Debug はデバッグ表示用です。
//
// 【#[command(...)] 属性】
// clap のコマンド全体の設定を行う属性です。
// - `name` → コマンド名（`worktree-link --help` で表示される名前）
// - `version` → Cargo.toml の version が自動で使われます
// - `about` → 上の doc comment が自動で使われます
#[derive(Parser, Debug)]
#[command(name = "worktree-link", version, about)]
// 【pub struct — 公開構造体】
// `pub` は他のモジュールからアクセスできることを意味します。
// main.rs の `Cli::parse()` で使えるのは `pub` だからです。
// `pub` がない場合、このモジュール内でしか使えません。
pub struct Cli {
    // 【Option<T> 型】
    // Option<PathBuf> は「値があるかもしれないし、ないかもしれない」を表します。
    // - Some(値) → 値がある
    // - None     → 値がない
    // CLI では --source が指定されなかった場合に None になります。
    //
    // 【#[arg(short, long)]】
    // - `short` → `-s` のような1文字のショートフラグを自動生成（フィールド名の頭文字）
    // - `long`  → `--source` のようなロングフラグを自動生成（フィールド名がそのまま使われる）
    /// ソースディレクトリ（メインワークツリー）。
    /// 省略時は `git worktree list` で自動検出します。
    #[arg(short, long)]
    pub source: Option<PathBuf>,

    // 【default_value】
    // `default_value = "."` はフラグが省略された場合のデフォルト値です。
    // "." はカレントディレクトリを意味します。
    // Option<T> ではなく PathBuf 型なので、必ず値が入ります。
    /// ターゲットディレクトリ（新しいワークツリー）
    #[arg(short, long, default_value = ".")]
    pub target: PathBuf,

    // 【long = "config" — フラグ名の手動指定】
    // デフォルトではフィールド名がそのままフラグ名になりますが、
    // `long = "config"` で明示的に指定することもできます。
    /// 設定ファイルのパス [デフォルト: <SOURCE>/.worktreelinks]
    #[arg(short, long = "config")]
    pub config: Option<PathBuf>,

    // 【bool フラグ】
    // bool 型のフィールドは「フラグ」として扱われます。
    // `--dry-run` のように指定すると true、省略すると false になります。
    // `short = 'n'` は `-n` という短いフラグ名を手動指定しています
    // （`d` は --dry-run の `d` と衝突するため `n` を使う）。
    /// 変更を加えずに実行内容を表示します（ドライラン）
    #[arg(short = 'n', long)]
    pub dry_run: bool,

    /// 既存のファイル/シンボリックリンクを上書きします
    #[arg(short, long)]
    pub force: bool,

    /// 詳細なログ出力を有効にします
    #[arg(short, long)]
    pub verbose: bool,

    // 【long のみ（short なし）】
    // `#[arg(long)]` だけを指定すると、ショートフラグは生成されません。
    // 頻繁に使わないオプションや、誤操作を防ぎたいオプションに適しています。
    /// worktree-link で作成したシンボリックリンクを削除します
    #[arg(long)]
    pub unlink: bool,

    /// .gitignore のルールを無視します。
    /// デフォルトでは .gitignore にマッチするファイルは除外されますが、
    /// .worktreelinks パターンにもマッチする場合はリンクされます。
    #[arg(long)]
    pub no_ignore: bool,
}
