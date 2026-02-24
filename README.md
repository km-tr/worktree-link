# worktree-link

`git worktree` 作成時に、元のディレクトリから指定ファイル/ディレクトリのシンボリックリンクを自動作成する CLI ツール。

## ユースケース

- `node_modules` の共有（巨大な依存を worktree ごとにインストールしない）
- `.env` / `.env.local` などの環境変数ファイル
- `.next/` / `tmp/` / `dist/` などのキャッシュ・ビルド成果物
- IDE 設定ファイル（`.idea/`, `.vscode/`）

## インストール

```bash
cargo install --path .
```

## 使い方

```
worktree-link [OPTIONS] <SOURCE> [TARGET]
```

### 引数

| 引数 | 説明 | デフォルト |
|------|------|-----------|
| `SOURCE` | リンク元ディレクトリ（メインの worktree） | 必須 |
| `TARGET` | リンク先ディレクトリ（新しい worktree） | `.`（カレントディレクトリ） |

### オプション

| オプション | 説明 | デフォルト |
|-----------|------|-----------|
| `-c, --config <FILE>` | 設定ファイルのパス | `<SOURCE>/.worktreelinks` |
| `-n, --dry-run` | 実行せずにリンク作成予定を表示 | `false` |
| `-f, --force` | 既存のファイル/リンクを上書き | `false` |
| `-v, --verbose` | 詳細ログ出力 | `false` |
| `--unlink` | 作成済みシンボリックリンクを解除 | `false` |

### 使用例

```bash
# メイン worktree から現在のディレクトリにリンク作成
worktree-link /path/to/main

# 対象ディレクトリを指定
worktree-link /path/to/main ./feature-branch

# dry-run で確認してからリンク作成
worktree-link --dry-run /path/to/main
worktree-link /path/to/main

# 既存ファイルを上書きしてリンク作成
worktree-link --force /path/to/main

# リンク解除
worktree-link --unlink /path/to/main
```

## 設定ファイル (`.worktreelinks`)

プロジェクトルートに `.worktreelinks` ファイルを作成し、リンクしたいファイル/ディレクトリを gitignore 互換の glob パターンで記述します。

```gitignore
# 依存・パッケージ
node_modules

# 環境変数
.env
.env.*

# ビルド成果物・キャッシュ
.next/
tmp/
dist/

# IDE
.idea/
.vscode/settings.json

# 特定のパターン
packages/*/node_modules
```

### パターンルール

- `#` で始まる行はコメント
- 空行は無視
- `/` で終わるパターンはディレクトリのみマッチ
- `*` は `/` を除く任意の文字にマッチ
- `**` はディレクトリを跨いでマッチ
- `!` で始まるパターンは除外（否定パターン）

## 動作の詳細

### ディレクトリリンク

`node_modules` のようなディレクトリにマッチした場合、ディレクトリ自体をシンボリックリンクとして作成します（中のファイルを個別にリンクしません）。

### 絶対パス

シンボリックリンクは絶対パスで作成されます。worktree の場所が移動しても壊れにくくなっています。

### 安全性

- `.git/` ディレクトリは常に除外されます
- `--force` を指定しない限り、既存のファイル/リンクは上書きしません
- `--unlink` はリンク先が SOURCE 配下を指すシンボリックリンクのみ解除します

## 開発

```bash
# ビルド
cargo build

# テスト
cargo test

# デバッグ実行
cargo run -- --dry-run /path/to/source /path/to/target
```

## ライセンス

MIT
