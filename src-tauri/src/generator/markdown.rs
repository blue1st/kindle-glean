use crate::models::{Clipping, ClippingType, Notebook, VocabLookup};
use chrono::Utc;
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub struct MarkdownGenerator {
    base_dir: PathBuf,
}

impl MarkdownGenerator {
    pub fn new<P: AsRef<Path>>(vault_path: P, subfolder: &str) -> Self {
        let raw = vault_path.as_ref().to_string_lossy();
        let resolved = crate::models::resolve_path(&raw);
        let sub = subfolder.trim();
        let base_dir = if sub.is_empty() {
            resolved
        } else {
            resolved.join(sub)
        };
        Self { base_dir }
    }

    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    pub fn ensure_directories(&self) -> std::io::Result<()> {
        fs::create_dir_all(self.base_dir.join("Books"))?;
        fs::create_dir_all(self.base_dir.join("Notebooks"))?;
        fs::create_dir_all(self.base_dir.join("Vocabulary"))?;
        Ok(())
    }

    pub fn sanitize_filename(name: &str) -> String {
        let invalid_chars = ['/', '\\', '?', '%', '*', ':', '|', '"', '<', '>', '.', '\0'];
        let mut clean: String = name
            .chars()
            .map(|c| if invalid_chars.contains(&c) { '_' } else { c })
            .take(100)
            .collect();
        if clean.trim().is_empty() {
            clean = "Untitled".to_string();
        }
        clean
    }

    pub fn get_book_path(&self, book_title: &str) -> PathBuf {
        let safe_title = Self::sanitize_filename(book_title);
        self.base_dir.join("Books").join(format!("{}.md", safe_title))
    }

    pub fn get_vocab_path(&self) -> PathBuf {
        self.base_dir.join("Vocabulary").join("Kindle_Vocabulary.md")
    }

    pub fn get_notebook_dir(&self, notebook_title: &str, relative_folder: Option<&str>) -> PathBuf {
        let safe_title = Self::sanitize_filename(notebook_title);
        let mut dir = self.base_dir.join("Notebooks");
        if let Some(rel) = relative_folder {
            let trimmed = rel.trim();
            if !trimmed.is_empty() {
                dir = dir.join(trimmed);
            }
        }
        dir.join(&safe_title)
    }

    pub fn get_notebook_path(&self, notebook_title: &str, relative_folder: Option<&str>) -> PathBuf {
        let safe_title = Self::sanitize_filename(notebook_title);
        self.get_notebook_dir(notebook_title, relative_folder)
            .join(format!("{}.md", safe_title))
    }

    // --- Books Highlights ---
    pub fn update_book_clippings(
        &self,
        book_title: &str,
        author: Option<&str>,
        new_clippings: &[Clipping],
    ) -> std::io::Result<usize> {
        if new_clippings.is_empty() {
            return Ok(0);
        }

        let safe_title = Self::sanitize_filename(book_title);
        let file_path = self.base_dir.join("Books").join(format!("{}.md", safe_title));

        let now_str = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

        let mut existing_content = String::new();
        let exists = file_path.exists();

        let mut existing_snippets = HashSet::new();
        if exists {
            let mut f = fs::File::open(&file_path)?;
            f.read_to_string(&mut existing_content)?;
            // Track existing snippet lines to prevent duplicate additions
            for line in existing_content.lines() {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    existing_snippets.insert(trimmed.to_string());
                }
            }
        }

        let mut count_added = 0;
        let mut buffer = String::new();

        if !exists {
            let author_val = author.unwrap_or("Unknown");
            buffer.push_str("---\n");
            buffer.push_str("type: book-highlight\n");
            buffer.push_str(&format!("title: \"{}\"\n", book_title.replace('"', "\\\"")));
            buffer.push_str(&format!("author: \"{}\"\n", author_val.replace('"', "\\\"")));
            buffer.push_str(&format!("last_sync: \"{}\"\n", now_str));
            buffer.push_str("tags:\n");
            buffer.push_str("  - kindle/highlights\n");
            buffer.push_str("---\n\n");

            buffer.push_str(&format!("# {}\n\n", book_title));
            buffer.push_str("## メタデータ\n");
            buffer.push_str(&format!("- 著者: {}\n", author_val));
            buffer.push_str(&format!("- 最終同期: {}\n\n", now_str));
            buffer.push_str("## ハイライト・メモ一覧\n\n");
        }

        for c in new_clippings {
            // Check if snippet or location already exists in file
            let location_tag = c.location.as_deref().unwrap_or("N/A");
            let loc_str = format!("位置: {}", location_tag);
            let first_line = c.content.lines().next().unwrap_or("").trim();

            if exists && (!first_line.is_empty() && existing_snippets.contains(first_line)) {
                continue;
            }

            let date_str = c
                .created_at
                .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| "Unknown".to_string());

            match c.clipping_type {
                ClippingType::Highlight => {
                    buffer.push_str(&format!("> {}\n", c.content.replace('\n', "\n> ")));
                    buffer.push_str(&format!("— {} | 追加日: {}\n\n", loc_str, date_str));
                }
                ClippingType::Note => {
                    buffer.push_str(&format!("> 💡 **メモ:** {}\n", c.content.replace('\n', "\n> ")));
                    buffer.push_str(&format!("— {} | 追加日: {}\n\n", loc_str, date_str));
                }
                ClippingType::Bookmark => {
                    buffer.push_str(&format!("🔖 **しおり:** {} | 追加日: {}\n\n", loc_str, date_str));
                }
            }
            count_added += 1;
        }

        if count_added > 0 {
            if exists {
                let mut f = OpenOptions::new().append(true).open(&file_path)?;
                f.write_all(buffer.as_bytes())?;
            } else {
                fs::write(&file_path, buffer)?;
            }
        }

        Ok(count_added)
    }

    // --- Vocabulary ---
    pub fn update_vocabulary(&self, new_words: &[VocabLookup]) -> std::io::Result<usize> {
        if new_words.is_empty() {
            return Ok(0);
        }

        let file_path = self.base_dir.join("Vocabulary").join("Kindle_Vocabulary.md");
        let now_str = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

        let mut existing_content = String::new();
        let exists = file_path.exists();

        let mut existing_words = HashSet::new();
        if exists {
            let mut f = fs::File::open(&file_path)?;
            f.read_to_string(&mut existing_content)?;
            for line in existing_content.lines() {
                if line.starts_with("### ") {
                    let w = line.trim_start_matches("### ").trim();
                    existing_words.insert(w.to_lowercase());
                }
            }
        }

        let mut buffer = String::new();
        let mut count_added = 0;

        if !exists {
            buffer.push_str("---\n");
            buffer.push_str("type: kindle-vocabulary\n");
            buffer.push_str(&format!("last_sync: \"{}\"\n", now_str));
            buffer.push_str("tags:\n");
            buffer.push_str("  - kindle/vocabulary\n");
            buffer.push_str("---\n\n");
            buffer.push_str("# Kindle 語彙・単語ログ\n\n");
        }

        for w in new_words {
            if existing_words.contains(&w.word.to_lowercase()) {
                continue;
            }

            let date_str = chrono::DateTime::from_timestamp_millis(w.timestamp)
                .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| "Unknown".to_string());

            buffer.push_str(&format!("### {}\n", w.word));
            if let Some(stem) = &w.stem {
                if stem != &w.word {
                    buffer.push_str(&format!("- **原形:** `{}`\n", stem));
                }
            }
            if let Some(book) = &w.book_title {
                buffer.push_str(&format!("- **書籍:** [[{}]]\n", book));
            }
            buffer.push_str(&format!("- **検索日時:** {}\n", date_str));

            if let Some(usage) = &w.usage {
                buffer.push_str(&format!("> {}\n", usage.replace('\n', " ")));
            }
            buffer.push_str("\n---\n\n");

            existing_words.insert(w.word.to_lowercase());
            count_added += 1;
        }

        if count_added > 0 {
            if exists {
                let mut f = OpenOptions::new().append(true).open(&file_path)?;
                f.write_all(buffer.as_bytes())?;
            } else {
                fs::write(&file_path, buffer)?;
            }
        }

        Ok(count_added)
    }

    // --- Scribe Notebooks ---
    pub fn save_notebook(&self, notebook: &Notebook) -> std::io::Result<()> {
        let safe_title = Self::sanitize_filename(&notebook.title);
        let notebook_dir = self.get_notebook_dir(&notebook.title, notebook.relative_folder.as_deref());
        fs::create_dir_all(&notebook_dir)?;

        let mut image_links = Vec::new();

        for page in &notebook.pages {
            let asset_path = notebook_dir.join(&page.image_filename);
            fs::write(&asset_path, &page.image_data)?;

            // Standard universal markdown relative link: ![alt](./image_name)
            // (100% compatible with Obsidian, VS Code, Typora, GitHub, and any markdown viewer)
            image_links.push(format!(
                "### ページ {}\n![ページ {}](./{})\n",
                page.page_num, page.page_num, page.image_filename
            ));
        }

        let file_path = notebook_dir.join(format!("{}.md", safe_title));

        let now_str = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let display_title = if notebook.title.len() == 36 && notebook.title.contains('-') {
            format!("ノート ({})", &notebook.title[..8])
        } else {
            notebook.title.clone()
        };

        let mut buffer = String::new();
        buffer.push_str("---\n");
        buffer.push_str("type: scribe-notebook\n");
        buffer.push_str(&format!("id: \"{}\"\n", notebook.id));
        buffer.push_str(&format!("title: \"{}\"\n", display_title.replace('"', "\\\"")));
        buffer.push_str(&format!("device_uuid: \"{}\"\n", notebook.title));
        buffer.push_str(&format!("pages: {}\n", notebook.pages.len()));
        buffer.push_str(&format!("last_sync: \"{}\"\n", now_str));
        buffer.push_str("tags:\n");
        buffer.push_str("  - kindle/notebook\n");
        buffer.push_str("---\n\n");

        buffer.push_str(&format!("# {}\n\n", display_title));
        buffer.push_str(&format!("- **端末ノートID:** `{}`\n", notebook.title));
        buffer.push_str(&format!("- **ページ数:** {}\n", notebook.pages.len()));
        buffer.push_str(&format!("- **最終同期:** {}\n\n", now_str));

        buffer.push_str("> [!TIP] ノートのタイトルについて\n");
        buffer.push_str("> このノートはお好みの名前にリネーム（変更）可能です。内部IDで管理されているため、今後の同期で上書きされることはありません。\n\n");

        buffer.push_str("## ページ一覧\n\n");

        for link in image_links {
            buffer.push_str(&link);
            buffer.push('\n');
        }

        fs::write(file_path, buffer)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename_multibyte() {
        // Japanese string longer than 100 characters to verify no UTF-8 char boundary panics
        let long_title = "これは非常に長い日本語の書籍タイトルのテストです。マルチバイト文字が境界にある場合にパニックが起きないかどうかを確実に検証するための長い文字列です。さらに文字を続けていきます。";
        let sanitized = MarkdownGenerator::sanitize_filename(long_title);
        assert!(sanitized.chars().count() <= 100);
        assert!(!sanitized.is_empty());
    }
}

