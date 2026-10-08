use crate::db::Database;
use crate::models::FileTransferResult;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

pub struct FilePusher;

impl FilePusher {
    /// Supported Kindle file extensions for direct USB transfer
    pub const SUPPORTED_EXTS: &'static [&'static str] = &[
        "pdf", "kfx", "azw3", "azw", "mobi", "prc", "txt",
    ];

    /// Check if a file extension is natively supported by Kindle direct transfer
    pub fn is_supported(path: &Path) -> bool {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        Self::SUPPORTED_EXTS.contains(&ext.as_str())
    }

    /// Check if a file is an EPUB file
    pub fn is_epub(path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("epub"))
            .unwrap_or(false)
    }

    /// Computes SHA-256 hash of a file for deduplication
    pub fn compute_file_hash(path: &Path) -> Result<String, String> {
        let mut file = fs::File::open(path).map_err(|e| format!("ファイルを開けませんでした: {}", e))?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 65536];

        loop {
            let n = file
                .read(&mut buffer)
                .map_err(|e| format!("ファイル読み込みエラー: {}", e))?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }

        Ok(format!("{:x}", hasher.finalize()))
    }

    /// Detect if Calibre's ebook-convert CLI tool is installed on the system
    pub fn find_ebook_convert() -> Option<PathBuf> {
        // 1. Check known absolute installation paths
        #[cfg(target_os = "macos")]
        {
            let mac_path = PathBuf::from("/Applications/calibre.app/Contents/MacOS/ebook-convert");
            if mac_path.exists() {
                return Some(mac_path);
            }
        }

        #[cfg(target_os = "windows")]
        {
            let win_paths = [
                PathBuf::from(r"C:\Program Files\Calibre2\ebook-convert.exe"),
                PathBuf::from(r"C:\Program Files (x86)\Calibre2\ebook-convert.exe"),
                PathBuf::from(r"C:\Calibre2\ebook-convert.exe"),
            ];
            for p in &win_paths {
                if p.exists() {
                    return Some(p.clone());
                }
            }
        }

        // 2. Fall back to system PATH
        if let Ok(path_var) = std::env::var("PATH") {
            let sep = if cfg!(target_os = "windows") { ';' } else { ':' };
            let exe_name = if cfg!(target_os = "windows") { "ebook-convert.exe" } else { "ebook-convert" };
            for dir in path_var.split(sep) {
                let candidate = PathBuf::from(dir).join(exe_name);
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }

        None
    }

    /// Converts an EPUB file to AZW3 using Calibre ebook-convert if available
    pub fn convert_epub_to_azw3(epub_path: &Path, temp_dir: &Path) -> Result<PathBuf, String> {
        let converter = Self::find_ebook_convert().ok_or_else(|| {
            "Calibre (ebook-convert) が見つかりませんでした。EPUBの転送にはCalibreのインストールが必要です。".to_string()
        })?;

        fs::create_dir_all(temp_dir).map_err(|e| e.to_string())?;

        let stem = epub_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("book");
        let output_path = temp_dir.join(format!("{}.azw3", stem));

        log::info!("Converting EPUB to AZW3: {} -> {}", epub_path.display(), output_path.display());

        let status = std::process::Command::new(converter)
            .arg(epub_path)
            .arg(&output_path)
            .status()
            .map_err(|e| format!("ebook-convert 実行エラー: {}", e))?;

        if !status.success() {
            return Err("ebook-convert によるEPUB変換に失敗しました。".to_string());
        }

        if !output_path.exists() {
            return Err("変換後のAZW3ファイルが生成されませんでした。".to_string());
        }

        Ok(output_path)
    }

    /// Push files to Kindle via standard filesystem mount (UMS: Paperwhite, Oasis, etc.)
    pub fn push_files_ums(
        files: &[PathBuf],
        mount_path: &Path,
        subfolder: &str,
        db: &Database,
    ) -> FileTransferResult {
        let mut target_dir = mount_path.join("documents");
        let sub = subfolder.trim();
        if !sub.is_empty() {
            // Sanitize subfolder name
            let clean_sub = sub.replace(['\\', ':', '*', '?', '"', '<', '>', '|'], "/");
            for part in clean_sub.split('/') {
                let p = part.trim();
                if !p.is_empty() && p != "." && p != ".." {
                    target_dir = target_dir.join(p);
                }
            }
        }

        if let Err(e) = fs::create_dir_all(&target_dir) {
            return FileTransferResult {
                success: false,
                transferred_count: 0,
                skipped_count: 0,
                failed_files: vec![("documents".to_string(), e.to_string())],
                message: format!("Kindleの documents ディレクトリを作成できませんでした: {}", e),
            };
        }

        let mut transferred = 0;
        let mut skipped = 0;
        let mut failed = Vec::new();

        for file_path in files {
            let filename = match file_path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => {
                    failed.push((file_path.display().to_string(), "無効なファイル名です".to_string()));
                    continue;
                }
            };

            // Calculate hash for deduplication
            let hash = match Self::compute_file_hash(file_path) {
                Ok(h) => h,
                Err(e) => {
                    failed.push((filename, e));
                    continue;
                }
            };

            // Check if already transferred
            if let Ok(true) = db.is_file_transferred(&hash) {
                log::info!("File already transferred, skipping: {}", filename);
                skipped += 1;
                continue;
            }

            let file_size = fs::metadata(file_path).map(|m| m.len()).unwrap_or(0);
            let dest_file = target_dir.join(&filename);

            log::info!("Copying {} to {}", file_path.display(), dest_file.display());
            match fs::copy(file_path, &dest_file) {
                Ok(_) => {
                    let _ = db.mark_file_transferred(&hash, &filename, file_size, &dest_file.to_string_lossy());
                    transferred += 1;
                }
                Err(e) => {
                    log::error!("Failed to copy {}: {}", filename, e);
                    failed.push((filename, format!("コピー失敗: {}", e)));
                }
            }
        }

        let success = failed.is_empty() && (transferred > 0 || skipped > 0);
        let message = if transferred > 0 {
            format!("{} 件のファイルをKindleへ転送しました（スキップ: {} 件）", transferred, skipped)
        } else if skipped > 0 {
            format!("転送対象ファイルはすべて転送済みです（スキップ: {} 件）", skipped)
        } else if !failed.is_empty() {
            format!("{} 件のファイル転送に失敗しました", failed.len())
        } else {
            "転送対象のファイルがありませんでした".to_string()
        };

        FileTransferResult {
            success,
            transferred_count: transferred,
            skipped_count: skipped,
            failed_files: failed,
            message,
        }
    }

    /// Push files to Kindle Scribe via Windows Shell COM MTP
    #[cfg(target_os = "windows")]
    pub fn push_files_windows_mtp(
        files: &[PathBuf],
        subfolder: &str,
        db: &Database,
    ) -> FileTransferResult {
        let mut transferred = 0;
        let mut skipped = 0;
        let mut failed = Vec::new();

        let clean_sub = subfolder.trim().replace('/', "\\");

        for file_path in files {
            let filename = match file_path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => {
                    failed.push((file_path.display().to_string(), "無効なファイル名です".to_string()));
                    continue;
                }
            };

            let hash = match Self::compute_file_hash(file_path) {
                Ok(h) => h,
                Err(e) => {
                    failed.push((filename, e));
                    continue;
                }
            };

            if let Ok(true) = db.is_file_transferred(&hash) {
                skipped += 1;
                continue;
            }

            let file_size = fs::metadata(file_path).map(|m| m.len()).unwrap_or(0);
            let src_str = file_path.to_string_lossy().to_string();

            // Run PowerShell to copy file to Kindle Scribe documents
            let ps_script = format!(
                r#"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
$srcFile = "{src_file}"
$subDir = "{sub_dir}"

$shell = New-Object -ComObject Shell.Application
$pc = $shell.Namespace(17)
if ($pc -eq $null) {{ exit 1 }}

$kindle = $null
foreach ($item in $pc.Items()) {{
    if ($item.Name -like "*Kindle*" -or $item.Name -like "*Scribe*" -or $item.Type -like "*Kindle*") {{
        $kindle = $item
        break
    }}
}}
if ($kindle -eq $null) {{ exit 2 }}

$storageRoot = $kindle
$kindleFolder = $kindle.GetFolder
if ($kindleFolder -ne $null) {{
    foreach ($item in $kindleFolder.Items()) {{
        if ($item.Name -like "*Internal*" -or $item.Name -like "*ストレージ*" -or $item.Name -like "*Storage*") {{
            $storageRoot = $item
            break
        }}
    }}
}}

$storageFolder = $storageRoot.GetFolder
if ($storageFolder -eq $null) {{ exit 3 }}

$docItem = $null
foreach ($item in $storageFolder.Items()) {{
    if ($item.Name -eq "documents" -or $item.Name -eq "Documents") {{
        $docItem = $item
        break
    }}
}}
if ($docItem -eq $null) {{ exit 4 }}

$destFolder = $docItem.GetFolder

# If subfolder requested, navigate into it or create it
if (-not [string]::IsNullOrWhiteSpace($subDir)) {{
    $subItem = $null
    foreach ($item in $destFolder.Items()) {{
        if ($item.Name -eq $subDir) {{
            $subItem = $item
            break
        }}
    }}
    if ($subItem -ne $null) {{
        $destFolder = $subItem.GetFolder
    }}
}}

$destFolder.CopyHere($srcFile, 1556)
Start-Sleep -Seconds 1
exit 0
"#,
                src_file = src_str.replace('"', "`\""),
                sub_dir = clean_sub.replace('"', "`\"")
            );

            let res = std::process::Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", &ps_script])
                .status();

            match res {
                Ok(status) if status.success() => {
                    let _ = db.mark_file_transferred(&hash, &filename, file_size, &format!("MTP:documents/{}", clean_sub));
                    transferred += 1;
                }
                Ok(status) => {
                    failed.push((filename, format!("Windows MTP コピー失敗 (終了コード: {:?})", status.code())));
                }
                Err(e) => {
                    failed.push((filename, format!("PowerShell 実行エラー: {}", e)));
                }
            }
        }

        let success = failed.is_empty() && (transferred > 0 || skipped > 0);
        let message = if transferred > 0 {
            format!("{} 件のファイルをKindleへ転送しました（スキップ: {} 件）", transferred, skipped)
        } else if skipped > 0 {
            format!("転送対象ファイルはすべて転送済みです（スキップ: {} 件）", skipped)
        } else {
            "ファイル転送に失敗しました".to_string()
        };

        FileTransferResult {
            success,
            transferred_count: transferred,
            skipped_count: skipped,
            failed_files: failed,
            message,
        }
    }

    /// Push files to Kindle Scribe via rusb MTP (macOS / Linux)
    #[cfg(not(target_os = "windows"))]
    pub fn push_files_usb_mtp(
        files: &[PathBuf],
        subfolder: &str,
        db: &Database,
    ) -> FileTransferResult {
        // Use MtpClient to send files directly over USB
        match crate::device::mtp::MtpClient::push_files(files, subfolder, db) {
            Ok(result) => result,
            Err(e) => FileTransferResult {
                success: false,
                transferred_count: 0,
                skipped_count: 0,
                failed_files: vec![("Kindle Scribe".to_string(), e.clone())],
                message: format!("MTP転送エラー: {}", e),
            },
        }
    }

    /// Scan hotfolder for pending files that are supported
    pub fn scan_hotfolder(
        hotfolder_path: &Path,
        db: &Database,
    ) -> Result<Vec<PathBuf>, String> {
        if !hotfolder_path.exists() {
            return Ok(Vec::new());
        }

        let mut pending = Vec::new();
        let entries = fs::read_dir(hotfolder_path)
            .map_err(|e| format!("ホットフォルダの読み込みに失敗しました: {}", e))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.starts_with('.') {
                    continue; // Skip hidden / system files
                }

                // Check supported or epub
                if Self::is_supported(&path) || Self::is_epub(&path) {
                    // Check if already transferred
                    if let Ok(hash) = Self::compute_file_hash(&path) {
                        if let Ok(false) = db.is_file_transferred(&hash) {
                            pending.push(path);
                        }
                    } else {
                        pending.push(path);
                    }
                }
            }
        }

        Ok(pending)
    }

    /// Execute post-transfer action (move to synced/ or keep)
    pub fn handle_post_transfer(
        file_path: &Path,
        hotfolder: &Path,
        action: &str,
    ) -> Result<(), String> {
        if action == "move_synced" {
            let synced_dir = hotfolder.join("synced");
            fs::create_dir_all(&synced_dir).map_err(|e| e.to_string())?;

            if let Some(name) = file_path.file_name() {
                let dest = synced_dir.join(name);
                // Try rename first, fallback to copy + remove
                if fs::rename(file_path, &dest).is_err() {
                    let _ = fs::copy(file_path, &dest);
                    let _ = fs::remove_file(file_path);
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supported_extensions() {
        assert!(FilePusher::is_supported(Path::new("book.pdf")));
        assert!(FilePusher::is_supported(Path::new("book.PDF")));
        assert!(FilePusher::is_supported(Path::new("book.kfx")));
        assert!(FilePusher::is_supported(Path::new("book.azw3")));
        assert!(FilePusher::is_supported(Path::new("book.mobi")));
        assert!(FilePusher::is_supported(Path::new("notes.txt")));

        assert!(!FilePusher::is_supported(Path::new("book.epub")));
        assert!(!FilePusher::is_supported(Path::new("program.exe")));
        assert!(!FilePusher::is_supported(Path::new("archive.zip")));
    }

    #[test]
    fn test_is_epub() {
        assert!(FilePusher::is_epub(Path::new("novel.epub")));
        assert!(FilePusher::is_epub(Path::new("novel.EPUB")));
        assert!(!FilePusher::is_epub(Path::new("novel.pdf")));
    }

    #[test]
    fn test_push_files_ums_and_deduplication() {
        let temp_dir = std::env::temp_dir().join(format!("test_push_ums_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let kindle_mount = temp_dir.join("KindleMock");
        let src_dir = temp_dir.join("Src");
        fs::create_dir_all(&src_dir).unwrap();

        let test_file = src_dir.join("test_doc.pdf");
        fs::write(&test_file, b"%PDF-1.4 test document content").unwrap();

        let db = Database::open_in_memory().unwrap();

        // 1. First push
        let res = FilePusher::push_files_ums(&[test_file.clone()], &kindle_mount, "Tech", &db);
        assert!(res.success);
        assert_eq!(res.transferred_count, 1);
        assert_eq!(res.skipped_count, 0);

        let copied_path = kindle_mount.join("documents").join("Tech").join("test_doc.pdf");
        assert!(copied_path.exists());
        assert_eq!(fs::read(&copied_path).unwrap(), b"%PDF-1.4 test document content");

        // 2. Second push (deduplication)
        let res2 = FilePusher::push_files_ums(&[test_file], &kindle_mount, "Tech", &db);
        assert!(res2.success);
        assert_eq!(res2.transferred_count, 0);
        assert_eq!(res2.skipped_count, 1);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_handle_post_transfer_move_synced() {
        let temp_dir = std::env::temp_dir().join(format!("test_post_action_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let hotfolder = temp_dir.join("KindleDrop");
        fs::create_dir_all(&hotfolder).unwrap();

        let file = hotfolder.join("paper.pdf");
        fs::write(&file, b"sample paper").unwrap();

        FilePusher::handle_post_transfer(&file, &hotfolder, "move_synced").unwrap();

        assert!(!file.exists());
        assert!(hotfolder.join("synced").join("paper.pdf").exists());

        let _ = fs::remove_dir_all(&temp_dir);
    }
}

