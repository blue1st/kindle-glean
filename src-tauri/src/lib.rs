pub mod db;
pub mod device;
pub mod generator;
pub mod models;
pub mod parsers;
pub mod sync;
pub mod tray;

use db::Database;
use device::DeviceDetector;
use models::{
    Clipping, DeviceInfo, DeviceProfile, NotebookSummary, SyncConfig, SyncStats, SyncedCounts,
    VocabLookup,
};
use std::sync::Arc;
use std::time::Duration;
use sync::SyncOrchestrator;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_notification::NotificationExt;

pub struct AppState {
    pub db: Arc<Database>,
    pub orchestrator: SyncOrchestrator,
    pub sync_lock: Arc<std::sync::Mutex<()>>,
}

#[tauri::command]
fn get_config(state: State<'_, AppState>) -> Result<SyncConfig, String> {
    state.db.get_config().map_err(|e| e.to_string())
}

#[tauri::command]
fn save_config(config: SyncConfig, state: State<'_, AppState>) -> Result<(), String> {
    state.db.save_config(&config).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_device_profile(
    device_id: String,
    state: State<'_, AppState>,
) -> Result<Option<DeviceProfile>, String> {
    state.db.get_device_profile(&device_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn save_device_profile(
    profile: DeviceProfile,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.db.save_device_profile(&profile).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_all_device_profiles(state: State<'_, AppState>) -> Result<Vec<DeviceProfile>, String> {
    state.db.get_all_device_profiles().map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_device_profile(
    device_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.db.delete_device_profile(&device_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn detect_device(state: State<'_, AppState>) -> Option<DeviceInfo> {
    let mut dev = DeviceDetector::detect_device()?;
    if let Ok(Some(profile)) = state.db.get_device_profile(&dev.device_id) {
        dev.is_registered = true;
        dev.nickname = Some(profile.nickname);
        let _ = state.db.update_device_last_connected(&dev.device_id);
    }
    Some(dev)
}

#[tauri::command]
fn inspect_folder(folder_path: String, state: State<'_, AppState>) -> Option<DeviceInfo> {
    let mut dev = DeviceDetector::inspect_kindle_directory(folder_path, "Custom Folder", "Folder")?;
    if let Ok(Some(profile)) = state.db.get_device_profile(&dev.device_id) {
        dev.is_registered = true;
        dev.nickname = Some(profile.nickname);
        let _ = state.db.update_device_last_connected(&dev.device_id);
    }
    Some(dev)
}

#[tauri::command]
fn get_sync_history(state: State<'_, AppState>) -> Result<Vec<SyncStats>, String> {
    state.db.get_recent_history(30).map_err(|e| e.to_string())
}

#[tauri::command]
fn sync_now(
    source_path: Option<String>,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<SyncStats, String> {
    let _guard = state
        .sync_lock
        .try_lock()
        .map_err(|_| "現在同期を実行中です。完了までお待ちください。".to_string())?;

    let (path, device_name, is_mtp, effective_config) = match source_path {
        Some(p) => {
            let config = state.db.get_config().map_err(|e| e.to_string())?;
            (p, "Custom Folder".to_string(), false, config)
        }
        None => {
            let mut device = DeviceDetector::detect_device()
                .ok_or_else(|| "Kindle端末が見つかりませんでした。USB接続を確認してください。".to_string())?;
            let is_mtp = device.connection_mode == "MTP";

            // If a custom profile exists for this device, use its specific destination folder and options
            let profile_opt = state.db.get_device_profile(&device.device_id).ok().flatten();
            let effective_config = if let Some(p) = profile_opt {
                device.nickname = Some(p.nickname.clone());
                let _ = state.db.update_device_last_connected(&device.device_id);
                SyncConfig {
                    vault_path: p.vault_path,
                    subfolder: p.subfolder,
                    sync_clippings: p.sync_clippings,
                    sync_vocab: p.sync_vocab,
                    sync_notebooks: p.sync_notebooks,
                    auto_sync: p.auto_sync,
                    auto_eject: p.auto_eject,
                }
            } else {
                state.db.get_config().map_err(|e| e.to_string())?
            };

            let name = device.nickname.unwrap_or(device.device_type);
            (device.mount_path, name, is_mtp, effective_config)
        }
    };

    let progress_cb = |p: crate::models::SyncProgress| {
        let _ = app_handle.emit("sync-progress", &p);
    };

    let actual_sync_path = if is_mtp {
        let cache_dir = std::env::temp_dir().join("kindle_scribe_cache");
        match device::MtpClient::pull_scribe_files(&cache_dir, Some(&progress_cb)) {
            Ok(_) => cache_dir.to_string_lossy().to_string(),
            Err(e) => {
                log::warn!("MTP pull failed in sync_now: {}. Attempting UMS fallback.", e);
                // Fallback: if mount path is a real filesystem path, try UMS sync
                if !path.starts_with("mtp://") && !path.starts_with("usb://") && std::path::Path::new(&path).exists() {
                    log::info!("Falling back to UMS mount path for sync: {}", path);
                    path.clone()
                } else {
                    return Err(format!(
                        "MTP同期エラー: {}. Kindle端末の画面ロック解除とUSB接続を確認してください。",
                        e
                    ));
                }
            }
        }
    } else {
        progress_cb(crate::models::SyncProgress {
            step: "checking".to_string(),
            message: "同期対象フォルダを確認中...".to_string(),
            percentage: 20,
            current_item: None,
        });
        path.clone()
    };

    let result = state.orchestrator.sync_from_path(&actual_sync_path, &effective_config, &device_name, Some(&progress_cb))?;

    // Emit event to frontend
    let _ = app_handle.emit("sync-completed", &result);

    // Send native desktop notification
    let _ = app_handle.notification()
        .builder()
        .title("Kindle Glean")
        .body(&result.message)
        .show();

    // Auto-eject if enabled and UMS
    if effective_config.auto_eject && !is_mtp && device_name != "Custom Folder" {
        eject_device(path).ok();
    }

    Ok(result)
}

#[tauri::command]
fn preview_clippings(file_path: String) -> Result<Vec<Clipping>, String> {
    parsers::ClippingsParser::parse_file(file_path)
}

#[tauri::command]
fn eject_device(mount_path: String) -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        let status = std::process::Command::new("diskutil")
            .arg("unmount")
            .arg(&mount_path)
            .status()
            .map_err(|e| e.to_string())?;
        return Ok(status.success());
    }

    #[cfg(target_os = "windows")]
    {
        // PowerShell script or mountvol could be invoked if needed
        return Ok(true);
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let status = std::process::Command::new("umount")
            .arg(&mount_path)
            .status()
            .map_err(|e| e.to_string())?;
        return Ok(status.success());
    }
}

#[tauri::command]
fn open_folder(path: Option<String>, state: State<'_, AppState>) -> Result<(), String> {
    let target_path = match path {
        Some(p) => crate::models::resolve_path(&p),
        None => {
            let config = state.db.get_config().map_err(|e| e.to_string())?;
            let base_vault = crate::models::resolve_path(&config.vault_path);
            let sub = config.subfolder.trim();
            if sub.is_empty() {
                base_vault
            } else {
                base_vault.join(sub)
            }
        }
    };

    if !target_path.exists() {
        std::fs::create_dir_all(&target_path).map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&target_path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&target_path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        std::process::Command::new("xdg-open")
            .arg(&target_path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
fn reexport_all(state: State<'_, AppState>, app_handle: AppHandle) -> Result<SyncStats, String> {
    let config = state.db.get_config().map_err(|e| e.to_string())?;
    let progress_cb = |p: crate::models::SyncProgress| {
        let _ = app_handle.emit("sync-progress", &p);
    };
    let stats = state.orchestrator.reexport_all(&config, Some(&progress_cb))?;

    let _ = app_handle.emit("sync-completed", &stats);
    let _ = app_handle.notification()
        .builder()
        .title("Kindle Glean")
        .body(&stats.message)
        .show();

    Ok(stats)
}

#[tauri::command]
fn get_autostart_status(app: AppHandle) -> Result<bool, String> {
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
fn set_autostart(enable: bool, app: AppHandle) -> Result<(), String> {
    if enable {
        app.autolaunch().enable().map_err(|e| e.to_string())
    } else {
        app.autolaunch().disable().map_err(|e| e.to_string())
    }
}

#[tauri::command]
fn get_synced_counts(state: State<'_, AppState>) -> Result<SyncedCounts, String> {
    state.db.get_synced_counts().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_synced_clippings(state: State<'_, AppState>) -> Result<Vec<Clipping>, String> {
    state.db.get_all_synced_clippings().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_synced_vocab(state: State<'_, AppState>) -> Result<Vec<VocabLookup>, String> {
    state.db.get_all_synced_vocab().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_synced_notebooks(state: State<'_, AppState>) -> Result<Vec<NotebookSummary>, String> {
    let mut list = state.db.get_all_synced_notebooks().map_err(|e| e.to_string())?;
    let config = state.db.get_config().map_err(|e| e.to_string())?;

    let base_vault = crate::models::resolve_path(&config.vault_path);
    let sub = config.subfolder.trim();
    let base_dir = if sub.is_empty() {
        base_vault
    } else {
        base_vault.join(sub)
    };

    let nb_root = base_dir.join("Notebooks");

    for nb in &mut list {
        let safe_title = crate::generator::MarkdownGenerator::sanitize_filename(&nb.title);
        let rel_path = base_dir.join(&nb.relative_folder);
        let dir = if rel_path.is_dir() {
            rel_path
        } else if nb_root.join(&safe_title).is_dir() {
            nb_root.join(&safe_title)
        } else {
            nb_root.join(&nb.title)
        };

        let candidates = [
            dir.join("thumbnail.png"),
            dir.join("page_1.png"),
            dir.join("page_1.jpg"),
            dir.join("page_1.svg"),
            dir.join("page_2.svg"),
        ];

        for cand in &candidates {
            if cand.is_file() {
                // If cand is page_1.svg, check if it is nearly empty (< 500 bytes). If so and page_2.svg exists, prefer page_2.svg for cover preview
                if cand.ends_with("page_1.svg") {
                    if let Ok(meta) = std::fs::metadata(cand) {
                        if meta.len() < 500 && dir.join("page_2.svg").is_file() {
                            nb.cover_image_path = Some(dir.join("page_2.svg").to_string_lossy().to_string());
                            break;
                        }
                    }
                }
                nb.cover_image_path = Some(cand.to_string_lossy().to_string());
                break;
            }
        }
    }

    Ok(list)
}

#[tauri::command]
fn get_notebook_pages(notebook_title: String, state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let config = state.db.get_config().map_err(|e| e.to_string())?;
    let base_vault = crate::models::resolve_path(&config.vault_path);
    let sub = config.subfolder.trim();
    let base_dir = if sub.is_empty() {
        base_vault
    } else {
        base_vault.join(sub)
    };

    let nb_root = base_dir.join("Notebooks");
    let safe_title = crate::generator::MarkdownGenerator::sanitize_filename(&notebook_title);

    let found_dir = if nb_root.join(&safe_title).is_dir() {
        Some(nb_root.join(&safe_title))
    } else if nb_root.join(&notebook_title).is_dir() {
        Some(nb_root.join(&notebook_title))
    } else {
        walkdir::WalkDir::new(&nb_root)
            .max_depth(4)
            .into_iter()
            .flatten()
            .find(|e| {
                if e.file_type().is_dir() {
                    let name = e.file_name().to_string_lossy();
                    name == safe_title || name == notebook_title
                } else {
                    false
                }
            })
            .map(|e| e.path().to_path_buf())
    };

    let nb_dir = match found_dir {
        Some(d) => d,
        None => return Ok(Vec::new()),
    };

    let mut pages = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&nb_dir) {
        let mut files: Vec<std::path::PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                let name = p
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                name.starts_with("page_")
                    && (name.ends_with(".png") || name.ends_with(".jpg") || name.ends_with(".svg"))
            })
            .collect();

        // Sort numerically by page number (page_1, page_2, ..., page_10) instead of lexicographically
        files.sort_by_key(|p| {
            p.file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| s.strip_prefix("page_"))
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(0)
        });

        for f in files {
            pages.push(f.to_string_lossy().to_string());
        }
    }

    Ok(pages)
}

#[tauri::command]
fn read_image_base64(file_path: String) -> Result<String, String> {
    let p = std::path::Path::new(&file_path);
    if !p.exists() || !p.is_file() {
        return Err(format!("ファイルが見つかりません: {}", file_path));
    }

    let ext = p
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let bytes = std::fs::read(p).map_err(|e| e.to_string())?;

    let mime = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    };

    let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
    Ok(format!("data:{};base64,{}", mime, b64))
}

#[tauri::command]
fn export_notebook_pages(
    source_paths: Vec<String>,
    target_dir: String,
    folder_name: Option<String>,
) -> Result<usize, String> {
    let base_target = std::path::Path::new(&target_dir);
    if !base_target.exists() {
        return Err("保存先ディレクトリが存在しません".to_string());
    }

    let dest_dir = if let Some(sub) = folder_name {
        let safe_sub = crate::generator::MarkdownGenerator::sanitize_filename(&sub);
        let d = base_target.join(safe_sub);
        std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
        d
    } else {
        base_target.to_path_buf()
    };

    let mut copied_count = 0;
    for src in source_paths {
        let src_path = std::path::Path::new(&src);
        if src_path.is_file() {
            if let Some(file_name) = src_path.file_name() {
                let target_file = dest_dir.join(file_name);
                if std::fs::copy(src_path, target_file).is_ok() {
                    copied_count += 1;
                }
            }
        }
    }

    Ok(copied_count)
}

#[tauri::command]
fn save_binary_file(file_path: String, data: Vec<u8>) -> Result<(), String> {
    let p = std::path::Path::new(&file_path);
    if let Some(parent) = p.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    std::fs::write(p, data).map_err(|e| e.to_string())?;
    Ok(())
}

// Background thread that monitors for device connections
fn start_device_watcher(app_handle: AppHandle, db: Arc<Database>, sync_lock: Arc<std::sync::Mutex<()>>) {
    std::thread::spawn(move || {
        let mut last_connected = false;

        loop {
            std::thread::sleep(Duration::from_secs(3));

            let device_opt = DeviceDetector::detect_device();
            let is_connected = device_opt.is_some();

            if is_connected && !last_connected {
                if let Some(mut device) = device_opt {
                    let profile_opt = db.get_device_profile(&device.device_id).ok().flatten();
                    if let Some(ref p) = profile_opt {
                        device.is_registered = true;
                        device.nickname = Some(p.nickname.clone());
                        let _ = db.update_device_last_connected(&device.device_id);
                    }

                    log::info!("Kindle device connected: {:?}", device);
                    let _ = app_handle.emit("device-connected", &device);

                    // Auto sync if configured
                    let should_auto_sync = match &profile_opt {
                        Some(p) => p.auto_sync,
                        None => db.get_config().map(|c| c.auto_sync).unwrap_or(false),
                    };

                    let sync_config = match &profile_opt {
                        Some(p) => SyncConfig {
                            vault_path: p.vault_path.clone(),
                            subfolder: p.subfolder.clone(),
                            sync_clippings: p.sync_clippings,
                            sync_vocab: p.sync_vocab,
                            sync_notebooks: p.sync_notebooks,
                            auto_sync: p.auto_sync,
                            auto_eject: p.auto_eject,
                        },
                        None => db.get_config().unwrap_or_default(),
                    };

                    if should_auto_sync {
                        if let Ok(_guard) = sync_lock.try_lock() {
                            let orchestrator = SyncOrchestrator::new(db.clone());
                            let app_h = app_handle.clone();
                            let on_progress = |p: models::SyncProgress| {
                                let _ = app_h.emit("sync-progress", &p);
                            };

                            let sync_path = if device.connection_mode == "MTP" {
                                let cache_dir = std::env::temp_dir().join("kindle_scribe_cache");
                                match device::MtpClient::pull_scribe_files(&cache_dir, Some(&on_progress)) {
                                    Ok(_) => Some(cache_dir.to_string_lossy().to_string()),
                                    Err(e) => {
                                        log::warn!("MTP pull failed: {}. Checking if UMS mount path is available as fallback.", e);
                                        // Fallback: if mount_path is a real filesystem path (not mtp:// or usb://),
                                        // try syncing from it directly (the device may actually be UMS-mounted)
                                        let mp = &device.mount_path;
                                        if !mp.starts_with("mtp://") && !mp.starts_with("usb://") && std::path::Path::new(mp).exists() {
                                            log::info!("Falling back to UMS mount path: {}", mp);
                                            Some(mp.clone())
                                        } else {
                                            None
                                        }
                                    }
                                }
                            } else {
                                Some(device.mount_path.clone())
                            };

                            let dev_display = device.nickname.as_deref().unwrap_or(&device.device_type);
                            if let Some(path) = sync_path {
                                if let Ok(stats) = orchestrator.sync_from_path(&path, &sync_config, dev_display, Some(&on_progress)) {
                                    let _ = app_handle.emit("sync-completed", &stats);
                                    let _ = app_handle.notification()
                                        .builder()
                                        .title("Kindle Glean")
                                        .body(&stats.message)
                                        .show();
                                }
                            }
                        }
                    }
                }
            } else if !is_connected && last_connected {
                log::info!("Kindle device disconnected");
                let _ = app_handle.emit("device-disconnected", ());
            }

            last_connected = is_connected;
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let db = match Database::init() {
        Ok(db) => Arc::new(db),
        Err(e) => {
            eprintln!("Failed to initialize database: {}", e);
            std::process::exit(1);
        }
    };

    let orchestrator = SyncOrchestrator::new(db.clone());
    let sync_lock = Arc::new(std::sync::Mutex::new(()));
    let state = AppState {
        db: db.clone(),
        orchestrator,
        sync_lock: sync_lock.clone(),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .manage(state)
        .setup(move |app| {
            // Setup system tray
            if let Err(e) = tray::setup_tray(&app.handle()) {
                log::warn!("Failed to setup tray: {}", e);
            }

            // Start background device monitoring
            start_device_watcher(app.handle().clone(), db.clone(), sync_lock.clone());

            // If started with --minimized, hide main window
            let args: Vec<String> = std::env::args().collect();
            if args.iter().any(|arg| arg == "--minimized") {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_config,
            get_device_profile,
            save_device_profile,
            get_all_device_profiles,
            delete_device_profile,
            detect_device,
            inspect_folder,
            get_sync_history,
            sync_now,
            preview_clippings,
            eject_device,
            open_folder,
            get_synced_counts,
            get_synced_clippings,
            get_synced_vocab,
            get_synced_notebooks,
            get_notebook_pages,
            read_image_base64,
            export_notebook_pages,
            save_binary_file,
            reexport_all,
            get_autostart_status,
            set_autostart,
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app_handle, event| match event {
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Reopen {
                has_visible_windows,
                ..
            } => {
                if !has_visible_windows {
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
            _ => {}
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_notebook_pages() {
        let temp_dir = std::env::temp_dir().join(format!("kindle_test_export_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let src_file1 = temp_dir.join("src_page_1.svg");
        let src_file2 = temp_dir.join("src_page_2.svg");
        std::fs::write(&src_file1, "<svg>page 1</svg>").unwrap();
        std::fs::write(&src_file2, "<svg>page 2</svg>").unwrap();

        let target_dir = temp_dir.join("target");
        std::fs::create_dir_all(&target_dir).unwrap();

        let count = export_notebook_pages(
            vec![src_file1.to_string_lossy().to_string(), src_file2.to_string_lossy().to_string()],
            target_dir.to_string_lossy().to_string(),
            Some("My Sample Notebook".to_string()),
        ).unwrap();

        assert_eq!(count, 2);
        assert!(target_dir.join("My Sample Notebook").join("src_page_1.svg").exists());
        assert!(target_dir.join("My Sample Notebook").join("src_page_2.svg").exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_save_binary_file() {
        let temp_dir = std::env::temp_dir().join(format!("kindle_test_save_bin_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let dest_file = temp_dir.join("subfolder/document.pdf");

        let test_bytes = b"%PDF-1.4 sample content".to_vec();
        save_binary_file(dest_file.to_string_lossy().to_string(), test_bytes.clone()).unwrap();

        assert!(dest_file.is_file());
        let read_bytes = std::fs::read(&dest_file).unwrap();
        assert_eq!(read_bytes, test_bytes);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
