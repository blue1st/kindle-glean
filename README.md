# Kindle Glean

Kindle端末（Paperwhite、Oasis、Kindle Scribeなど）がPCにUSB接続されたことをトリガーに、読書ハイライト・手書きノート・語彙ログを自動抽出・収集（Glean）し、Obsidian Vault連携を想定したローカルディレクトリへMarkdownおよび画像/SVGアセットとして出力・整理するデスクトップアプリケーションです。

---

## 🌟 主な機能

1. **USB接続の自動検知 & バックグラウンド常駐**
   - macOS (`/Volumes/Kindle` / ボリューム監視) および Windows ドライブレターをバックグラウンドスレッドで自動監視。
   - 接続時にシステム通知を発行し、自動で取り込み（同期）を実行（設定でON/OFF可能）。
   - タスクトレイ（メニューバー）常駐型で、いつでも「今すぐ同期」やメイン画面の呼び出しが可能。
   - 取り込み完了後の自動安全アンマウント（イジェクト）機能。

2. **ハイライト & メモ (`My Clippings.txt`)**
   - 日本語・英語の両フォーマットに完全対応した正規表現パーサー。
   - タイトル・ロケーション・内容に基づく SHA-256 ハッシュによる厳格な重複排除。
   - Obsidian フレンドリーなフロントマター（YAML）と引用ブロック（`> 本文`）の自動生成。
   - 既存Markdownファイルへの安全な差分追記（ユーザー自身の手動編集箇所を破壊しません）。

3. **語彙ログ & 単語帳 (`vocab.db`)**
   - Kindle内蔵辞書で調べた単語帳 SQLite データベースを直読み解析。
   - 単語、見出し語（stem）、調べた文脈（例文）、該当書籍名、検索日時を抽出して `Vocabulary/Kindle_Vocabulary.md` へ自動集約。

4. **Kindle Scribe 手書きノート (`.nbk`)**
   - Scribe のノートアーカイブ（`.nbk` / ZIP形式）からページごとのベクター/ラスターデータを展開。
   - `_assets/notebooks/{ノート名}/` 配下にページ画像/SVGを格納。
   - `Notebooks/{ノート名}.md` から `![[page_1.svg]]` の Obsidian 記法でリンクを自動埋め込み。

5. **デスクトップ UI & プレビュー**
   - Tailwind CSS によるダークモード対応のモダンUI。
   - リアルタイムな接続ステータス、過去の同期履歴タイムライン。
   - `My Clippings.txt` を開いて書籍や種別（ハイライト/メモ/しおり）で全文検索できるプレビュービューア。
   - 実機がなくてもテストできる「フォルダ指定同期」機能。

---

## 📂 出力ディレクトリ構造（Obsidianフレンドリー設計）

設定で指定した Vault フォルダ（デフォルト: `{Vault Root}/Kindle/`）配下に自動展開されます。

```text
{Vault Root}/Kindle/
├── Books/
│   ├── クリーンアーキテクチャ.md
│   ├── プロフェッショナル原論.md
│   └── The Pragmatic Programmer.md
├── Notebooks/
│   └── 要件定義ラフ.md
├── Vocabulary/
│   └── Kindle_Vocabulary.md
└── _assets/
    └── notebooks/
        └── 要件定義ラフ/
            └── page_1.svg
```

### 書籍Markdownの出力例

```markdown
---
type: book-highlight
title: "クリーンアーキテクチャ"
author: "Robert C. Martin"
last_sync: "2026-09-22 03:40:00"
tags:
  - kindle/highlights
---

# クリーンアーキテクチャ

## メタデータ
- 著者: Robert C. Martin
- 最終同期: 2026-09-22 03:40:00

## ハイライト・メモ一覧

> アーキテクチャの目的は、求められるシステムを構築・保守するために必要な人材を最小限に抑えることである。
— 位置: 245-247 | 追加日: 2026-09-18 21:30

> 💡 **メモ:** 優れたアーキテクトは、方針を詳細から切り離し、決定をできるだけ遅らせる。
— 位置: 250 | 追加日: 2026-09-18 21:32
```

---

## 📦 インストール（一般利用者向け）

### macOS (Homebrew Cask) 推奨
Homebrew 経由で簡単にインストールできます。未署名アプリに対する Gatekeeper 隔離属性の解除（`xattr -cr`）も自動で行われるため、警告ダイアログに煩わされずスムーズに起動できます。

```bash
brew tap blue1st/taps
brew install --cask kindle-glean
```

### GitHub Releases から直接ダウンロード
GitHub の [Releases](https://github.com/blue1st/kindle-glean/releases) ページから、お使いの OS に合わせたインストーラ（macOS 用 `.dmg`、Windows 用 `.exe` / `.msi`）をダウンロードして実行することも可能です。

> [!NOTE]
> **Python などの追加インストールは一切不要です。**
> Kindle Scribe の手書きノート（`.nbk`）をベクター SVG に変換する内部コンバータはスタンドアロンバイナリとしてアプリ本体に同梱されているため、ダブルクリックですぐにお使いいただけます。

---

## 🚀 開発 & 起動方法

### 前提環境
- Node.js (v18+)
- Rust (1.78+)
- Cargo & Tauri CLI v2
- （開発・テスト時）Python 3.8+

### 1. 依存関係のインストール
```bash
npm install
```

### 2. デスクトップアプリの起動（開発モード）
```bash
npm run tauri dev
```
または Web UI のみ起動してブラウザ確認:
```bash
npm run dev
```

### 3. テストの実行
パーサー、差分ロジック、Markdown生成、エンドツーエンド統合テストを実行します:
```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

### 4. ビルド（配布用バイナリ生成）

Python 非依存の完全スタンドアロンバイナリを含めた配布パッケージを生成する場合:
```bash
# 1. Python コンバータを単一実行バイナリにパッケージング (PyInstaller)
npm run build:sidecar

# 2. Tauri デスクトップアプリのバンドル生成
npm run tauri build
```

### 5. リリースの実行 (release-it)
自動テスト・型チェック・バージョン同期・GitHub Releases & Homebrew Tap への Cask 反映を一括実行します:
```bash
npm run release
```
> [!TIP]
> `release-it` の実行により、以下のフローが自動的に行われます:
> 1. **事前検証**: TypeScript の型チェック (`npm run check`) および Rust のテスト (`cargo test`) を実行。
> 2. **バージョン同期**: `package.json`, `tauri.conf.json`, `Cargo.toml`, `Cargo.lock` のバージョンを一括更新。
> 3. **タグ生成 & Push**: GitHub へリリース用のタグ（`v*`）をプッシュ。
> 4. **自動ビルド & Homebrew 反映**: GitHub Actions が起動し、各 OS 向けインストーラを作成後、[`blue1st/homebrew-taps`](https://github.com/blue1st/homebrew-taps) の Cask 定義（`xattr -cr` 認証スルー処理含む）を自動更新します。


---

## 🧪 サンプルデータによる動作確認

プロジェクト内に動作確認用サンプルデータ `sample_kindle_data/` が用意されています。
1. アプリを起動 (`npm run tauri dev`)
2. ダッシュボードの「フォルダ指定同期」をクリック
3. `sample_kindle_data/` フォルダを選択
4. 指定した Vault フォルダ内に即座に `Books/`, `Vocabulary/`, `Notebooks/` が同期出力されます。
