// ============================================================================
// linker.rs — シンボリックリンクの作成・削除ロジック
// ============================================================================
//
// このモジュールがツールの中核で、実際にファイルシステムを操作します。
// 主な機能：
//   - create_link()     — ソースからターゲットへのシンボリックリンクを作成
//   - unlink_targets()  — ターゲット内のシンボリックリンクを削除
//   - walk_symlinks()   — ディレクトリを再帰的に走査してシンボリックリンクを発見
//
// 安全性への配慮として、以下のガードが実装されています：
//   - 親ディレクトリがシンボリックリンクの場合はスキップ（データ破損防止）
//   - dry_run モードでは実際の変更を行わない
//   - .git ディレクトリは常にスキップ
// ============================================================================

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

// 【enum（列挙型）— Rust の最も強力な型の一つ】
// Rust の enum は他言語の enum とは異なり、各バリアントがデータを持てます。
// これを「代数的データ型」と呼びます。
//
// LinkAction は「リンク操作の結果」を表し、3つの可能な状態を持ちます：
//   - Created    → 新規作成された（ソースとターゲットのパスを持つ）
//   - Skipped    → スキップされた（ターゲットと理由を持つ）
//   - Overwritten→ 上書きされた（ソースとターゲットのパスを持つ）
//
// 【構造体バリアント vs タプルバリアント】
// `Created { source, target }` は名前付きフィールド（構造体バリアント）、
// `Removed(PathBuf)` は位置指定フィールド（タプルバリアント）です。
// フィールドが1つだけの場合はタプル、複数の場合は構造体が読みやすいです。

/// リンク作成操作の結果を表す列挙型。
#[derive(Debug, PartialEq)]
pub enum LinkAction {
    Created { source: PathBuf, target: PathBuf },
    Skipped { target: PathBuf, reason: String },
    Overwritten { source: PathBuf, target: PathBuf },
}

/// リンク削除操作の結果を表す列挙型。
#[derive(Debug, PartialEq)]
pub enum UnlinkAction {
    Removed(PathBuf),
    Skipped { target: PathBuf, reason: String },
}

// 【Display トレイトの実装】
// Display トレイトを実装すると、`println!("{}", action)` や `format!("{action}")`
// で人間が読みやすい形式に変換されます。
// Debug（{:?}）がデバッグ用途なのに対し、Display（{}）はユーザー向けの出力です。
//
// colored クレートの .green().bold() などで、ターミナル出力に色を付けています。
impl std::fmt::Display for LinkAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinkAction::Created { source, target } => {
                write!(
                    f,
                    "{} {} -> {}",
                    "LINK".green().bold(),
                    target.display(),
                    source.display()
                )
            }
            LinkAction::Skipped { target, reason } => {
                write!(
                    f,
                    "{} {} ({})",
                    "SKIP".yellow().bold(),
                    target.display(),
                    reason
                )
            }
            LinkAction::Overwritten { source, target } => {
                write!(
                    f,
                    "{} {} -> {}",
                    "OVERWRITE".magenta().bold(),
                    target.display(),
                    source.display()
                )
            }
        }
    }
}

impl std::fmt::Display for UnlinkAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnlinkAction::Removed(path) => {
                write!(f, "{} {}", "UNLINK".red().bold(), path.display())
            }
            UnlinkAction::Skipped { target, reason } => {
                write!(
                    f,
                    "{} {} ({})",
                    "SKIP".yellow().bold(),
                    target.display(),
                    reason
                )
            }
        }
    }
}

/// ソースパスからターゲットパスへのシンボリックリンクを作成します。
///
/// 【設計上の重要な判断】
/// source_path に対して fs::canonicalize() を呼ばないのは意図的です。
/// ソースツリー内にあるシンボリックリンクをそのまま保持することで、
/// link → unlink の往復操作が一貫した結果になります。
///
/// 【anyhow::ensure! マクロ】
/// 条件が false の場合にエラーを返すマクロです。
/// assert! に似ていますが、パニックではなく Result のエラーを返す点が異なります。
pub fn create_link(
    source_path: &Path,
    target_path: &Path,
    force: bool,
    dry_run: bool,
) -> Result<LinkAction> {
    anyhow::ensure!(
        source_path.is_absolute(),
        "source_path must be absolute: {}",
        source_path.display()
    );

    debug!(
        "create_link: {} -> {}",
        target_path.display(),
        source_path.display()
    );

    // ターゲットが既に存在する場合の処理
    // 【.is_symlink() を別途チェックする理由】
    // .exists() は壊れたシンボリックリンク（リンク先がない）の場合に false を返します。
    // .is_symlink() はリンク自体の存在をチェックするので、壊れたリンクも検出できます。
    if target_path.exists() || target_path.is_symlink() {
        if !force {
            return Ok(LinkAction::Skipped {
                target: target_path.to_path_buf(),
                reason: "already exists (use --force to overwrite)".into(),
            });
        }

        // 【安全ガード：親ディレクトリがシンボリックリンクの場合】
        // 親がシンボリックリンクだと、remove 操作がリンク先の実データを
        // 削除してしまう危険があるため、スキップします。
        if has_symlink_parent(target_path) {
            return Ok(LinkAction::Skipped {
                target: target_path.to_path_buf(),
                reason: "parent directory is a symlink (remove it first)".into(),
            });
        }

        if !dry_run {
            remove_entry(target_path)
                .with_context(|| format!("Failed to remove: {}", target_path.display()))?;
            create_parent_dirs(target_path)?;
            symlink(source_path, target_path)?;
        }

        if dry_run {
            info!("[dry-run] would overwrite: {}", target_path.display());
        } else {
            info!("overwritten: {}", target_path.display());
        }
        return Ok(LinkAction::Overwritten {
            source: source_path.to_path_buf(),
            target: target_path.to_path_buf(),
        });
    }

    // ターゲットが存在しない場合でも、親ディレクトリのシンボリックリンクチェックは必要
    // （リンク先のディレクトリに誤ってファイルを作成してしまうのを防ぐ）
    if has_symlink_parent(target_path) {
        return Ok(LinkAction::Skipped {
            target: target_path.to_path_buf(),
            reason: "parent directory is a symlink (remove it first)".into(),
        });
    }

    if !dry_run {
        create_parent_dirs(target_path)?;
        symlink(source_path, target_path)?;
    }

    if dry_run {
        info!("[dry-run] would link: {}", target_path.display());
    } else {
        info!("linked: {}", target_path.display());
    }
    Ok(LinkAction::Created {
        source: source_path.to_path_buf(),
        target: target_path.to_path_buf(),
    })
}

/// ターゲットディレクトリ内を走査し、ソースディレクトリを指すシンボリックリンクを削除します。
///
/// 【設計のポイント】
/// ソース側ではなくターゲット側を走査するのは、ソース側で元ファイルが
/// 削除・リネームされた「壊れたリンク」も検出できるようにするためです。
///
/// 【クロージャ（|entry_path| { ... }）】
/// クロージャは「その場で定義する無名関数」です。
/// `|引数| { 本体 }` という構文で書きます。
/// ここでは walk_symlinks に「各エントリに対して何をするか」を渡しています。
/// クロージャは外側のスコープの変数（actions など）をキャプチャ（借用）できます。
pub fn unlink_targets(
    source_dir: &Path,
    target_dir: &Path,
    dry_run: bool,
) -> Result<Vec<UnlinkAction>> {
    // ソースディレクトリを正規化して、starts_with による比較を正確にする
    let canonical_source = fs::canonicalize(source_dir).with_context(|| {
        format!(
            "Failed to canonicalize source dir: {}",
            source_dir.display()
        )
    })?;

    let mut actions = Vec::new();

    // 【&mut によるクロージャの渡し方】
    // `&mut dyn FnMut(...)` は「可変参照のトレイトオブジェクト」です。
    // クロージャが外部変数（actions）を変更するため FnMut が必要です。
    walk_symlinks(target_dir, &mut |entry_path| {
        if !entry_path.is_symlink() {
            return Ok(());
        }

        // 【fs::read_link — シンボリックリンクの宛先を取得】
        // リンクが指しているパスを読み取ります。
        let link_dest = match fs::read_link(&entry_path) {
            Ok(dest) => dest,
            Err(e) => {
                warn!("Skipping {}: {e}", entry_path.display());
                actions.push(UnlinkAction::Skipped {
                    target: entry_path,
                    reason: format!("cannot read symlink: {e}"),
                });
                return Ok(());
            }
        };

        // 相対パスのシンボリックリンクを絶対パスに変換して比較可能にする。
        // fs::read_link は相対パスを返すことがあるが、source_dir は正規化済みのため。
        let resolved = if link_dest.is_absolute() {
            link_dest
        } else {
            match entry_path.parent() {
                Some(parent) => parent.join(&link_dest),
                None => link_dest,
            }
        };

        // 【canonicalize の落とし穴と対策】
        // macOS では /var が実際には /private/var のエイリアスだったりします。
        // 壊れたシンボリックリンク（リンク先が存在しない）は canonicalize が失敗するため、
        // 存在する最も深い祖先まで canonicalize して残りを結合する fallback を使います。
        let resolved = canonicalize_with_ancestor_fallback(&resolved);

        // ソースディレクトリを指すリンクだけを削除対象とする
        if !resolved.starts_with(&canonical_source) {
            return Ok(());
        }

        if !dry_run {
            if let Err(e) = remove_entry(&entry_path) {
                warn!("Failed to remove {}: {e}", entry_path.display());
                actions.push(UnlinkAction::Skipped {
                    target: entry_path,
                    reason: format!("removal failed: {e}"),
                });
                return Ok(());
            }
        }

        if dry_run {
            info!("[dry-run] would unlink: {}", entry_path.display());
        } else {
            info!("unlinked: {}", entry_path.display());
        }
        actions.push(UnlinkAction::Removed(entry_path));
        Ok(())
    })?;

    // 【sort_by — カスタムソート】
    // 出力の順序を決定的にするため、パスでソートします。
    // `|a, b| { ... }` はクロージャで、2つの要素を受け取って順序を返します。
    actions.sort_by(|a, b| {
        let path_a = match a {
            UnlinkAction::Removed(p) | UnlinkAction::Skipped { target: p, .. } => p,
        };
        let path_b = match b {
            UnlinkAction::Removed(p) | UnlinkAction::Skipped { target: p, .. } => p,
        };
        path_a.cmp(path_b)
    });

    Ok(actions)
}

/// ディレクトリを再帰的に走査し、見つけたシンボリックリンクに対して visitor を呼び出します。
/// シンボリックリンク先には追従しません（リンクされたディレクトリの中には入らない）。
///
/// 【dyn FnMut — トレイトオブジェクト】
/// `&mut dyn FnMut(PathBuf) -> Result<()>` は動的ディスパッチのトレイトオブジェクトです。
/// - `dyn` = コンパイル時ではなく実行時に具体的な型が決まる（vtable 経由で呼び出し）
/// - `FnMut` = 環境を可変にキャプチャできるクロージャのトレイト
///   - `Fn`    = 不変にキャプチャ（読み取りのみ）
///   - `FnMut` = 可変にキャプチャ（変更可能）
///   - `FnOnce`= 所有権を奪ってキャプチャ（一度だけ呼べる）
///
/// 【ベストエフォート戦略】
/// 個々のエントリのエラーは warn ログを出してスキップし、走査を続行します。
/// これにより、一部のファイルに権限がなくてもツール全体が中断しません。
fn walk_symlinks(dir: &Path, visitor: &mut dyn FnMut(PathBuf) -> Result<()>) -> Result<()> {
    // 【match ガード（if e.kind() == ...）】
    // match のアームに条件を追加できます。
    // ここでは NotFound と PermissionDenied を特別扱いし、
    // それ以外のエラーだけを上位に伝播させています。
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            warn!("Skipping directory {}: {e}", dir.display());
            return Ok(());
        }
        Err(e) => return Err(e).with_context(|| format!("Failed to read dir: {}", dir.display())),
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                warn!("Skipping entry: {e}");
                continue;
            }
        };
        let path = entry.path();
        // 【symlink_metadata vs metadata】
        // fs::metadata() はシンボリックリンクを辿ってリンク先の情報を返します。
        // fs::symlink_metadata() はリンク自体の情報を返します。
        // ここではリンクかどうかを判定するため、symlink_metadata を使います。
        let meta = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) => {
                warn!("Skipping {}: {e}", path.display());
                continue;
            }
        };

        if meta.file_type().is_symlink() {
            visitor(path)?;
        } else if meta.is_dir() {
            // .git ディレクトリはスキップ（リポジトリの内部構造を壊さないため）
            if path.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            // 【再帰呼び出し】
            // ディレクトリの場合は自分自身を再帰的に呼び出して子ディレクトリも走査します。
            walk_symlinks(&path, visitor)?;
        }
    }

    Ok(())
}

/// パスを字句的に正規化します（ファイルシステムにはアクセスしない）。
/// `.` と `..` を解決しますが、シンボリックリンクは解決しません。
///
/// 【なぜ必要？】
/// fs::canonicalize() はパスが存在しないとエラーになります。
/// 壊れたシンボリックリンクのターゲットなど、存在しないパスを
/// 正規化したい場合のフォールバックとして使います。
fn normalize_lexically(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                // ルートやプレフィックスより上には遡らない
                // 【ネストした match とパターンの or（|）】
                // `Some(Component::RootDir | Component::Prefix(_))` は
                // 「RootDir または Prefix のどちらか」にマッチします。
                match parts.last() {
                    Some(Component::RootDir | Component::Prefix(_)) | None => {}
                    _ => {
                        parts.pop();
                    }
                }
            }
            Component::CurDir => {} // "." は無視
            other => parts.push(other),
        }
    }
    // 【.iter().collect() — イテレータから PathBuf を構築】
    // Component のイテレータから PathBuf を collect で組み立てます。
    parts.iter().collect()
}

/// プレフィックス比較用にパスを正規化します。
///
/// 完全な canonicalize が失敗する場合（壊れたシンボリックリンクなど）、
/// 存在する最も深い祖先まで canonicalize し、残りのパスを結合します。
/// macOS での `/var` vs `/private/var` のようなエイリアス問題を防ぎます。
fn canonicalize_with_ancestor_fallback(path: &Path) -> PathBuf {
    let normalized = normalize_lexically(path);
    // まず完全な canonicalize を試みる
    if let Ok(canonical) = fs::canonicalize(&normalized) {
        return canonical;
    }

    // 失敗した場合、祖先を辿って存在する最も深いディレクトリを見つける
    // 【.ancestors() イテレータ】
    // パスの祖先を順に返します。
    // 例: "/a/b/c/d" → "/a/b/c/d", "/a/b/c", "/a/b", "/a", "/"
    for ancestor in normalized.ancestors() {
        if ancestor.as_os_str().is_empty() {
            continue;
        }

        // 【let-else 構文】
        // `let Ok(x) = expr else { return/continue/break };` は
        // パターンにマッチしなかった場合の早期脱出を簡潔に書けます。
        // Rust 1.65 で安定化された比較的新しい構文です。
        let Ok(canonical_ancestor) = fs::canonicalize(ancestor) else {
            continue;
        };
        let Ok(remainder) = normalized.strip_prefix(ancestor) else {
            continue;
        };

        return if remainder.as_os_str().is_empty() {
            canonical_ancestor
        } else {
            canonical_ancestor.join(remainder)
        };
    }

    normalized
}

/// パスの親ディレクトリのいずれかがシンボリックリンクかどうかを判定します。
///
/// 【while let — ループ内のパターンマッチ】
/// `while let Some(parent) = current.parent()` は、
/// parent() が Some を返す間ループを続けます。
/// ルートディレクトリに到達すると parent() は空文字列を返すため、
/// そこで break します。
fn has_symlink_parent(path: &Path) -> bool {
    let mut current = path.to_path_buf();
    while let Some(parent) = current.parent() {
        if parent.as_os_str().is_empty() {
            break;
        }
        if parent.is_symlink() {
            return true;
        }
        current = parent.to_path_buf();
    }
    false
}

/// 指定パスの親ディレクトリを再帰的に作成します。
/// mkdir -p に相当する操作です。
fn create_parent_dirs(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create parent directory: {}", parent.display()))?;
    }
    Ok(())
}

/// ファイルまたはディレクトリを削除します。
/// ディレクトリの場合は中身ごと再帰的に削除します（rm -rf 相当）。
fn remove_entry(path: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if meta.is_dir() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }
    Ok(())
}

// 【条件付きコンパイル（#[cfg(...)]）】
// `#[cfg(unix)]` はUnix系OSでのみコンパイルされるコードを示します。
// `#[cfg(not(unix))]` はそれ以外のOS用です。
// これにより、プラットフォーム固有のAPIを安全に使い分けられます。
// Rust ではこのようなクロスプラットフォーム対応をコンパイル時に行います。

#[cfg(unix)]
fn symlink(source: &Path, target: &Path) -> Result<()> {
    std::os::unix::fs::symlink(source, target).with_context(|| {
        format!(
            "Failed to create symlink: {} -> {}",
            target.display(),
            source.display()
        )
    })
}

#[cfg(not(unix))]
fn symlink(source: &Path, target: &Path) -> Result<()> {
    anyhow::bail!("Symlink creation is only supported on Unix systems")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir() -> PathBuf {
        let mut dir = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before UNIX_EPOCH")
            .as_nanos();
        dir.push(format!("worktree-link-test-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&dir).expect("failed to create test temp dir");
        dir
    }

    #[cfg(unix)]
    #[test]
    fn canonicalize_with_ancestor_fallback_resolves_alias_for_dangling_path() {
        let root = unique_temp_dir();
        let real = root.join("real");
        fs::create_dir_all(&real).expect("failed to create real dir");

        let alias = root.join("alias");
        std::os::unix::fs::symlink(&real, &alias).expect("failed to create alias symlink");

        let dangling = alias.join("missing").join("child");
        let resolved = canonicalize_with_ancestor_fallback(&dangling);
        let canonical_real = fs::canonicalize(&real).expect("failed to canonicalize real dir");
        assert_eq!(resolved, canonical_real.join("missing").join("child"));

        fs::remove_dir_all(&root).expect("failed to cleanup temp dir");
    }
}
