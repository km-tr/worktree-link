// ============================================================================
// walker.rs — パターンマッチによるファイル/ディレクトリの収集
// ============================================================================
//
// このモジュールは `ignore` クレートを使って、ソースディレクトリ内から
// .worktreelinks のパターンに一致するファイルとディレクトリを収集します。
//
// 【ignore クレート】
// ripgrep の作者が開発した高速なファイル走査ライブラリです。
// .gitignore のルールを理解し、パフォーマンスに優れています。
// 「Override」は .gitignore のルールを上書きする仕組みで、
// 通常は無視されるファイルも強制的にマッチさせることができます。
//
// 【重要な設計判断：ディレクトリマッチ時の挙動】
// ディレクトリがパターンにマッチした場合、そのディレクトリ自体を
// リンク対象として記録し、中には降りません（ディレクトリごとリンクされるため）。
// 例: "node_modules" にマッチ → node_modules ディレクトリ全体が1つのリンクになる
// ============================================================================

use anyhow::{Context, Result};
use ignore::overrides::{Override, OverrideBuilder};
use ignore::{Match, WalkBuilder};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::debug;

/// パターン文字列の配列から Override マッチャーを構築します。
///
/// 【&[String] — スライス参照】
/// `&[String]` は String の配列（Vec<String> など）への借用参照です。
/// Vec でも固定長配列でも受け取れる柔軟な型です。
/// 関数の引数には Vec<String> より &[String] を使うのが Rust のイディオムです。
pub fn build_overrides(source: &Path, patterns: &[String]) -> Result<Override> {
    let mut builder = OverrideBuilder::new(source);
    for pattern in patterns {
        builder
            .add(pattern)
            .with_context(|| format!("Invalid pattern: {pattern}"))?;
    }
    builder.build().with_context(|| "Failed to build overrides")
}

/// ソースディレクトリ内でパターンに一致するファイル/ディレクトリを収集します。
///
/// パターンは gitignore 構文に従います。ディレクトリがマッチした場合は
/// ディレクトリ自体を結果に含めますが、その中には降りません
/// （ディレクトリ全体がシンボリックリンクされるため）。
pub fn collect_targets(
    source: &Path,
    patterns: &[String],
    no_ignore: bool,
) -> Result<Vec<PathBuf>> {
    let overrides = build_overrides(source, patterns)?;
    // 【.clone() — 値の複製】
    // Override を clone（複製）しているのは、1つは WalkBuilder に渡し（所有権を移動）、
    // もう1つはメインループでのマッチング判定に使うためです。
    // Rust の所有権システムでは、1つの値を2箇所で使う場合、
    // clone するか参照を使うかの選択が必要です。
    let walker_overrides = overrides.clone();
    // 【Arc（Atomic Reference Counting）— スレッド安全な参照カウント】
    // Arc は複数の所有者でデータを共有するためのスマートポインタです。
    // Rc と似ていますが、Arc はスレッド間で安全に共有できます。
    // Arc::clone() は実際のデータをコピーせず、参照カウントを増やすだけです。
    let overrides = Arc::new(overrides);

    let mut targets: Vec<PathBuf> = Vec::new();

    // 【Arc<Mutex<T>> パターン — スレッド間の共有可変データ】
    // Arc で複数のスレッド/クロージャが同じデータを共有し、
    // Mutex で排他的アクセス（同時に1つだけが書き込み可能）を保証します。
    // これは Go のチャネルや Java の synchronized に相当する同期プリミティブです。
    //
    // 【なぜ必要？】
    // filter_entry クロージャ内でマッチしたディレクトリを記録する必要がありますが、
    // クロージャは walker に move されるため、外部の Vec に直接書き込めません。
    // Arc<Mutex<Vec>> を使って、クロージャと外部コードの両方からアクセスします。
    let matched_dirs: Arc<std::sync::Mutex<Vec<PathBuf>>> =
        Arc::new(std::sync::Mutex::new(Vec::new()));

    let overrides_clone = Arc::clone(&overrides);
    let matched_dirs_clone = Arc::clone(&matched_dirs);
    let source_owned = source.to_path_buf();

    // 【WalkBuilder — ファイル走査の設定】
    // ビルダーパターンでオプションをチェーンし、最後に .build() で完成させます。
    // - hidden(false)    → 隠しファイルもスキップしない
    // - git_ignore(...)  → .gitignore のルールに従うかどうか
    // - overrides(...)   → .worktreelinks のパターンを設定
    let walker = WalkBuilder::new(source)
        .hidden(false)
        .ignore(!no_ignore)
        .git_ignore(!no_ignore)
        .git_global(!no_ignore)
        .git_exclude(!no_ignore)
        .overrides(walker_overrides)
        // 【move クロージャ】
        // `move` キーワードにより、クロージャが参照する変数の所有権を
        // クロージャに移動します。これにより、クロージャが関数のスコープを
        // 超えて生存しても安全になります。
        .filter_entry(move |entry| {
            // .git は常にスキップ
            if entry.file_name() == ".git" {
                return false;
            }

            let path = entry.path();

            // ルートディレクトリ自体は常に許可
            if path == source_owned {
                return true;
            }

            // 【.map().unwrap_or() チェーン — Option の安全な変換】
            // entry.file_type() は Option<FileType> を返します。
            // .map(|ft| ft.is_dir()) で Option<bool> に変換し、
            // .unwrap_or(false) で None の場合は false をデフォルト値とします。
            // これにより、ファイルタイプが不明な場合も安全に処理できます。
            let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);

            if is_dir {
                // 【Match::Whitelist — パターンにマッチした場合】
                // ignore クレートの Match enum は以下のバリアントを持ちます：
                //   - None       → どのパターンにもマッチしない
                //   - Whitelist  → 包含パターンにマッチ
                //   - Ignore     → 除外パターンにマッチ
                if let Match::Whitelist(_) = overrides_clone.matched(path, true) {
                    // マッチしたディレクトリを記録し、false を返して中に降りないようにする
                    // 【.lock().unwrap()】
                    // Mutex のロックを取得します。.unwrap() は他のスレッドがパニックして
                    // ロックが「毒された」場合にパニックしますが、ここでは安全です。
                    matched_dirs_clone.lock().unwrap().push(path.to_path_buf());
                    return false;
                }
            }

            true
        })
        .build();

    // walker はイテレータを実装しているので for ループで使えます
    for entry in walker {
        let entry = entry.with_context(|| "Error walking directory")?;
        let path = entry.path();

        // ソースルート自体はスキップ
        if path == source {
            continue;
        }

        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);

        // パターンにマッチしたファイルを収集
        if let Match::Whitelist(_) = overrides.matched(path, is_dir) {
            debug!("matched: {}", path.display());
            targets.push(path.to_path_buf());
        }
    }

    // filter_entry で刈り取られたディレクトリを追加
    let dirs = matched_dirs.lock().unwrap();
    for dir in dirs.iter() {
        debug!("matched dir: {}", dir.display());
        targets.push(dir.clone());
    }

    // 【sort() と dedup() — 決定的な出力の保証】
    // ファイルシステムの走査順序は OS やファイルシステムによって異なることがあります。
    // sort() でパスをアルファベット順にし、dedup() で重複を除去することで、
    // どの環境でも同じ出力を保証します。
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
        // パターンに一致しないファイルがスキップされることを確認
        let dir = tempdir("collect_skip");
        fs::write(dir.join(".env"), "SECRET=1").unwrap();
        fs::write(dir.join("main.rs"), "fn main(){}").unwrap();
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(dir.join("src/lib.rs"), "").unwrap();

        // ".env" パターンのみ → .env だけが収集される
        let targets = collect_targets(&dir, &[".env".into()], true).unwrap();
        // 【テストでのイテレータ活用】
        // strip_prefix で絶対パスを相対パスに変換し、比較しやすくしています。
        // `Vec<_>` の `_` は型推論に任せる書き方です（コンパイラが自動で推論）。
        let rel: Vec<_> = targets
            .iter()
            .map(|p| p.strip_prefix(&dir).unwrap())
            .collect();
        assert_eq!(rel, vec![Path::new(".env")]);
    }

    #[test]
    fn collect_targets_matches_directory_without_descending() {
        // ディレクトリがマッチした場合、中身には降りずディレクトリ自体だけが返る
        let dir = tempdir("collect_dir");
        let nm = dir.join("node_modules/pkg");
        fs::create_dir_all(&nm).unwrap();
        fs::write(nm.join("index.js"), "").unwrap();
        fs::write(dir.join("app.js"), "").unwrap();

        let targets = collect_targets(&dir, &["node_modules".into()], true).unwrap();
        let rel: Vec<_> = targets
            .iter()
            .map(|p| p.strip_prefix(&dir).unwrap())
            .collect();
        // ディレクトリ自体のみ含まれ、子要素（pkg/index.js）は含まれない
        assert_eq!(rel, vec![Path::new("node_modules")]);
    }

    #[test]
    fn collect_targets_negation_pattern() {
        // 【否定パターン（!）のテスト】
        // `!.env.production` は .env.production を除外するパターンです。
        // .gitignore と同じ構文で、先に包含パターンでマッチしたものから除外します。
        let dir = tempdir("collect_neg");
        fs::write(dir.join(".env"), "A=1").unwrap();
        fs::write(dir.join(".env.local"), "B=2").unwrap();
        fs::write(dir.join(".env.production"), "C=3").unwrap();

        let targets = collect_targets(
            &dir,
            // 【&[...] — 配列リテラルからスライスへの変換】
            // `.into()` で &str → String に変換しています。
            &[".env".into(), ".env.*".into(), "!.env.production".into()],
            true,
        )
        .unwrap();
        let rel: Vec<_> = targets
            .iter()
            .map(|p| p.strip_prefix(&dir).unwrap())
            .collect();
        // .env.production は否定パターンで除外されるため含まれない
        assert_eq!(rel, vec![Path::new(".env"), Path::new(".env.local")]);
    }

    #[test]
    fn collect_targets_respects_gitignore() {
        // 【.gitignore との連携テスト】
        // dist/ が .gitignore で無視されている場合、
        // override パターン **/*.js はファイルにマッチするが
        // ディレクトリ dist/ 自体にはマッチしないため、
        // .gitignore が適用されて walker がディレクトリごとスキップします。
        let dir = git_tempdir("collect_gitignore");
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(dir.join("src/app.js"), "").unwrap();
        fs::create_dir_all(dir.join("dist")).unwrap();
        fs::write(dir.join("dist/bundle.js"), "").unwrap();
        fs::write(dir.join(".gitignore"), "dist/\n").unwrap();

        let targets = collect_targets(&dir, &["**/*.js".into()], false).unwrap();
        let rel: Vec<_> = targets
            .iter()
            .map(|p| p.strip_prefix(&dir).unwrap())
            .collect();
        // dist/bundle.js は .gitignore により除外される
        assert_eq!(rel, vec![Path::new("src/app.js")]);
    }

    #[test]
    fn collect_targets_no_ignore_includes_all() {
        // no_ignore=true で .gitignore を完全に無効化した場合のテスト
        let dir = git_tempdir("collect_noignore");
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(dir.join("src/app.js"), "").unwrap();
        fs::create_dir_all(dir.join("dist")).unwrap();
        fs::write(dir.join("dist/bundle.js"), "").unwrap();
        fs::write(dir.join(".gitignore"), "dist/\n").unwrap();

        let targets = collect_targets(&dir, &["**/*.js".into()], true).unwrap();
        let rel: Vec<_> = targets
            .iter()
            .map(|p| p.strip_prefix(&dir).unwrap())
            .collect();
        // .gitignore に関係なく全てのマッチが含まれる
        assert_eq!(
            rel,
            vec![Path::new("dist/bundle.js"), Path::new("src/app.js")]
        );
    }

    #[test]
    fn collect_targets_worktreelinks_overrides_gitignore() {
        // 【Override の優先度テスト】
        // .env が .gitignore で無視されていても、
        // .worktreelinks の override パターンが優先されてリンク対象になります。
        // これが ignore クレートの Override 機能の核心的な動作です。
        let dir = git_tempdir("collect_override");
        fs::write(dir.join(".env"), "SECRET=1").unwrap();
        fs::write(dir.join(".gitignore"), ".env\n").unwrap();
        fs::write(dir.join("README.md"), "# Hello").unwrap();

        let targets = collect_targets(&dir, &[".env".into()], false).unwrap();
        let rel: Vec<_> = targets
            .iter()
            .map(|p| p.strip_prefix(&dir).unwrap())
            .collect();
        // .worktreelinks の override が .gitignore より優先されるため .env が含まれる
        assert_eq!(rel, vec![Path::new(".env")]);
    }

    /// テスト用の一時 git リポジトリを作成するヘルパー関数。
    /// ignore クレートが .gitignore を認識するために git init が必要です。
    fn git_tempdir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("worktree-link-test-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        // ignore クレートが .gitignore を認識するために git リポジトリを初期化
        let status = std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&dir)
            .status()
            .expect("git init に失敗");
        assert!(status.success(), "git init が終了コード {status} で失敗");
        // パスの比較で一致させるために正規化して返す
        fs::canonicalize(&dir).unwrap()
    }

    /// テスト用の一時ディレクトリを作成するヘルパー関数（git 初期化なし）。
    fn tempdir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("worktree-link-test-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        // パスの比較で一致させるために正規化して返す
        fs::canonicalize(&dir).unwrap()
    }
}
