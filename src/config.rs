// ============================================================================
// config.rs — 設定ファイル（.worktreelinks）の読み込みと解析
// ============================================================================
//
// .worktreelinks ファイルは .gitignore と同じ書式で、
// リンク対象のファイル/ディレクトリパターンを1行ずつ記述します。
//
// 例:
//   # 共有する設定ファイル
//   .env
//   .env.*
//   node_modules
//   !.env.production    ← 否定パターン（除外）
// ============================================================================

use anyhow::{Context, Result};
use std::path::Path;

/// .worktreelinks ファイルを解析した結果を保持する構造体。
///
/// 【#[derive(Debug)]】
/// Debug トレイトを自動実装すると、`println!("{:?}", config)` のように
/// 構造体の中身をデバッグ出力できるようになります。
#[derive(Debug)]
pub struct Config {
    /// リンク対象を選択する glob パターンの一覧。
    ///
    /// 【Vec<String>】
    /// Vec は Rust の可変長配列（他言語の ArrayList や Array に相当）。
    /// String はヒープに確保された可変長の文字列です。
    pub patterns: Vec<String>,
}

// 【impl ブロック】
// Rust では構造体にメソッドを追加するために `impl` ブロックを使います。
// Java のクラス定義や Python の class 内メソッドに近い概念ですが、
// Rust では `self` を取る「メソッド」と、取らない「関連関数」（例: Config::parse()）
// の2種類があり、後者はコンストラクタ的な用途でよく使われます。
impl Config {
    /// 指定パスの `.worktreelinks` ファイルを読み込んで Config を生成します。
    ///
    /// 【&Path と PathBuf の違い】
    /// - `&Path` は借用（参照）— 読み取り専用で所有権を持たない（&str に似ている）
    /// - `PathBuf` は所有型 — 自由に変更でき、値を所有する（String に似ている）
    /// - 関数の引数には借用（&Path）を使うのが一般的で、呼び出し側の所有権を奪いません。
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;
        // 【Self は Config 自身の型を指すエイリアス】
        Ok(Self::parse(&content))
    }

    /// .worktreelinks の内容を解析してパターンを抽出します。
    /// `#` で始まる行はコメント、空行は無視されます。
    ///
    /// 【イテレータチェーン】
    /// Rust のイテレータは遅延評価（lazy）で、.collect() が呼ばれるまで
    /// 実際の処理は行われません。各メソッドの役割：
    ///   .lines()          → 文字列を行ごとに分割
    ///   .map(|line| ...)  → 各行に変換処理を適用
    ///   .filter(|line|...)→ 条件に合う行だけを残す
    ///   .collect()        → 結果を Vec<String> に収集
    fn parse(content: &str) -> Self {
        let patterns = content
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(|line| line.to_string())
            .collect();
        Config { patterns }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ignores_comments_and_blank_lines() {
        let input = r#"
# This is a comment
node_modules

.env
.env.*

# Build artifacts
.next/
dist/
        "#;
        let config = Config::parse(input);
        assert_eq!(
            config.patterns,
            vec!["node_modules", ".env", ".env.*", ".next/", "dist/"]
        );
    }

    #[test]
    fn parse_empty_file() {
        let config = Config::parse("");
        assert!(config.patterns.is_empty());
    }

    #[test]
    fn parse_only_comments() {
        let config = Config::parse("# comment\n# another");
        assert!(config.patterns.is_empty());
    }
}
