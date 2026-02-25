// ============================================================================
// git.rs — git worktree のメインワークツリー自動検出
// ============================================================================
//
// ユーザーが --source を省略した場合、`git worktree list --porcelain` コマンドを
// 実行して、メインワークツリー（最初のエントリ）のパスを自動検出します。
//
// 【git worktree とは？】
// git worktree は1つのリポジトリから複数の作業ディレクトリを作る機能です。
// 例えば feature ブランチの作業中に別ブランチの修正が必要になった場合、
// stash せずに別のディレクトリで作業できます。
//
// 【--porcelain オプション】
// git の --porcelain はスクリプトで解析しやすい機械可読な出力形式です。
// 出力例：
//   worktree /home/user/repo
//   HEAD abc1234
//   branch refs/heads/main
//   （空行で区切り）
//   worktree /home/user/repo-feature
//   HEAD def5678
//   branch refs/heads/feature
//
// 【pub(crate) — クレート内限定公開】
// `pub` は完全公開、`pub(crate)` はクレート内でのみ公開されます。
// このモジュール外からでも同じクレート内なら呼べますが、
// 外部クレートからは呼べません。適切なカプセル化に役立ちます。
// ============================================================================

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// 指定ディレクトリから `git worktree list --porcelain` を実行し、
/// メインワークツリーのパスを検出します。
pub(crate) fn detect_main_worktree_in(dir: &Path) -> Result<PathBuf> {
    // 【std::process::Command — 外部コマンドの実行】
    // Command::new("git") で git プロセスを起動します。
    // .args() で引数を、.current_dir() で実行ディレクトリを指定します。
    // .output() はプロセスの終了を待ち、stdout/stderr/終了コードを返します。
    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(dir)
        .output()
        .context("Failed to run git. Use --source to specify the main worktree path.")?;

    if !output.status.success() {
        // 【String::from_utf8_lossy】
        // バイト列を UTF-8 文字列に変換します。不正なバイトは
        // Unicode の置換文字（�）に置き換えられるため、パニックしません。
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "Failed to detect main worktree: `git worktree list --porcelain` exited with {}.\nstderr:\n{}\nUse --source to specify the main worktree path.",
            output.status,
            stderr.trim_end(),
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_main_worktree(&stdout)
}

/// porcelain 出力を解析して最初の worktree パスを取得します。
/// 最初のエントリが常にメインワークツリーです。
fn parse_main_worktree(porcelain_output: &str) -> Result<PathBuf> {
    for line in porcelain_output.lines() {
        // 【if let — パターンマッチの簡略形】
        // `if let Some(x) = ...` は match 式の1パターンだけを扱う場合に便利です。
        // strip_prefix は「指定の接頭辞を取り除いた残り」を Some で返し、
        // 接頭辞がなければ None を返します。
        if let Some(path_str) = line.strip_prefix("worktree ") {
            let path = PathBuf::from(path_str);
            return fs::canonicalize(&path).with_context(|| {
                format!(
                    "Main worktree not found at: {path_str}. Use --source to specify it manually."
                )
            });
        }
    }

    bail!("Failed to detect main worktree from git output. Use --source to specify it manually.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn detect_main_worktree_returns_main_path() {
        let main_dir = git_tempdir("detect_main");

        // `git worktree add` を動かすために初期コミットを作成する
        fs::write(main_dir.join("README.md"), "# test").unwrap();
        let status = Command::new("git")
            .args(["add", "."])
            .current_dir(&main_dir)
            .status()
            .unwrap();
        assert!(status.success());
        let status = Command::new("git")
            .args(["commit", "-m", "init", "--quiet"])
            .current_dir(&main_dir)
            .status()
            .unwrap();
        assert!(status.success());

        // リンクされたワークツリーを追加する
        let wt_dir = std::env::temp_dir().join("worktree-link-test-detect_main_wt");
        let _ = fs::remove_dir_all(&wt_dir);
        let status = Command::new("git")
            .args(["worktree", "add", wt_dir.to_str().unwrap(), "-b", "test-wt"])
            .current_dir(&main_dir)
            .status()
            .unwrap();
        assert!(status.success());

        // リンクされたワークツリーから検出すると、メインワークツリーのパスが返るべき
        let detected = detect_main_worktree_in(&wt_dir).unwrap();
        assert_eq!(detected, main_dir);

        // テスト後のクリーンアップ
        // 【`let _ = ...` — 戻り値の明示的な破棄】
        // Rust では未使用の Result に対して警告が出ますが、
        // `let _ = ...` で意図的に無視していることを明示できます。
        // クリーンアップ処理では失敗しても問題ないため、エラーを無視しています。
        let _ = Command::new("git")
            .args(["worktree", "remove", "--force", wt_dir.to_str().unwrap()])
            .current_dir(&main_dir)
            .status();
        let _ = fs::remove_dir_all(&wt_dir);
    }

    #[test]
    fn detect_main_worktree_from_main_returns_self() {
        // メインワークツリー自身から検出した場合、自分自身のパスが返る
        let main_dir = git_tempdir("detect_self");

        let detected = detect_main_worktree_in(&main_dir).unwrap();
        assert_eq!(detected, main_dir);
    }

    #[test]
    fn detect_main_worktree_outside_git_repo_fails() {
        // git リポジトリ外で実行するとエラーになることを確認
        let dir = tempdir("detect_nogit");

        let result = detect_main_worktree_in(&dir);
        assert!(result.is_err());
        // 【.unwrap_err() — Err の中身を取り出す】
        // Result が Err であることを前提に中身を取り出します。
        // Ok だった場合はパニックします（テストでは意図的に使います）。
        let err_msg = result.unwrap_err().to_string();
        // エラーメッセージに --source が含まれていることを確認
        // （ユーザーに代替手段を案内しているか）
        assert!(
            err_msg.contains("--source"),
            "エラーメッセージに --source が含まれるべき、実際: {err_msg}"
        );
    }

    #[test]
    fn parse_main_worktree_extracts_first_entry() {
        let dir = git_tempdir("parse_first");
        // 【--allow-empty — 空のコミットを許可】
        // 通常 git はファイル変更なしのコミットを拒否しますが、
        // テストではこのフラグで空コミットを作成しています。
        let commit = Command::new("git")
            .args(["commit", "--allow-empty", "-m", "init"])
            .current_dir(&dir)
            .output()
            .unwrap();
        assert!(commit.status.success(), "git commit に失敗");

        // 生の porcelain 出力を取得してパース関数に渡す
        let output = Command::new("git")
            .args(["worktree", "list", "--porcelain"])
            .current_dir(&dir)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);

        // テスト対象の関数に委譲
        let parsed = parse_main_worktree(&stdout).unwrap();
        assert_eq!(parsed, dir);

        let _ = fs::remove_dir_all(&dir);
    }

    /// テスト用の一時 git リポジトリを作成するヘルパー関数。
    ///
    /// 【テストヘルパーのパターン】
    /// テストで繰り返し使うセットアップ処理は、ヘルパー関数にまとめます。
    /// `#[test]` 属性がない関数はテストケースとして実行されません。
    fn git_tempdir(name: &str) -> PathBuf {
        // 【format! マクロと文字列補間】
        // `format!("...{name}")` は変数を直接文字列に埋め込む構文です。
        // Rust 1.58 で安定化された「キャプチャ付きフォーマット」です。
        let dir = std::env::temp_dir().join(format!("worktree-link-test-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let status = Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&dir)
            .status()
            .expect("git init に失敗");
        assert!(status.success(), "git init が終了コード {status} で失敗");
        // コミット用にユーザー設定を行う（テスト環境にはグローバル設定がないため）
        let status = Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(&dir)
            .status()
            .unwrap();
        assert!(status.success());
        let status = Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(&dir)
            .status()
            .unwrap();
        assert!(status.success());
        // 正規化したパスを返す（パスの比較で一致させるため）
        // 【fs::canonicalize の重要性】
        // macOS では /tmp が /private/tmp のシンボリックリンクなので、
        // canonicalize しないと assert_eq! でパスが一致しません。
        fs::canonicalize(&dir).unwrap()
    }

    /// テスト用の一時ディレクトリを作成するヘルパー関数（git 初期化なし）。
    fn tempdir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("worktree-link-test-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::canonicalize(&dir).unwrap()
    }
}
