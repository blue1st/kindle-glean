use crate::models::{
    Clipping, ClippingType, DeviceProfile, NotebookSummary, SyncConfig, SyncStats, SyncedCounts,
    VocabLookup,
};
use directories::ProjectDirs;
use rusqlite::{params, Connection, Result};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn init() -> Result<Self> {
        let db_path = Self::get_db_path();
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent).ok();
        }

        let conn = Connection::open(&db_path)?;
        let db = Database {
            conn: Mutex::new(conn),
        };
        db.create_tables()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Database {
            conn: Mutex::new(conn),
        };
        db.create_tables()?;
        Ok(db)
    }

    fn get_db_path() -> PathBuf {
        if let Some(proj_dirs) = ProjectDirs::from("com", "kindleglean", "KindleGlean") {
            let new_path = proj_dirs.data_local_dir().join("glean_state.db");
            // Seamless migration from legacy db if exists
            if !new_path.exists() {
                if let Some(old_dirs) = ProjectDirs::from("com", "kindlesync", "KindleLocalSync") {
                    let old_path = old_dirs.data_local_dir().join("sync_state.db");
                    if old_path.exists() {
                        if let Some(parent) = new_path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        let _ = std::fs::copy(&old_path, &new_path);
                    }
                }
            }
            new_path
        } else {
            PathBuf::from("kindle_glean_state.db")
        }
    }

    fn create_tables(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        // Config table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
            [],
        )?;

        // Synced clippings table (deduplication)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS synced_clippings (
                id TEXT PRIMARY KEY,
                book_title TEXT NOT NULL,
                author TEXT,
                clipping_type TEXT NOT NULL,
                location TEXT,
                page INTEGER,
                content TEXT NOT NULL,
                created_at TEXT,
                synced_at TEXT NOT NULL
            )",
            [],
        )?;
        let _ = conn.execute("ALTER TABLE synced_clippings ADD COLUMN author TEXT", []);

        // Synced vocabulary table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS synced_vocab (
                id TEXT PRIMARY KEY,
                word TEXT NOT NULL,
                stem TEXT,
                lang TEXT,
                usage TEXT,
                book_title TEXT,
                timestamp INTEGER NOT NULL,
                synced_at TEXT NOT NULL
            )",
            [],
        )?;
        let _ = conn.execute("ALTER TABLE synced_vocab ADD COLUMN usage TEXT", []);

        // Synced notebooks table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS synced_notebooks (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                content_hash TEXT,
                last_modified INTEGER NOT NULL,
                synced_at TEXT NOT NULL
            )",
            [],
        )?;
        let _ = conn.execute("ALTER TABLE synced_notebooks ADD COLUMN content_hash TEXT", []);
        let _ = conn.execute("ALTER TABLE synced_notebooks ADD COLUMN relative_folder TEXT", []);
        let _ = conn.execute("DELETE FROM synced_notebooks WHERE id LIKE 'notebook_%' OR id LIKE 'nbk:notebook_%'", []);

        // Sync history logs
        conn.execute(
            "CREATE TABLE IF NOT EXISTS sync_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                device_name TEXT NOT NULL,
                highlights_added INTEGER NOT NULL,
                notes_added INTEGER NOT NULL,
                vocab_added INTEGER NOT NULL,
                notebooks_added INTEGER NOT NULL,
                success INTEGER NOT NULL,
                message TEXT
            )",
            [],
        )?;

        // Device Profiles table (per-device settings)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS device_profiles (
                device_id TEXT PRIMARY KEY,
                nickname TEXT NOT NULL,
                device_type TEXT NOT NULL,
                vault_path TEXT NOT NULL,
                subfolder TEXT NOT NULL,
                sync_clippings INTEGER NOT NULL DEFAULT 1,
                sync_vocab INTEGER NOT NULL DEFAULT 1,
                sync_notebooks INTEGER NOT NULL DEFAULT 1,
                auto_sync INTEGER NOT NULL DEFAULT 1,
                auto_eject INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                last_connected_at TEXT NOT NULL
            )",
            [],
        )?;

        Ok(())
    }

    // --- Config ---
    pub fn get_config(&self) -> Result<SyncConfig> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT key, value FROM config")?;
        let rows = stmt.query_map([], |row| {
            let k: String = row.get(0)?;
            let v: String = row.get(1)?;
            Ok((k, v))
        })?;

        let mut config = SyncConfig::default();
        for row in rows.flatten() {
            match row.0.as_str() {
                "vault_path" => config.vault_path = row.1,
                "sync_clippings" => config.sync_clippings = row.1 == "true",
                "sync_vocab" => config.sync_vocab = row.1 == "true",
                "sync_notebooks" => config.sync_notebooks = row.1 == "true",
                "auto_sync" => config.auto_sync = row.1 == "true",
                "auto_eject" => config.auto_eject = row.1 == "true",
                "subfolder" => config.subfolder = row.1,
                _ => {}
            }
        }
        Ok(config)
    }

    pub fn save_config(&self, config: &SyncConfig) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let entries = [
            ("vault_path", config.vault_path.clone()),
            ("sync_clippings", config.sync_clippings.to_string()),
            ("sync_vocab", config.sync_vocab.to_string()),
            ("sync_notebooks", config.sync_notebooks.to_string()),
            ("auto_sync", config.auto_sync.to_string()),
            ("auto_eject", config.auto_eject.to_string()),
            ("subfolder", config.subfolder.clone()),
        ];

        for (k, v) in entries {
            conn.execute(
                "INSERT INTO config (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![k, v],
            )?;
        }
        Ok(())
    }

    // --- Clippings Deduplication ---
    pub fn is_clipping_synced(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM synced_clippings WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn mark_clipping_synced(&self, clipping: &Clipping) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        let created_at_str = clipping.created_at.map(|t| t.to_rfc3339());

        conn.execute(
            "INSERT INTO synced_clippings (id, book_title, author, clipping_type, location, page, content, created_at, synced_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(id) DO UPDATE SET
                author = excluded.author,
                book_title = excluded.book_title",
            params![
                clipping.id,
                clipping.book_title,
                clipping.author,
                clipping.clipping_type.to_string(),
                clipping.location,
                clipping.page,
                clipping.content,
                created_at_str,
                now
            ],
        )?;
        Ok(())
    }

    // --- Vocabulary Deduplication ---
    pub fn is_vocab_synced(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM synced_vocab WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn mark_vocab_synced(&self, item: &VocabLookup) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO synced_vocab (id, word, stem, lang, usage, book_title, timestamp, synced_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
                usage = excluded.usage,
                book_title = excluded.book_title",
            params![
                item.id,
                item.word,
                item.stem,
                item.lang,
                item.usage,
                item.book_title,
                item.timestamp,
                now
            ],
        )?;
        Ok(())
    }

    // --- Notebook Deduplication ---
    pub fn is_notebook_synced(&self, id: &str, content_hash: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let stored_hash: Option<String> = conn
            .query_row(
                "SELECT content_hash FROM synced_notebooks WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .ok();

        match stored_hash {
            Some(h) => Ok(h == content_hash),
            None => Ok(false),
        }
    }

    pub fn mark_notebook_synced(
        &self,
        id: &str,
        title: &str,
        content_hash: &str,
        last_modified: i64,
        relative_folder: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO synced_notebooks (id, title, content_hash, last_modified, synced_at, relative_folder)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                content_hash = excluded.content_hash,
                last_modified = excluded.last_modified,
                synced_at = excluded.synced_at,
                relative_folder = excluded.relative_folder",
            params![id, title, content_hash, last_modified, now, relative_folder],
        )?;
        Ok(())
    }

    // --- History ---
    pub fn add_history(&self, stats: &SyncStats) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO sync_history (timestamp, device_name, highlights_added, notes_added, vocab_added, notebooks_added, success, message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                stats.timestamp,
                stats.device_name,
                stats.highlights_added as i64,
                stats.notes_added as i64,
                stats.vocab_added as i64,
                stats.notebooks_added as i64,
                if stats.success { 1 } else { 0 },
                stats.message
            ],
        )?;
        Ok(())
    }

    pub fn get_recent_history(&self, limit: usize) -> Result<Vec<SyncStats>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT timestamp, device_name, highlights_added, notes_added, vocab_added, notebooks_added, success, message
             FROM sync_history
             ORDER BY id DESC LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let success_int: i32 = row.get(6)?;
            Ok(SyncStats {
                timestamp: row.get(0)?,
                device_name: row.get(1)?,
                highlights_added: row.get::<_, i64>(2)? as usize,
                notes_added: row.get::<_, i64>(3)? as usize,
                vocab_added: row.get::<_, i64>(4)? as usize,
                notebooks_added: row.get::<_, i64>(5)? as usize,
                total_books_updated: 0,
                success: success_int == 1,
                message: row.get(7)?,
            })
        })?;

        let mut results = Vec::new();
        for r in rows.flatten() {
            results.push(r);
        }
        Ok(results)
    }

    // --- Synced Content Queries for In-App Viewer ---
    pub fn get_all_synced_clippings(&self) -> Result<Vec<Clipping>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, book_title, author, clipping_type, location, page, content, created_at
             FROM synced_clippings
             ORDER BY id DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            let type_str: String = row.get(3)?;
            let ctype = match type_str.to_lowercase().as_str() {
                "note" => ClippingType::Note,
                "bookmark" => ClippingType::Bookmark,
                _ => ClippingType::Highlight,
            };

            let created_at_str: Option<String> = row.get(7)?;
            let created_at = created_at_str.and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(&s)
                    .ok()
                    .map(|dt| dt.with_timezone(&chrono::Utc))
            });

            Ok(Clipping {
                id: row.get(0)?,
                book_title: row.get(1)?,
                author: row.get(2)?,
                clipping_type: ctype,
                location: row.get(4)?,
                page: row.get::<_, Option<i64>>(5)?.map(|p| p as u32),
                content: row.get(6)?,
                created_at,
            })
        })?;

        let mut list = Vec::new();
        for r in rows.flatten() {
            list.push(r);
        }
        Ok(list)
    }

    pub fn get_all_synced_vocab(&self) -> Result<Vec<VocabLookup>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, word, stem, lang, usage, book_title, timestamp
             FROM synced_vocab
             ORDER BY timestamp DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(VocabLookup {
                id: row.get(0)?,
                word: row.get(1)?,
                stem: row.get(2)?,
                lang: row.get(3)?,
                usage: row.get(4)?,
                book_title: row.get(5)?,
                book_author: None,
                timestamp: row.get(6)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows.flatten() {
            list.push(r);
        }
        Ok(list)
    }

    pub fn get_all_synced_notebooks(&self) -> Result<Vec<NotebookSummary>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title, content_hash, last_modified, synced_at, relative_folder
             FROM synced_notebooks
             ORDER BY synced_at DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let title: String = row.get(1)?;
            let content_hash: String = row.get(2)?;
            let last_modified: i64 = row.get(3)?;
            let synced_at: String = row.get(4)?;
            let rel_folder: Option<String> = row.get(5).ok().flatten();

            let display_title = if title.len() == 36 && title.contains('-') {
                format!("ノート ({})", &title[..8])
            } else {
                title.clone()
            };

            let safe_title = crate::generator::MarkdownGenerator::sanitize_filename(&title);
            let relative_folder = if let Some(ref rf) = rel_folder {
                let trimmed = rf.trim();
                if !trimmed.is_empty() {
                    format!("Notebooks/{}/{}", trimmed, safe_title)
                } else {
                    format!("Notebooks/{}", safe_title)
                }
            } else {
                format!("Notebooks/{}", safe_title)
            };

            Ok(NotebookSummary {
                id,
                title,
                display_title,
                content_hash,
                last_modified,
                synced_at,
                relative_folder,
                cover_image_path: None,
            })
        })?;

        let mut list = Vec::new();
        for r in rows.flatten() {
            list.push(r);
        }
        Ok(list)
    }

    pub fn get_synced_counts(&self) -> Result<SyncedCounts> {
        let conn = self.conn.lock().unwrap();
        let highlights: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM synced_clippings WHERE LOWER(clipping_type) = 'highlight'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let notes: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM synced_clippings WHERE LOWER(clipping_type) = 'note'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let vocab: i64 = conn
            .query_row("SELECT COUNT(*) FROM synced_vocab", [], |row| row.get(0))
            .unwrap_or(0);

        let notebooks: i64 = conn
            .query_row("SELECT COUNT(*) FROM synced_notebooks", [], |row| {
                row.get(0)
            })
            .unwrap_or(0);

        Ok(SyncedCounts {
            highlights: highlights as usize,
            notes: notes as usize,
            vocab: vocab as usize,
            notebooks: notebooks as usize,
        })
    }

    // --- Device Profiles ---
    pub fn get_device_profile(&self, device_id: &str) -> Result<Option<DeviceProfile>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT device_id, nickname, device_type, vault_path, subfolder,
                    sync_clippings, sync_vocab, sync_notebooks, auto_sync, auto_eject,
                    created_at, last_connected_at
             FROM device_profiles WHERE device_id = ?1",
        )?;

        let mut rows = stmt.query_map(params![device_id], |row| {
            let sync_clippings_int: i64 = row.get(5)?;
            let sync_vocab_int: i64 = row.get(6)?;
            let sync_notebooks_int: i64 = row.get(7)?;
            let auto_sync_int: i64 = row.get(8)?;
            let auto_eject_int: i64 = row.get(9)?;

            Ok(DeviceProfile {
                device_id: row.get(0)?,
                nickname: row.get(1)?,
                device_type: row.get(2)?,
                vault_path: row.get(3)?,
                subfolder: row.get(4)?,
                sync_clippings: sync_clippings_int != 0,
                sync_vocab: sync_vocab_int != 0,
                sync_notebooks: sync_notebooks_int != 0,
                auto_sync: auto_sync_int != 0,
                auto_eject: auto_eject_int != 0,
                created_at: row.get(10)?,
                last_connected_at: row.get(11)?,
            })
        })?;

        match rows.next() {
            Some(Ok(p)) => Ok(Some(p)),
            Some(Err(e)) => Err(e),
            None => Ok(None),
        }
    }

    pub fn save_device_profile(&self, profile: &DeviceProfile) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO device_profiles (
                device_id, nickname, device_type, vault_path, subfolder,
                sync_clippings, sync_vocab, sync_notebooks, auto_sync, auto_eject,
                created_at, last_connected_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(device_id) DO UPDATE SET
                nickname = excluded.nickname,
                device_type = excluded.device_type,
                vault_path = excluded.vault_path,
                subfolder = excluded.subfolder,
                sync_clippings = excluded.sync_clippings,
                sync_vocab = excluded.sync_vocab,
                sync_notebooks = excluded.sync_notebooks,
                auto_sync = excluded.auto_sync,
                auto_eject = excluded.auto_eject,
                last_connected_at = excluded.last_connected_at",
            params![
                profile.device_id,
                profile.nickname,
                profile.device_type,
                profile.vault_path,
                profile.subfolder,
                if profile.sync_clippings { 1 } else { 0 },
                if profile.sync_vocab { 1 } else { 0 },
                if profile.sync_notebooks { 1 } else { 0 },
                if profile.auto_sync { 1 } else { 0 },
                if profile.auto_eject { 1 } else { 0 },
                profile.created_at,
                profile.last_connected_at,
            ],
        )?;
        Ok(())
    }

    pub fn update_device_last_connected(&self, device_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE device_profiles SET last_connected_at = ?1 WHERE device_id = ?2",
            params![now, device_id],
        )?;
        Ok(())
    }

    pub fn get_all_device_profiles(&self) -> Result<Vec<DeviceProfile>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT device_id, nickname, device_type, vault_path, subfolder,
                    sync_clippings, sync_vocab, sync_notebooks, auto_sync, auto_eject,
                    created_at, last_connected_at
             FROM device_profiles
             ORDER BY last_connected_at DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            let sync_clippings_int: i64 = row.get(5)?;
            let sync_vocab_int: i64 = row.get(6)?;
            let sync_notebooks_int: i64 = row.get(7)?;
            let auto_sync_int: i64 = row.get(8)?;
            let auto_eject_int: i64 = row.get(9)?;

            Ok(DeviceProfile {
                device_id: row.get(0)?,
                nickname: row.get(1)?,
                device_type: row.get(2)?,
                vault_path: row.get(3)?,
                subfolder: row.get(4)?,
                sync_clippings: sync_clippings_int != 0,
                sync_vocab: sync_vocab_int != 0,
                sync_notebooks: sync_notebooks_int != 0,
                auto_sync: auto_sync_int != 0,
                auto_eject: auto_eject_int != 0,
                created_at: row.get(10)?,
                last_connected_at: row.get(11)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows.flatten() {
            list.push(r);
        }
        Ok(list)
    }

    pub fn delete_device_profile(&self, device_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM device_profiles WHERE device_id = ?1", params![device_id])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_profile_crud() {
        let db = Database::open_in_memory().expect("In-memory db should open");

        // 1. None initially
        let p = db.get_device_profile("G091_TEST_SCRIBE").unwrap();
        assert!(p.is_none());

        // 2. Save profile
        let profile1 = DeviceProfile {
            device_id: "G091_TEST_SCRIBE".to_string(),
            nickname: "書斎のKindle Scribe".to_string(),
            device_type: "Kindle Scribe (MTP)".to_string(),
            vault_path: "/test/vault1".to_string(),
            subfolder: "ScribeNotes".to_string(),
            sync_clippings: true,
            sync_vocab: true,
            sync_notebooks: true,
            auto_sync: true,
            auto_eject: false,
            created_at: "2026-09-22T00:00:00Z".to_string(),
            last_connected_at: "2026-09-22T00:00:00Z".to_string(),
        };
        db.save_device_profile(&profile1).expect("Should save profile");

        // 3. Read profile
        let saved = db.get_device_profile("G091_TEST_SCRIBE").unwrap();
        assert!(saved.is_some());
        let saved = saved.unwrap();
        assert_eq!(saved.nickname, "書斎のKindle Scribe");
        assert_eq!(saved.subfolder, "ScribeNotes");

        // 4. Save second profile
        let profile2 = DeviceProfile {
            device_id: "G090_TEST_PAPERWHITE".to_string(),
            nickname: "通勤用Paperwhite".to_string(),
            device_type: "Kindle Paperwhite (UMS)".to_string(),
            vault_path: "/test/vault2".to_string(),
            subfolder: "".to_string(),
            sync_clippings: true,
            sync_vocab: false,
            sync_notebooks: false,
            auto_sync: false,
            auto_eject: true,
            created_at: "2026-09-22T01:00:00Z".to_string(),
            last_connected_at: "2026-09-22T01:00:00Z".to_string(),
        };
        db.save_device_profile(&profile2).expect("Should save profile 2");

        // 5. Get all profiles
        let all = db.get_all_device_profiles().unwrap();
        assert_eq!(all.len(), 2);

        // 6. Update last connected
        db.update_device_last_connected("G091_TEST_SCRIBE").unwrap();
        let updated = db.get_device_profile("G091_TEST_SCRIBE").unwrap().unwrap();
        assert_ne!(updated.last_connected_at, "2026-09-22T00:00:00Z");

        // 7. Delete profile
        db.delete_device_profile("G090_TEST_PAPERWHITE").unwrap();
        let remaining = db.get_all_device_profiles().unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].device_id, "G091_TEST_SCRIBE");
    }

    #[test]
    fn test_get_synced_counts() {
        let db = Database::open_in_memory().expect("In-memory db should open");

        // Initially all zero
        let initial_counts = db.get_synced_counts().unwrap();
        assert_eq!(initial_counts.highlights, 0);
        assert_eq!(initial_counts.notes, 0);
        assert_eq!(initial_counts.vocab, 0);
        assert_eq!(initial_counts.notebooks, 0);

        // Add 2 highlights and 1 note
        let h1 = Clipping {
            id: "h1".to_string(),
            book_title: "Book A".to_string(),
            author: None,
            clipping_type: ClippingType::Highlight,
            location: None,
            page: None,
            created_at: None,
            content: "Highlight 1".to_string(),
        };
        let h2 = Clipping {
            id: "h2".to_string(),
            book_title: "Book A".to_string(),
            author: None,
            clipping_type: ClippingType::Highlight,
            location: None,
            page: None,
            created_at: None,
            content: "Highlight 2".to_string(),
        };
        let n1 = Clipping {
            id: "n1".to_string(),
            book_title: "Book A".to_string(),
            author: None,
            clipping_type: ClippingType::Note,
            location: None,
            page: None,
            created_at: None,
            content: "Note 1".to_string(),
        };
        db.mark_clipping_synced(&h1).unwrap();
        db.mark_clipping_synced(&h2).unwrap();
        db.mark_clipping_synced(&n1).unwrap();

        // Add 1 vocab
        let v1 = VocabLookup {
            id: "v1".to_string(),
            word: "rust".to_string(),
            stem: None,
            lang: None,
            usage: None,
            book_title: None,
            book_author: None,
            timestamp: 12345,
        };
        db.mark_vocab_synced(&v1).unwrap();

        // Add 2 notebooks
        db.mark_notebook_synced("nbk:1", "Note 1", "hash1", 1000, None).unwrap();
        db.mark_notebook_synced("nbk:2", "Note 2", "hash2", 1000, None).unwrap();

        // Check counts
        let counts = db.get_synced_counts().unwrap();
        assert_eq!(counts.highlights, 2);
        assert_eq!(counts.notes, 1);
        assert_eq!(counts.vocab, 1);
        assert_eq!(counts.notebooks, 2);

        // Re-syncing the same notebook (updating content hash) should NOT increase notebook count
        db.mark_notebook_synced("nbk:1", "Note 1", "hash1_updated", 2000, None).unwrap();
        let counts_after_update = db.get_synced_counts().unwrap();
        assert_eq!(counts_after_update.notebooks, 2);
    }
}

