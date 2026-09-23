use crate::db::Database;
use crate::generator::MarkdownGenerator;
use crate::models::{Clipping, ClippingType, SyncConfig, SyncStats, VocabLookup};
use crate::parsers::{ClippingsParser, NotebookParser, VocabParser};
use chrono::Utc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use walkdir::WalkDir;

pub struct SyncOrchestrator {
    db: Arc<Database>,
}

impl SyncOrchestrator {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    pub fn sync_from_path<P: AsRef<Path>>(
        &self,
        source_path: P,
        config: &SyncConfig,
        device_name: &str,
        on_progress: Option<&dyn Fn(crate::models::SyncProgress)>,
    ) -> Result<SyncStats, String> {
        let p = source_path.as_ref();
        if !p.exists() {
            return Err(format!("Source path '{}' does not exist", p.display()));
        }

        let generator = MarkdownGenerator::new(&config.vault_path, &config.subfolder);
        generator
            .ensure_directories()
            .map_err(|e| format!("Failed to create output directories in Vault: {}", e))?;

        let mut stats = SyncStats {
            timestamp: Utc::now().to_rfc3339(),
            device_name: device_name.to_string(),
            success: true,
            ..Default::default()
        };

        // 1. Sync Clippings
        if config.sync_clippings {
            if let Some(cb) = on_progress {
                cb(crate::models::SyncProgress {
                    step: "clippings".to_string(),
                    message: "ハイライト・読書メモをMarkdownに変換中...".to_string(),
                    percentage: 70,
                    current_item: Some("My Clippings.txt".to_string()),
                });
            }
            let clippings_path = self.find_file(p, &["documents/My Clippings.txt", "My Clippings.txt"]);
            if let Some(cpath) = clippings_path {
                match self.process_clippings(&cpath, &generator) {
                    Ok((highlights, notes, books_updated)) => {
                        stats.highlights_added += highlights;
                        stats.notes_added += notes;
                        stats.total_books_updated += books_updated;
                    }
                    Err(e) => {
                        log::error!("Error parsing clippings: {}", e);
                    }
                }
            }
        }

        // 2. Sync Vocabulary
        if config.sync_vocab {
            if let Some(cb) = on_progress {
                cb(crate::models::SyncProgress {
                    step: "vocab".to_string(),
                    message: "単語帳・語彙データを更新中...".to_string(),
                    percentage: 80,
                    current_item: Some("Kindle_Vocabulary.md".to_string()),
                });
            }
            let vocab_path = self.find_file(p, &["system/vocabulary/vocab.db", "vocab.db"]);
            if let Some(vpath) = vocab_path {
                match self.process_vocab(&vpath, &generator) {
                    Ok(count) => {
                        stats.vocab_added += count;
                    }
                    Err(e) => {
                        log::error!("Error parsing vocab: {}", e);
                    }
                }
            }
        }

        // 3. Sync Notebooks (Kindle Scribe)
        if config.sync_notebooks {
            if let Some(cb) = on_progress {
                cb(crate::models::SyncProgress {
                    step: "notebooks".to_string(),
                    message: "Scribe手書きノートを処理中...".to_string(),
                    percentage: 90,
                    current_item: None,
                });
            }
            let notebook_dir = self.find_dir(p, &[".notebooks", "notebooks"]);
            if let Some(ndir) = notebook_dir {
                match self.process_notebooks(&ndir, &generator) {
                    Ok(count) => {
                        stats.notebooks_added += count;
                    }
                    Err(e) => {
                        log::error!("Error parsing notebooks: {}", e);
                    }
                }
            }
        }

        if let Some(cb) = on_progress {
            cb(crate::models::SyncProgress {
                step: "completed".to_string(),
                message: "すべての同期が完了しました".to_string(),
                percentage: 100,
                current_item: None,
            });
        }

        stats.message = format!(
            "同期完了: ハイライト {}件, メモ {}件, 語彙 {}件, ノート {}件",
            stats.highlights_added, stats.notes_added, stats.vocab_added, stats.notebooks_added
        );

        // Record history
        self.db.add_history(&stats).ok();

        Ok(stats)
    }

    fn find_file(&self, base: &Path, candidates: &[&str]) -> Option<PathBuf> {
        for rel in candidates {
            let target = base.join(rel);
            if target.is_file() {
                return Some(target);
            }
        }
        None
    }

    fn find_dir(&self, base: &Path, candidates: &[&str]) -> Option<PathBuf> {
        for rel in candidates {
            let target = base.join(rel);
            if target.is_dir() {
                return Some(target);
            }
        }
        None
    }

    fn process_clippings(
        &self,
        path: &Path,
        generator: &MarkdownGenerator,
    ) -> Result<(usize, usize, usize), String> {
        let all_clippings = ClippingsParser::parse_file(path)?;

        // Group by book title
        let mut books_map: HashMap<String, (Option<String>, Vec<Clipping>)> = HashMap::new();
        for c in all_clippings {
            let entry = books_map
                .entry(c.book_title.clone())
                .or_insert_with(|| (c.author.clone(), Vec::new()));
            entry.1.push(c);
        }

        let mut highlights_count = 0;
        let mut notes_count = 0;
        let mut books_updated = 0;

        for (book_title, (author, list)) in &books_map {
            let book_file = generator.get_book_path(book_title);
            let file_exists = book_file.exists();

            // If the markdown file does not exist on disk, we pass all clippings to create it!
            // If the file already exists, only pass clippings that haven't been synced yet.
            let clippings_to_add: Vec<Clipping> = if file_exists {
                list.iter()
                    .filter(|c| {
                        match self.db.is_clipping_synced(&c.id) {
                            Ok(synced) => !synced,
                            Err(_) => true,
                        }
                    })
                    .cloned()
                    .collect()
            } else {
                list.clone()
            };

            if clippings_to_add.is_empty() {
                continue;
            }

            let added = generator
                .update_book_clippings(book_title, author.as_deref(), &clippings_to_add)
                .map_err(|e| e.to_string())?;

            if added > 0 {
                books_updated += 1;
                for c in &clippings_to_add {
                    match c.clipping_type {
                        crate::models::ClippingType::Highlight => highlights_count += 1,
                        crate::models::ClippingType::Note => notes_count += 1,
                        crate::models::ClippingType::Bookmark => {}
                    }
                    self.db.mark_clipping_synced(c).ok();
                }
            }
        }

        Ok((highlights_count, notes_count, books_updated))
    }

    fn process_vocab(&self, path: &Path, generator: &MarkdownGenerator) -> Result<usize, String> {
        let all_lookups = VocabParser::parse_file(path)?;

        let vocab_file = generator.get_vocab_path();
        let file_exists = vocab_file.exists();

        // If Vocabulary file does not exist, include all lookups to create it.
        // Otherwise, only include unsynced lookups.
        let items_to_add: Vec<VocabLookup> = if file_exists {
            all_lookups
                .into_iter()
                .filter(|item| {
                    match self.db.is_vocab_synced(&item.id) {
                        Ok(synced) => !synced,
                        Err(_) => true,
                    }
                })
                .collect()
        } else {
            all_lookups
        };

        if items_to_add.is_empty() {
            return Ok(0);
        }

        let added = generator
            .update_vocabulary(&items_to_add)
            .map_err(|e| e.to_string())?;

        for item in &items_to_add {
            self.db.mark_vocab_synced(item).ok();
        }

        Ok(added)
    }

    fn process_notebooks(&self, dir: &Path, generator: &MarkdownGenerator) -> Result<usize, String> {
        let mut count = 0;
        let mut processed_ids = std::collections::HashSet::new();

        for entry in WalkDir::new(dir).max_depth(4).into_iter().flatten() {
            let path = entry.path();
            if path.is_file() {
                let fname_lower = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                let ext_lower = path
                    .extension()
                    .map(|e| e.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                let is_nbk = ext_lower == "nbk" || ext_lower == "zip" || fname_lower == "nbk";

                if is_nbk {
                    let rel_folder = if fname_lower == "nbk" {
                        path.parent()
                            .and_then(|p| p.parent())
                            .and_then(|p| p.strip_prefix(dir).ok())
                            .map(|p| p.to_string_lossy().to_string())
                            .filter(|s| !s.is_empty())
                    } else {
                        path.parent()
                            .and_then(|p| p.strip_prefix(dir).ok())
                            .map(|p| p.to_string_lossy().to_string())
                            .filter(|s| !s.is_empty())
                    };

                    if let Ok(notebook) = NotebookParser::parse_file_with_folder(path, rel_folder.clone()) {
                        if !processed_ids.insert(notebook.id.clone()) {
                            continue;
                        }

                        let notebook_md = generator.get_notebook_path(&notebook.title, rel_folder.as_deref());
                        let file_exists = notebook_md.exists();
                        let is_synced = self.db.is_notebook_synced(&notebook.id, &notebook.content_hash).unwrap_or(false);
                        let notebook_dir = generator.get_notebook_dir(&notebook.title, rel_folder.as_deref());
                        let last_page_exists = notebook.pages.last()
                            .map(|p| notebook_dir.join(&p.image_filename).exists())
                            .unwrap_or(true);

                        // If markdown does not exist on disk, or if content_hash changed, or if newly extracted pages are missing:
                        if !file_exists || !is_synced || !last_page_exists {
                            if generator.save_notebook(&notebook).is_ok() {
                                self.db.mark_notebook_synced(
                                    &notebook.id,
                                    &notebook.title,
                                    &notebook.content_hash,
                                    notebook.last_modified,
                                    rel_folder.as_deref(),
                                ).ok();
                                count += 1;
                            }
                        }
                    }
                }
            }
        }

        Ok(count)
    }

    pub fn reexport_all(
        &self,
        config: &SyncConfig,
        on_progress: Option<&dyn Fn(crate::models::SyncProgress)>,
    ) -> Result<SyncStats, String> {
        let generator = MarkdownGenerator::new(&config.vault_path, &config.subfolder);
        generator
            .ensure_directories()
            .map_err(|e| format!("出力先ディレクトリの作成に失敗しました: {}", e))?;

        let mut stats = SyncStats {
            timestamp: Utc::now().to_rfc3339(),
            device_name: "ライブラリ全再出力".to_string(),
            success: true,
            ..Default::default()
        };

        // 1. Re-export clippings from DB
        if config.sync_clippings {
            if let Some(cb) = on_progress {
                cb(crate::models::SyncProgress {
                    step: "clippings".to_string(),
                    message: "ハイライト・メモを再出力中...".to_string(),
                    percentage: 30,
                    current_item: None,
                });
            }
            if let Ok(all_clippings) = self.db.get_all_synced_clippings() {
                let mut books_map: HashMap<String, (Option<String>, Vec<Clipping>)> = HashMap::new();
                for c in all_clippings {
                    let entry = books_map
                        .entry(c.book_title.clone())
                        .or_insert_with(|| (c.author.clone(), Vec::new()));
                    entry.1.push(c);
                }

                for (book_title, (author, list)) in &books_map {
                    if let Ok(added) = generator.update_book_clippings(book_title, author.as_deref(), list) {
                        if added > 0 {
                            stats.total_books_updated += 1;
                        }
                        stats.highlights_added += list.iter().filter(|c| c.clipping_type == ClippingType::Highlight).count();
                        stats.notes_added += list.iter().filter(|c| c.clipping_type == ClippingType::Note).count();
                    }
                }
            }
        }

        // 2. Re-export vocabulary from DB
        if config.sync_vocab {
            if let Some(cb) = on_progress {
                cb(crate::models::SyncProgress {
                    step: "vocab".to_string(),
                    message: "単語帳・語彙データを再出力中...".to_string(),
                    percentage: 60,
                    current_item: None,
                });
            }
            if let Ok(all_vocab) = self.db.get_all_synced_vocab() {
                if let Ok(added) = generator.update_vocabulary(&all_vocab) {
                    stats.vocab_added = added;
                }
            }
        }

        // 3. Re-export notebooks from cache or connected device
        if config.sync_notebooks {
            if let Some(cb) = on_progress {
                cb(crate::models::SyncProgress {
                    step: "notebooks".to_string(),
                    message: "Scribeノートを再出力中...".to_string(),
                    percentage: 85,
                    current_item: None,
                });
            }
            let cache_dir = std::env::temp_dir().join("kindle_scribe_cache");
            let nb_dir = cache_dir.join(".notebooks");
            if nb_dir.is_dir() {
                if let Ok(count) = self.process_notebooks(&nb_dir, &generator) {
                    stats.notebooks_added = count;
                }
            }
        }

        if let Some(cb) = on_progress {
            cb(crate::models::SyncProgress {
                step: "completed".to_string(),
                message: "再出力が完了しました".to_string(),
                percentage: 100,
                current_item: None,
            });
        }

        stats.message = format!(
            "再出力完了: 書籍 {}冊, 語彙 {}件, ノート {}冊",
            stats.total_books_updated, stats.vocab_added, stats.notebooks_added
        );

        self.db.add_history(&stats).ok();

        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_sync_clippings_and_vocab_flow() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let orchestrator = SyncOrchestrator::new(db.clone());

        // Create temporary source folder mimicking a Kindle
        let temp_dir = std::env::temp_dir().join(format!("kindle_test_{}", Utc::now().timestamp_nanos_opt().unwrap_or(0)));
        let kindle_docs = temp_dir.join("documents");
        let kindle_sys = temp_dir.join("system").join("vocabulary");
        fs::create_dir_all(&kindle_docs).unwrap();
        fs::create_dir_all(&kindle_sys).unwrap();

        // Write clippings sample
        let sample_clippings = r#"The Pragmatic Programmer (Andy Hunt)
- Your Highlight on page 42 | location 500 | Added on Tuesday, September 15, 2026 8:43:10 PM

Care About Your Craft.
==========
The Pragmatic Programmer (Andy Hunt)
- Your Note on page 42 | location 501 | Added on Tuesday, September 15, 2026 8:45:00 PM

Essential advice for all developers!
==========
"#;
        fs::write(kindle_docs.join("My Clippings.txt"), sample_clippings).unwrap();

        // Target Vault
        let vault_dir = temp_dir.join("Vault");
        fs::create_dir_all(&vault_dir).unwrap();

        let config = SyncConfig {
            vault_path: vault_dir.to_string_lossy().to_string(),
            sync_clippings: true,
            sync_vocab: true,
            sync_notebooks: true,
            auto_sync: true,
            auto_eject: false,
            subfolder: "Kindle".to_string(),
        };

        // First Sync
        let stats = orchestrator.sync_from_path(&temp_dir, &config, "Test Kindle", None).unwrap();
        assert_eq!(stats.highlights_added, 1);
        assert_eq!(stats.notes_added, 1);
        assert_eq!(stats.total_books_updated, 1);

        // Verify generated book markdown
        let book_md = vault_dir.join("Kindle").join("Books").join("The Pragmatic Programmer.md");
        assert!(book_md.exists());
        let content = fs::read_to_string(&book_md).unwrap();
        assert!(content.contains("Care About Your Craft."));
        assert!(content.contains("Essential advice for all developers!"));

        // Second Sync (Deduplication check - nothing new should be added)
        let stats2 = orchestrator.sync_from_path(&temp_dir, &config, "Test Kindle", None).unwrap();
        assert_eq!(stats2.highlights_added, 0);
        assert_eq!(stats2.notes_added, 0);

        // Clean up
        fs::remove_dir_all(&temp_dir).ok();
    }
}
