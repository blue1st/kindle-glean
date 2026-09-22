use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClippingType {
    Highlight,
    Note,
    Bookmark,
}

impl std::fmt::Display for ClippingType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClippingType::Highlight => write!(f, "Highlight"),
            ClippingType::Note => write!(f, "Note"),
            ClippingType::Bookmark => write!(f, "Bookmark"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clipping {
    pub id: String, // SHA-256 hash of title + location + content
    pub book_title: String,
    pub author: Option<String>,
    pub clipping_type: ClippingType,
    pub location: Option<String>,
    pub page: Option<u32>,
    pub created_at: Option<DateTime<Utc>>,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VocabLookup {
    pub id: String,
    pub word: String,
    pub stem: Option<String>,
    pub lang: Option<String>,
    pub usage: Option<String>,
    pub book_title: Option<String>,
    pub book_author: Option<String>,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotebookPage {
    pub page_num: usize,
    #[serde(default)]
    pub image_data: Vec<u8>,
    pub image_filename: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notebook {
    pub id: String,
    pub title: String,
    pub content_hash: String,
    pub last_modified: i64,
    pub pages: Vec<NotebookPage>,
    pub relative_folder: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    pub vault_path: String,
    pub sync_clippings: bool,
    pub sync_vocab: bool,
    pub sync_notebooks: bool,
    pub auto_sync: bool,
    pub auto_eject: bool,
    pub subfolder: String, // optional subfolder, default empty
}

impl Default for SyncConfig {
    fn default() -> Self {
        let default_dir = if let Some(user_dirs) = directories::UserDirs::new() {
            if let Some(doc_dir) = user_dirs.document_dir() {
                doc_dir.join("KindleGlean")
            } else {
                user_dirs.home_dir().join("Documents").join("KindleGlean")
            }
        } else {
            std::path::PathBuf::from("KindleGlean")
        };

        Self {
            vault_path: default_dir.to_string_lossy().to_string(),
            sync_clippings: true,
            sync_vocab: true,
            sync_notebooks: true,
            auto_sync: true,
            auto_eject: false,
            subfolder: String::new(),
        }
    }
}

pub fn resolve_path(path: &str) -> std::path::PathBuf {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return SyncConfig::default().vault_path.into();
    }
    if trimmed == "~" {
        if let Some(user_dirs) = directories::UserDirs::new() {
            return user_dirs.home_dir().to_path_buf();
        }
    } else if trimmed.starts_with("~/") || trimmed.starts_with("~\\") {
        if let Some(user_dirs) = directories::UserDirs::new() {
            let home = user_dirs.home_dir();
            return home.join(&trimmed[2..]);
        }
    }
    std::path::PathBuf::from(trimmed)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotebookSummary {
    pub id: String,
    pub title: String,
    pub display_title: String,
    pub content_hash: String,
    pub last_modified: i64,
    pub synced_at: String,
    pub relative_folder: String,
    pub cover_image_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyncStats {
    pub highlights_added: usize,
    pub notes_added: usize,
    pub vocab_added: usize,
    pub notebooks_added: usize,
    pub total_books_updated: usize,
    pub timestamp: String,
    pub device_name: String,
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub device_id: String,       // Hardware serial number or UUID (e.g. "G091...", "vol_uuid", etc.)
    pub device_type: String,     // "Paperwhite (UMS)", "Kindle Scribe (MTP)", "Custom Folder"
    pub connection_mode: String, // "UMS", "MTP", "Folder"
    pub mount_path: String,
    pub has_clippings: bool,
    pub has_vocab: bool,
    pub has_notebooks: bool,
    pub connected: bool,
    pub status_message: Option<String>,
    pub is_registered: bool,     // True if a profile exists in device_profiles table
    pub nickname: Option<String>,// Custom nickname if registered (e.g. "書斎のScribe")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceProfile {
    pub device_id: String,
    pub nickname: String,
    pub device_type: String,
    pub vault_path: String,
    pub subfolder: String,
    pub sync_clippings: bool,
    pub sync_vocab: bool,
    pub sync_notebooks: bool,
    pub auto_sync: bool,
    pub auto_eject: bool,
    pub created_at: String,
    pub last_connected_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncProgress {
    pub step: String,
    pub message: String,
    pub percentage: u32,
    pub current_item: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyncedCounts {
    pub highlights: usize,
    pub notes: usize,
    pub vocab: usize,
    pub notebooks: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_config_default_uses_document_dir() {
        let config = SyncConfig::default();
        assert!(!config.vault_path.is_empty());
        assert!(config.vault_path.contains("KindleGlean"));
    }

    #[test]
    fn test_resolve_path_expansion() {
        let home = directories::UserDirs::new()
            .map(|u| u.home_dir().to_path_buf())
            .unwrap();

        assert_eq!(resolve_path("~"), home);
        assert_eq!(resolve_path("~/KindleGlean"), home.join("KindleGlean"));
        assert_eq!(resolve_path("~\\KindleGlean"), home.join("KindleGlean"));
        assert_eq!(resolve_path("/tmp/vault"), std::path::PathBuf::from("/tmp/vault"));
    }
}

