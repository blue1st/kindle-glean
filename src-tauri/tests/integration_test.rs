use std::fs;
use std::sync::Arc;
use tauri_app_lib::db::Database;
use tauri_app_lib::models::SyncConfig;
use tauri_app_lib::sync::SyncOrchestrator;

#[test]
fn test_end_to_end_kindle_sync() {
    let temp_root = std::env::temp_dir().join(format!("e2e_kindle_{}", chrono::Utc::now().timestamp_millis()));
    let kindle_dir = temp_root.join("KindleDevice");
    let vault_dir = temp_root.join("ObsidianVault");

    // 1. Prepare Kindle mock structure
    let docs_dir = kindle_dir.join("documents");
    let vocab_dir = kindle_dir.join("system").join("vocabulary");
    let notebooks_dir = kindle_dir.join(".notebooks");
    fs::create_dir_all(&docs_dir).unwrap();
    fs::create_dir_all(&vocab_dir).unwrap();
    fs::create_dir_all(&notebooks_dir).unwrap();

    // 1.1 My Clippings.txt with both Japanese and English highlights
    let clippings_content = r#"クリーンアーキテクチャ (Robert C. Martin)
- あなたのハイライト (位置No. 245-247) | 作成日時: 2026年9月18日金曜日 21:30:15

アーキテクチャの目的は、求められるシステムを構築・保守するために必要な人材を最小限に抑えることである。
==========
クリーンアーキテクチャ (Robert C. Martin)
- あなたのメモ (位置No. 250) | 作成日時: 2026年9月18日金曜日 21:32:00

優れたアーキテクトは、方針を詳細から切り離し、決定をできるだけ遅らせる。
==========
プロフェッショナル原論 (波頭 亮)
- あなたのハイライト (位置No. 120-122) | 作成日時: 2026年9月19日土曜日 10:15:00

プロフェッショナルの条件とは、高い専門性と高い倫理観の双方を兼ね備えていることである。
==========
"#;
    fs::write(docs_dir.join("My Clippings.txt"), clippings_content).unwrap();

    // 1.2 Mock vocab.db (SQLite)
    let vocab_db_path = vocab_dir.join("vocab.db");
    let conn = rusqlite::Connection::open(&vocab_db_path).unwrap();
    conn.execute_batch(
        r#"
        CREATE TABLE WORDS (id TEXT PRIMARY KEY, word TEXT, stem TEXT, lang TEXT, category INTEGER, timestamp INTEGER);
        CREATE TABLE BOOK_INFO (id TEXT PRIMARY KEY, asin TEXT, title TEXT, authors TEXT);
        CREATE TABLE LOOKUPS (id TEXT PRIMARY KEY, word_key TEXT, book_key TEXT, dict_key TEXT, usage TEXT, timestamp INTEGER);

        INSERT INTO WORDS VALUES ('en:ubiquitous', 'ubiquitous', 'ubiquitous', 'en', 0, 1726700000000);
        INSERT INTO BOOK_INFO VALUES ('B079Y79K4Y', 'B079Y79K4Y', 'クリーンアーキテクチャ', 'Robert C. Martin');
        INSERT INTO LOOKUPS VALUES ('lk_1', 'en:ubiquitous', 'B079Y79K4Y', 'dict_en', 'Ubiquitous language is a core pillar of domain driven design.', 1726700000000);
    "#
    ).unwrap();
    drop(conn);

    // 1.3 Mock Kindle Scribe .nbk file
    let sample_nbk_path = notebooks_dir.join("アイデアメモ_2026-09.nbk");
    let file = fs::File::create(&sample_nbk_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);
    zip.start_file("page_1.svg", options).unwrap();
    std::io::Write::write_all(
        &mut zip,
        b"<svg xmlns=\"http://www.w3.org/2000/svg\"><text y=\"50\">Handwritten Idea 1</text></svg>",
    ).unwrap();
    zip.finish().unwrap();

    // 2. Run Sync
    let db = Arc::new(Database::open_in_memory().unwrap());
    let orchestrator = SyncOrchestrator::new(db.clone());

    let config = SyncConfig {
        vault_path: vault_dir.to_string_lossy().to_string(),
        sync_clippings: true,
        sync_vocab: true,
        sync_notebooks: true,
        auto_sync: true,
        auto_eject: false,
        subfolder: "Kindle".to_string(),
    };

    let stats = orchestrator.sync_from_path(&kindle_dir, &config, "Kindle Scribe (Test)", None).unwrap();
    assert_eq!(stats.highlights_added, 2);
    assert_eq!(stats.notes_added, 1);
    assert_eq!(stats.vocab_added, 1);
    assert_eq!(stats.notebooks_added, 1);
    assert_eq!(stats.total_books_updated, 2);

    // 3. Verify Obsidian Directory Structure & Markdown
    let base_out = vault_dir.join("Kindle");

    // Check Books
    let clean_arch_md = base_out.join("Books").join("クリーンアーキテクチャ.md");
    assert!(clean_arch_md.exists(), "クリーンアーキテクチャ.md must exist");
    let content = fs::read_to_string(&clean_arch_md).unwrap();
    assert!(content.contains("type: book-highlight"));
    assert!(content.contains("Robert C. Martin"));
    assert!(content.contains("アーキテクチャの目的は"));
    assert!(content.contains("優れたアーキテクトは"));

    let pro_md = base_out.join("Books").join("プロフェッショナル原論.md");
    assert!(pro_md.exists());
    let pro_content = fs::read_to_string(&pro_md).unwrap();
    assert!(pro_content.contains("波頭 亮"));
    assert!(pro_content.contains("プロフェッショナルの条件とは"));

    // Check Vocabulary
    let vocab_md = base_out.join("Vocabulary").join("Kindle_Vocabulary.md");
    assert!(vocab_md.exists(), "Kindle_Vocabulary.md must exist");
    let vocab_content = fs::read_to_string(&vocab_md).unwrap();
    assert!(vocab_content.contains("### ubiquitous"));
    assert!(vocab_content.contains("Ubiquitous language is a core pillar"));

    // Check Notebooks & Assets (Self-contained in Notebooks/アイデアメモ_2026-09/)
    let nb_dir = base_out.join("Notebooks").join("アイデアメモ_2026-09");
    let nb_md = nb_dir.join("アイデアメモ_2026-09.md");
    assert!(nb_md.exists(), "Notebook markdown must exist");
    let nb_content = fs::read_to_string(&nb_md).unwrap();
    assert!(nb_content.contains("type: scribe-notebook"));
    assert!(nb_content.contains("![ページ 1](./page_1.svg)"));

    let nb_asset = nb_dir.join("page_1.svg");
    assert!(nb_asset.exists(), "Notebook page_1.svg asset must exist in same directory");

    // 4. Verify Deduplication (2nd Run should not duplicate)
    let stats_2nd = orchestrator.sync_from_path(&kindle_dir, &config, "Kindle Scribe (Test)", None).unwrap();
    assert_eq!(stats_2nd.highlights_added, 0);
    assert_eq!(stats_2nd.notes_added, 0);
    assert_eq!(stats_2nd.vocab_added, 0);
    assert_eq!(stats_2nd.notebooks_added, 0);

    // 5. Verify that modifying the notebook updates it on 3rd Run
    let file = fs::File::create(&sample_nbk_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);
    zip.start_file("page_1.svg", options).unwrap();
    std::io::Write::write_all(
        &mut zip,
        b"<svg xmlns=\"http://www.w3.org/2000/svg\"><text y=\"50\">Updated Handwritten Idea 2</text></svg>",
    ).unwrap();
    zip.finish().unwrap();

    let stats_3rd = orchestrator.sync_from_path(&kindle_dir, &config, "Kindle Scribe (Test)", None).unwrap();
    assert_eq!(stats_3rd.notebooks_added, 1);

    // 6. 4th Run without changes should not duplicate
    let stats_4th = orchestrator.sync_from_path(&kindle_dir, &config, "Kindle Scribe (Test)", None).unwrap();
    assert_eq!(stats_4th.notebooks_added, 0);

    // Clean up temp
    fs::remove_dir_all(&temp_root).ok();
}
