// ============================================================================
// main.rs — アプリケーションのエントリポイント
// ============================================================================
//
// このファイルはプログラム全体の流れを制御する「司令塔」です。
// 各モジュール（cli, config, git, linker, walker）を組み合わせて、
// シンボリックリンクの作成・削除を行います。
//
// 【Rust のモジュールシステム】
// `mod xxx;` は「xxx.rs ファイルをこのクレートの一部として読み込む」という宣言です。
// これにより、各モジュールの公開（pub）アイテムを `xxx::関数名` で使えるようになります。
// ============================================================================

mod cli;
mod config;
mod git;
mod linker;
mod walker;

// 【use 宣言の整理】
// Rust では use 宣言を以下の順序で書くのが慣例です：
//   1. 外部クレート（anyhow, clap, colored など）
//   2. 標準ライブラリ（std::fs など）
//   3. 自クレート内のモジュール（cli::Cli など）
use anyhow::{bail, Context, Result};
use clap::Parser;
use colored::Colorize;
use std::fs;

use cli::Cli;
use config::Config;

// 【fn main() -> Result<()>】
// Rust の main 関数は通常 `()` を返しますが、`Result<()>` を返すこともできます。
// これにより、`?` 演算子を使ってエラーを簡潔に処理できます。
// エラーが発生すると、エラーメッセージが自動的に表示されてプログラムが終了します。
fn main() -> Result<()> {
    // 【clap の derive マクロ】
    // `Cli::parse()` は clap の derive マクロが自動生成したパーサーを呼び出します。
    // コマンドライン引数を解析し、Cli 構造体のインスタンスを返します。
    // 引数が不正な場合は、ヘルプメッセージを表示して自動的に終了します。
    let cli = Cli::parse();

    // 【tracing によるログ設定】
    // tracing はRust界で標準的なログ/トレーシングフレームワークです。
    // RUST_LOG 環境変数が設定されていればそれを使い、なければ --verbose フラグに
    // 応じてログレベルを切り替えます（verbose: debug レベル、通常: warn レベル）。
    let level = if cli.verbose { "debug" } else { "warn" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(level)),
        )
        .without_time()
        .init();

    // 【fs::canonicalize — パスの正規化】
    // 相対パスを絶対パスに変換し、シンボリックリンクも解決します。
    // 例: "./my_dir" → "/home/user/projects/my_dir"
    // パスが存在しない場合はエラーを返します。
    //
    // 【? 演算子と .with_context()】
    // `?` は Result が Err の場合にその場で関数からエラーを返す糖衣構文です。
    // `.with_context(|| ...)` は anyhow のメソッドで、エラーにわかりやすい説明を追加します。
    let target = fs::canonicalize(&cli.target)
        .with_context(|| format!("Target directory does not exist: {}", cli.target.display()))?;

    // 【bail! マクロ】
    // anyhow の `bail!` はエラーを生成して即座に関数から返すマクロです。
    // `return Err(anyhow!("..."))` の省略形です。
    if !target.is_dir() {
        bail!("Target is not a directory: {}", target.display());
    }

    // 【Option の match パターン】
    // `cli.source` は `Option<PathBuf>` 型です。
    // - `Some(s)` → ユーザーが --source で指定した場合
    // - `None`    → 指定がない場合、git コマンドで自動検出
    let source = match cli.source {
        Some(s) => {
            let resolved = fs::canonicalize(&s)
                .with_context(|| format!("Source directory does not exist: {}", s.display()))?;
            if !resolved.is_dir() {
                bail!("Source is not a directory: {}", resolved.display());
            }
            resolved
        }
        None => git::detect_main_worktree_in(&target)?,
    };

    // ソースとターゲットが同じ、またはネストしている場合はエラー
    // （自分自身にリンクを張ったり、無限ループになるのを防ぐ）
    if source == target {
        bail!("Source and target cannot be the same directory");
    }

    if target.starts_with(&source) || source.starts_with(&target) {
        bail!("Source and target must not be nested");
    }

    if cli.dry_run {
        println!("{}", "DRY RUN — no changes will be made".cyan().bold());
    }

    // 【リンクモード vs アンリンクモード】
    // --unlink フラグの有無で2つのモードに分岐します。
    if cli.unlink {
        // === アンリンクモード ===
        // ターゲットディレクトリ内を走査して、ソースを指すシンボリックリンクを削除します。
        // 設定ファイルは不要 — ターゲット内のリンク先を見て判断します。
        let actions = linker::unlink_targets(&source, &target, cli.dry_run)?;

        // 【変数の可変性（mut）】
        // Rust では変数はデフォルトで不変（immutable）です。
        // 値を変更したい場合は `mut` キーワードが必要です。
        let mut removed = 0;
        let mut skipped = 0;
        for action in &actions {
            println!("  {action}");
            // 【match 式による enum の分岐】
            // Rust の match は全てのバリアントを網羅する必要があります（網羅性チェック）。
            // これにより、パターンの追加忘れをコンパイル時に検出できます。
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
        // === リンクモード ===
        // 設定ファイルを読み込み、パターンに一致するファイルのシンボリックリンクを作成します。

        // 【unwrap_or_else — Option のデフォルト値】
        // config が None（未指定）の場合、ソースディレクトリ直下の ".worktreelinks" をデフォルトとして使います。
        // unwrap_or_else はクロージャを受け取るため、デフォルト値の計算が遅延評価されます。
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

        // walker でソースディレクトリからパターンに一致するファイル/ディレクトリを収集
        let targets = walker::collect_targets(&source, &config.patterns, cli.no_ignore)?;

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
            // 【strip_prefix — パスの相対部分を取得】
            // ソースパスからソースルートを取り除いて相対パスを得ます。
            // 例: "/home/main/node_modules" - "/home/main" → "node_modules"
            // その相対パスをターゲットディレクトリに結合してリンク先を決定します。
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
