use crate::models::{Notebook, NotebookPage};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

pub struct NotebookParser;

#[derive(serde::Deserialize)]
struct ScriptPage {
    page_num: usize,
    filename: String,
}

#[derive(serde::Deserialize)]
struct ScriptOutput {
    success: bool,
    #[serde(default)]
    pages: Vec<ScriptPage>,
}

fn find_converter_binary() -> Option<std::path::PathBuf> {
    let binary_name = if cfg!(windows) { "kfx-converter.exe" } else { "kfx-converter" };

    // 1. Compile-time manifest dir (guaranteed in tests and cargo dev)
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("binaries")
        .join(binary_name);
    if manifest.is_file() {
        return Some(manifest);
    }

    // 2. Working directory candidates
    let cwd_candidates = [
        format!("src-tauri/binaries/{}", binary_name),
        format!("binaries/{}", binary_name),
        format!("../src-tauri/binaries/{}", binary_name),
    ];
    for c in &cwd_candidates {
        let p = std::path::PathBuf::from(c);
        if p.is_file() {
            return Some(p);
        }
    }

    // 3. Executable / App bundle directory
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let exe_candidates = [
                exe_dir.join(binary_name),
                exe_dir.join("binaries").join(binary_name),
                exe_dir.join("../Resources/binaries").join(binary_name),
                exe_dir.join("../Resources").join(binary_name),
                exe_dir.join("../../Resources/binaries").join(binary_name),
            ];
            for p in &exe_candidates {
                if p.is_file() {
                    return Some(p.clone());
                }
            }
        }
    }

    None
}

fn find_python_command() -> Option<String> {
    #[allow(unused_mut)]
    let mut candidates = vec![
        "python3".to_string(),
        "python".to_string(),
        "py".to_string(),
    ];

    #[cfg(windows)]
    {
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let base = std::path::PathBuf::from(local_app_data).join("Programs").join("Python");
            if let Ok(entries) = std::fs::read_dir(&base) {
                for e in entries.flatten() {
                    let py_exe = e.path().join("python.exe");
                    if py_exe.is_file() {
                        candidates.push(py_exe.to_string_lossy().to_string());
                    }
                }
            }
        }
        for root in [
            "C:\\Python312\\python.exe",
            "C:\\Python311\\python.exe",
            "C:\\Python310\\python.exe",
            "C:\\Python39\\python.exe",
            "C:\\Program Files\\Python312\\python.exe",
            "C:\\Program Files\\Python311\\python.exe",
            "C:\\Program Files\\Python310\\python.exe",
        ] {
            if std::path::Path::new(root).is_file() {
                candidates.push(root.to_string());
            }
        }
    }

    for cmd in candidates {
        let mut check = std::process::Command::new(&cmd);
        check.arg("--version");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            check.creation_flags(0x08000000);
        }
        if let Ok(output) = check.output() {
            if output.status.success() {
                return Some(cmd);
            }
        }
    }
    None
}

fn find_convert_script() -> Option<std::path::PathBuf> {
    // 1. Compile-time manifest dir (guaranteed in tests and cargo dev)
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("scripts/kfx_parser/convert_notebook.py");
    if manifest.is_file() {
        return Some(manifest);
    }

    // 2. Working directory candidates
    let cwd_candidates = [
        "src-tauri/scripts/kfx_parser/convert_notebook.py",
        "scripts/kfx_parser/convert_notebook.py",
        "../src-tauri/scripts/kfx_parser/convert_notebook.py",
    ];
    for c in &cwd_candidates {
        let p = std::path::PathBuf::from(c);
        if p.is_file() {
            return Some(p);
        }
    }

    // 3. Executable / App bundle directory (Windows and macOS)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let exe_candidates = [
                exe_dir.join("scripts/kfx_parser/convert_notebook.py"),
                exe_dir.join("resources/scripts/kfx_parser/convert_notebook.py"),
                exe_dir.join("../resources/scripts/kfx_parser/convert_notebook.py"),
                exe_dir.join("../Resources/scripts/kfx_parser/convert_notebook.py"),
                exe_dir.join("../Resources/src-tauri/scripts/kfx_parser/convert_notebook.py"),
                exe_dir.join("../../Resources/scripts/kfx_parser/convert_notebook.py"),
                exe_dir.join("_up_/scripts/kfx_parser/convert_notebook.py"),
            ];
            for p in &exe_candidates {
                if p.is_file() {
                    return Some(p.clone());
                }
            }
        }
    }

    None
}

impl NotebookParser {
    pub fn parse_file<P: AsRef<Path>>(path: P) -> Result<Notebook, String> {
        Self::parse_file_with_folder(path, None)
    }

    pub fn parse_file_with_folder<P: AsRef<Path>>(
        path: P,
        relative_folder: Option<String>,
    ) -> Result<Notebook, String> {
        let path = path.as_ref();
        let fname_lower = path
            .file_name()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let raw_stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Untitled Notebook".to_string());

        let title = if fname_lower == "nbk" || raw_stem.to_lowercase() == "nbk" {
            path.parent()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or(raw_stem)
        } else {
            raw_stem
        };

        let metadata = std::fs::metadata(path)
            .map_err(|e| format!("Failed to read metadata for {}: {}", path.display(), e))?;
        let last_modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        // Read entire file to calculate content SHA-256 hash for reliable deduplication
        let mut file_bytes = Vec::new();
        let mut f = File::open(path).map_err(|e| format!("Cannot open {}: {}", path.display(), e))?;
        f.read_to_end(&mut file_bytes)
            .map_err(|e| format!("Cannot read {}: {}", path.display(), e))?;

        let mut hasher = Sha256::new();
        hasher.update(&file_bytes);
        let content_hash = format!("{:x}", hasher.finalize());

        let id = format!("nbk:{}", title);
        let mut pages = Vec::new();

        // 1. Primary extractor for Kindle Scribe .nbk files: convert vector pen strokes to clean SVG
        let is_nbk = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase() == "nbk")
            .unwrap_or(false)
            || fname_lower == "nbk";

        if is_nbk {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            let temp_dir_name = format!("kindle_nbk_{}_{}", title, timestamp);
            let temp_dir = std::env::temp_dir().join(temp_dir_name);
            let _ = std::fs::create_dir_all(&temp_dir);

            // Priority 1: Standalone sidecar binary (user needs NO Python installed)
            // Priority 2: Script fallback using python3 or python (for dev & test)
            let result = if let Some(bin_path) = find_converter_binary() {
                let mut cmd = std::process::Command::new(&bin_path);
                cmd.arg(path).arg(&temp_dir);
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
                }
                cmd.output()
            } else if let Some(script_path) = find_convert_script() {
                if let Some(py_cmd) = find_python_command() {
                    let mut cmd = std::process::Command::new(py_cmd);
                    cmd.arg(&script_path).arg(path).arg(&temp_dir);
                    #[cfg(windows)]
                    {
                        use std::os::windows::process::CommandExt;
                        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
                    }
                    cmd.output()
                } else {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "Neither standalone kfx-converter nor Python (python3/python) was found",
                    ))
                }
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "No converter binary or script found",
                ))
            };

            if let Ok(output) = result {
                if output.status.success() {
                    if let Ok(script_res) = serde_json::from_slice::<ScriptOutput>(&output.stdout) {
                        if script_res.success && !script_res.pages.is_empty() {
                            for sp in script_res.pages {
                                let svg_path = temp_dir.join(&sp.filename);
                                if let Ok(svg_bytes) = std::fs::read(&svg_path) {
                                    pages.push(NotebookPage {
                                        page_num: sp.page_num,
                                        image_data: svg_bytes,
                                        image_filename: sp.filename,
                                    });
                                }
                            }
                        }
                    }
                } else {
                    log::warn!(
                        "kfx converter failed for {}: {}",
                        path.display(),
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
            }

            let _ = std::fs::remove_dir_all(&temp_dir);
        }

        // 2. Secondary fallback: Try reading as a ZIP archive
        if pages.is_empty() {
            if let Ok(file) = File::open(path) {
                if let Ok(mut archive) = ZipArchive::new(file) {
                    let mut page_indices: Vec<usize> = Vec::new();

                    for i in 0..archive.len() {
                        if let Ok(f) = archive.by_index(i) {
                            let name = f.name().to_string();
                            if name.ends_with(".svg") || name.ends_with(".png") || name.ends_with(".json") || name.contains("page") {
                                page_indices.push(i);
                            }
                        }
                    }

                    page_indices.sort();

                    let mut page_num = 1;
                    for idx in page_indices {
                        if let Ok(mut zf) = archive.by_index(idx) {
                            let name = zf.name().to_string();
                            let mut buf = Vec::new();
                            zf.read_to_end(&mut buf).ok();

                            if name.ends_with(".svg") {
                                pages.push(NotebookPage {
                                    page_num,
                                    image_data: buf,
                                    image_filename: format!("page_{}.svg", page_num),
                                });
                                page_num += 1;
                            } else if name.ends_with(".png") || name.ends_with(".jpg") {
                                let ext = if name.ends_with(".jpg") { "jpg" } else { "png" };
                                pages.push(NotebookPage {
                                    page_num,
                                    image_data: buf,
                                    image_filename: format!("page_{}.{}", page_num, ext),
                                });
                                page_num += 1;
                            }
                        }
                    }
                }
            }
        }

        // If no pages were extracted from ZIP, check for thumbnail images generated by Kindle Scribe
        if pages.is_empty() {
            let mut found_images = false;
            let mut search_dirs = Vec::new();
            if let Some(parent) = path.parent() {
                search_dirs.push(parent.join("thumbnails"));
                if let Some(grandparent) = parent.parent() {
                    search_dirs.push(grandparent.join("thumbnails"));
                }
            }

            let clean_title = title.replace('-', "");
            for thumb_dir in &search_dirs {
                if thumb_dir.is_dir() {
                    if let Ok(entries) = std::fs::read_dir(thumb_dir) {
                        let mut matching: Vec<std::path::PathBuf> = entries
                            .flatten()
                            .map(|e| e.path())
                            .filter(|p| {
                                let fname = p
                                    .file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_default();
                                let is_img = fname.ends_with(".png") || fname.ends_with(".jpg");
                                let matches_id = fname.starts_with(&title)
                                    || (!clean_title.is_empty() && fname.starts_with(&clean_title));
                                is_img && matches_id
                            })
                            .collect();
                        matching.sort();

                        let mut page_num = 1;
                        for img_path in matching {
                            if let Ok(buf) = std::fs::read(&img_path) {
                                let ext = img_path
                                    .extension()
                                    .map(|e| e.to_string_lossy().to_string())
                                    .unwrap_or_else(|| "png".to_string());
                                pages.push(NotebookPage {
                                    page_num,
                                    image_data: buf,
                                    image_filename: format!("page_{}.{}", page_num, ext),
                                });
                                page_num += 1;
                                found_images = true;
                            }
                        }
                    }
                }
                if found_images {
                    break;
                }
            }

            // If not found in thumbnails directory, check direct candidate filenames
            if !found_images {
                if let Some(parent) = path.parent() {
                    let candidates = [
                        parent.join("thumbnails").join(format!("{}.png", title)),
                        parent.join("thumbnails").join(format!("{}.jpg", title)),
                        parent.join(format!("{}.png", title)),
                        parent.join(format!("{}.jpg", title)),
                    ];
                    for cand in candidates {
                        if cand.is_file() {
                            if let Ok(buf) = std::fs::read(&cand) {
                                let ext = cand
                                    .extension()
                                    .map(|e| e.to_string_lossy().to_string())
                                    .unwrap_or_else(|| "png".to_string());
                                pages.push(NotebookPage {
                                    page_num: 1,
                                    image_data: buf,
                                    image_filename: format!("page_1.{}", ext),
                                });
                                found_images = true;
                                break;
                            }
                        }
                    }
                }
            }

            if !found_images {
                let fallback_svg = format!(
                    r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1404 1872" width="1404" height="1872">
  <rect width="100%" height="100%" fill="#f8fafc" stroke="#e2e8f0" stroke-width="2"/>
  <rect x="40" y="40" width="1324" height="1792" fill="#ffffff" rx="8" stroke="#cbd5e1" stroke-width="1"/>
  <text x="100" y="200" font-family="-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif" font-size="36" font-weight="bold" fill="#1e293b">Kindle Scribe ノート</text>
  <text x="100" y="260" font-family="-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif" font-size="20" fill="#64748b">ID: {}</text>
  <line x1="100" y1="300" x2="1304" y2="300" stroke="#e2e8f0" stroke-width="2"/>
  <text x="100" y="360" font-family="-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif" font-size="22" fill="#334155">※ このノートは手書きストロークデータ（KDFバイナリ）として同期されました。</text>
  <text x="100" y="410" font-family="-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif" font-size="18" fill="#64748b">Kindle Scribe 端末側で一度ノートを開いてページを更新するか、</text>
  <text x="100" y="450" font-family="-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif" font-size="18" fill="#64748b">Kindleの「共有・メール送信」機能をご利用いただくことで、完全なPDFとして取り込むことも可能です。</text>
</svg>"##,
                    title
                );

                pages.push(NotebookPage {
                    page_num: 1,
                    image_data: fallback_svg.into_bytes(),
                    image_filename: "page_1.svg".to_string(),
                });
            }
        }

        Ok(Notebook {
            id,
            title,
            content_hash,
            last_modified,
            pages,
            relative_folder,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_convert_script() {
        let script = find_convert_script();
        assert!(script.is_some(), "Should find convert_notebook.py");
        let script_path = script.unwrap();
        assert!(script_path.is_file(), "Script path must be an existing file");
    }

    #[test]
    fn test_find_python_command() {
        // In this dev environment, python3 or python should be found
        let py = find_python_command();
        assert!(py.is_some(), "Should find python3 or python executable in dev environment");
    }

    #[test]
    fn test_parse_scribe_nbk_if_exists() {
        let nbk_path = std::path::Path::new("/var/folders/jg/2h1hm3cn57l94zntzvzdmj7m0000gn/T/kindle_scribe_cache/.notebooks/8eca449d-1d90-55e9-480c-28800b9cc8f7.nbk");
        if nbk_path.is_file() {
            if let Ok(nb) = NotebookParser::parse_file(nbk_path) {
                assert_eq!(nb.pages.len(), 5, "5-page notebook should produce exactly 5 pages");
                assert_eq!(nb.pages[0].image_filename, "page_1.svg");
                assert_eq!(nb.pages[4].image_filename, "page_5.svg");
                assert!(!nb.pages[1].image_data.is_empty(), "Page 2 should contain SVG stroke data");
            }
        }
    }

    #[test]
    fn test_parse_extensionless_nbk_in_uuid_folder() {
        let temp_dir = std::env::temp_dir().join("test_scribe_nbk_uuid");
        let uuid_dir = temp_dir.join("87e9358b-7c2b-34dc-b250-93aace5dd640");
        let _ = std::fs::create_dir_all(&uuid_dir);
        let nbk_file = uuid_dir.join("nbk");
        std::fs::write(&nbk_file, b"dummy content for testing").unwrap();

        let nb = NotebookParser::parse_file(&nbk_file).expect("Must parse dummy extensionless nbk");
        assert_eq!(nb.title, "87e9358b-7c2b-34dc-b250-93aace5dd640");
        assert_eq!(nb.id, "nbk:87e9358b-7c2b-34dc-b250-93aace5dd640");
        assert_eq!(nb.pages.len(), 1, "Should generate fallback page for dummy binary");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

