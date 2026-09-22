use crate::models::VocabLookup;
use rusqlite::{Connection, OpenFlags, Result};
use std::path::Path;

pub struct VocabParser;

impl VocabParser {
    pub fn parse_file<P: AsRef<Path>>(path: P) -> Result<Vec<VocabLookup>, String> {
        let conn = Connection::open_with_flags(
            path.as_ref(),
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| format!("Failed to open vocab.db: {}", e))?;

        Self::parse_connection(&conn).map_err(|e| format!("Failed to query vocab.db: {}", e))
    }

    pub fn parse_connection(conn: &Connection) -> Result<Vec<VocabLookup>> {
        let sql = r#"
            SELECT 
                l.id,
                w.word,
                w.stem,
                w.lang,
                l.usage,
                b.title,
                b.authors,
                l.timestamp
            FROM LOOKUPS l
            JOIN WORDS w ON l.word_key = w.id
            LEFT JOIN BOOK_INFO b ON l.book_key = b.id
            ORDER BY l.timestamp ASC
        "#;

        let mut stmt = match conn.prepare(sql) {
            Ok(stmt) => stmt,
            Err(_) => {
                // Fallback: If LOOKUPS does not exist or schema differs slightly, query WORDS directly
                let fallback_sql = "SELECT id, word, stem, lang, timestamp FROM WORDS";
                let mut f_stmt = conn.prepare(fallback_sql)?;
                let rows = f_stmt.query_map([], |row| {
                    Ok(VocabLookup {
                        id: row.get(0)?,
                        word: row.get(1)?,
                        stem: row.get(2)?,
                        lang: row.get(3)?,
                        usage: None,
                        book_title: None,
                        book_author: None,
                        timestamp: row.get(4)?,
                    })
                })?;
                let mut results = Vec::new();
                for r in rows.flatten() {
                    results.push(r);
                }
                return Ok(results);
            }
        };

        let rows = stmt.query_map([], |row| {
            Ok(VocabLookup {
                id: row.get(0)?,
                word: row.get(1)?,
                stem: row.get(2)?,
                lang: row.get(3)?,
                usage: row.get(4)?,
                book_title: row.get(5)?,
                book_author: row.get(6)?,
                timestamp: row.get(7)?,
            })
        })?;

        let mut results = Vec::new();
        for r in rows.flatten() {
            results.push(r);
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vocab_db_query() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE WORDS (id TEXT PRIMARY KEY, word TEXT, stem TEXT, lang TEXT, category INTEGER, timestamp INTEGER);
            CREATE TABLE BOOK_INFO (id TEXT PRIMARY KEY, asin TEXT, title TEXT, authors TEXT);
            CREATE TABLE LOOKUPS (id TEXT PRIMARY KEY, word_key TEXT, book_key TEXT, dict_key TEXT, usage TEXT, timestamp INTEGER);

            INSERT INTO WORDS VALUES ('en:ephemeral', 'ephemeral', 'ephemeral', 'en', 0, 1695000000000);
            INSERT INTO BOOK_INFO VALUES ('B001', 'B001', 'Clean Code', 'Robert Martin');
            INSERT INTO LOOKUPS VALUES ('look1', 'en:ephemeral', 'B001', 'dict1', 'Knowledge in software is often ephemeral.', 1695000000000);
        "#
        ).unwrap();

        let items = VocabParser::parse_connection(&conn).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].word, "ephemeral");
        assert_eq!(items[0].book_title, Some("Clean Code".to_string()));
        assert_eq!(
            items[0].usage,
            Some("Knowledge in software is often ephemeral.".to_string())
        );
    }
}
