use crate::models::DeviceInfo;
use std::path::{Path, PathBuf};
use sysinfo::Disks;

pub struct DeviceDetector;

impl DeviceDetector {
    /// Detects connected Kindle devices either via USB Mass Storage (UMS) mounts
    /// or direct USB / MTP bus enumeration (for Kindle Scribe, etc.).
    pub fn detect_device() -> Option<DeviceInfo> {
        // 0. On Windows, directly scan drive letters (D:\ to Z:\) first for maximum speed & reliability
        #[cfg(target_os = "windows")]
        {
            if let Some(win_info) = Self::scan_windows_drives() {
                return Some(win_info);
            }
        }

        // 1. First, scan all mounted disks via sysinfo (Windows drive letters, macOS /Volumes, Linux /media)
        let disks = Disks::new_with_refreshed_list();
        for disk in &disks {
            let mount_point = disk.mount_point();
            let disk_name = disk.name().to_string_lossy();

            log::debug!(
                "Scanning disk: name='{}', mount='{}', fs='{}'",
                disk_name,
                mount_point.display(),
                disk.file_system().to_string_lossy()
            );

            // On Windows, mount_point.file_name() returns None for root paths
            // like "E:\" — use the full path string instead for name matching.
            let mount_str = mount_point.to_string_lossy().to_lowercase();
            let is_kindle_named = disk_name.to_lowercase().contains("kindle")
                || mount_str.contains("kindle");

            if is_kindle_named || Self::has_kindle_signatures(mount_point) {
                log::info!(
                    "Kindle candidate found via disk scan: name='{}', mount='{}'",
                    disk_name,
                    mount_point.display()
                );
                if let Some(info) = Self::inspect_kindle_directory(
                    mount_point,
                    &format!("Kindle ({})", disk_name),
                    "UMS",
                ) {
                    return Some(info);
                }
            }
        }

        // 2. Check known macOS Kindle mount path (/Volumes/Kindle)
        #[cfg(target_os = "macos")]
        {
            let default_macos_kindle = PathBuf::from("/Volumes/Kindle");
            if default_macos_kindle.exists() {
                if let Some(info) = Self::inspect_kindle_directory(
                    &default_macos_kindle,
                    "Kindle Paperwhite / Oasis (UMS)",
                    "UMS",
                ) {
                    return Some(info);
                }
            }
        }

        // 3. If no mounted disk was found, check USB bus directly (for MTP devices like Kindle Scribe, 11th/12th Gen)
        if let Some(usb_info) = Self::detect_usb_raw() {
            return Some(usb_info);
        }

        // 4. Check macOS ioreg as fallback for USB enumeration
        #[cfg(target_os = "macos")]
        {
            if let Some(ioreg_info) = Self::detect_macos_ioreg() {
                return Some(ioreg_info);
            }
        }

        None
    }

    /// Scan USB devices via rusb (libusb)
    pub fn detect_usb_raw() -> Option<DeviceInfo> {
        if let Ok(devices) = rusb::devices() {
            for device in devices.iter() {
                if let Ok(desc) = device.device_descriptor() {
                    let vid = desc.vendor_id();
                    let pid = desc.product_id();

                    // 0x1949 = Amazon.com, Inc.
                    let is_amazon = vid == 0x1949;
                    if is_amazon {
                        let is_scribe = pid == 0x9981;

                        // On Windows, if this is not a Scribe, it's a standard Kindle (Paperwhite, Oasis, Basic).
                        // Standard Kindles mount as USB Mass Storage (UMS) drive letters (e.g. "D:\", "E:\").
                        // Scan Windows drive letters directly to see if it is mounted.
                        #[cfg(target_os = "windows")]
                        if !is_scribe {
                            if let Some(drive_info) = Self::scan_windows_drives() {
                                log::info!(
                                    "Amazon USB device (PID: 0x{:04x}) mapped to Windows UMS drive: {}",
                                    pid, drive_info.mount_path
                                );
                                return Some(drive_info);
                            }
                        }

                        // Kindle Scribe is always MTP.
                        // Standard Kindle Paperwhite / Oasis are NEVER MTP devices.
                        let is_mtp = is_scribe;

                        let dev_title = if is_scribe {
                            "Kindle Scribe (MTP)".to_string()
                        } else if is_mtp {
                            format!("Kindle (MTP PID: 0x{:04x})", pid)
                        } else {
                            format!("Kindle Device (USB PID: 0x{:04x})", pid)
                        };

                        // Extract USB Serial Number from device descriptor
                        let serial_str = if let Ok(handle) = device.open() {
                            let timeout = std::time::Duration::from_millis(200);
                            handle
                                .read_languages(timeout)
                                .ok()
                                .and_then(|langs| langs.first().copied())
                                .and_then(|lang| handle.read_serial_number_string(lang, &desc, timeout).ok())
                                .filter(|s| !s.trim().is_empty())
                        } else {
                            None
                        };

                        let device_id = serial_str.unwrap_or_else(|| {
                            format!("usb_kindle_{:04x}_{:04x}", vid, pid)
                        });

                        log::info!(
                            "USB raw detection result: type='{}', mode='{}', id='{}'",
                            dev_title,
                            if is_mtp { "MTP" } else { "UMS" },
                            device_id
                        );

                        return Some(DeviceInfo {
                            device_id,
                            device_type: dev_title,
                            connection_mode: if is_mtp { "MTP".to_string() } else { "UMS".to_string() },
                            mount_path: if is_mtp {
                                format!("mtp://0x{:04x}:0x{:04x}", vid, pid)
                            } else {
                                format!("usb://0x{:04x}:0x{:04x}", vid, pid)
                            },
                            has_clippings: true,
                            has_vocab: true,
                            has_notebooks: is_scribe,
                            connected: true,
                            status_message: if is_mtp {
                                Some("MTP接続中 (画面のロックを解除してファイルアクセスを許可してください)".to_string())
                            } else {
                                Some("USB接続中 (画面ロックを解除してドライブを認識させるか、「フォルダ選択同期」からKindleドライブを選択してください)".to_string())
                            },
                            is_registered: false,
                            nickname: None,
                        });
                    }
                }
            }
        }
        None
    }

    #[cfg(target_os = "macos")]
    fn detect_macos_ioreg() -> Option<DeviceInfo> {
        let output = std::process::Command::new("ioreg")
            .args(["-p", "IOUSB", "-w0", "-l"])
            .output()
            .ok()?;

        let text = String::from_utf8_lossy(&output.stdout);
        if text.contains("Kindle Scribe") || (text.contains("1949") && text.contains("Amazon")) {
            let mut serial = "kindle_scribe_mtp".to_string();
            for line in text.lines() {
                if (line.contains("USB Serial Number") || line.contains("kUSBSerialNumber")) && line.contains('=') {
                    if let Some((_, val)) = line.split_once('=') {
                        let clean = val.trim().trim_matches('"').trim();
                        if !clean.is_empty() {
                            serial = clean.to_string();
                            break;
                        }
                    }
                }
            }

            return Some(DeviceInfo {
                device_id: serial,
                device_type: "Kindle Scribe (MTP)".to_string(),
                connection_mode: "MTP".to_string(),
                mount_path: "usb://kindle-scribe".to_string(),
                has_clippings: true,
                has_vocab: true,
                has_notebooks: true,
                connected: true,
                status_message: Some("MTP接続中 (画面ロック解除でファイル同期可能)".to_string()),
                is_registered: false,
                nickname: None,
            });
        }
        None
    }

    /// Directly scan Windows drive letters (D:\ to Z:\, then C:\) for mounted Kindle devices.
    pub fn scan_windows_drives() -> Option<DeviceInfo> {
        #[cfg(target_os = "windows")]
        {
            // First check common removable letters (D to Z), then C as fallback
            let drive_letters: Vec<char> = ('D'..='Z').chain(std::iter::once('C')).collect();
            for dl in drive_letters {
                let drive_root = format!("{}:\\", dl);
                let p = Path::new(&drive_root);
                if p.exists() && Self::has_kindle_signatures(p) {
                    log::info!("Kindle UMS drive found via direct Windows drive scan: {}", drive_root);
                    if let Some(info) = Self::inspect_kindle_directory(
                        p,
                        &format!("Kindle ({}:)", dl),
                        "UMS",
                    ) {
                        return Some(info);
                    }
                }
            }
        }
        None
    }

    /// Check if directory contains Kindle signatures (documents/My Clippings.txt or system/vocabulary/vocab.db)
    pub fn has_kindle_signatures<P: AsRef<Path>>(path: P) -> bool {
        let p = path.as_ref();
        p.join("documents").join("My Clippings.txt").exists()
            || p.join("My Clippings.txt").exists()
            || p.join("system").join("vocabulary").join("vocab.db").exists()
            || p.join(".notebooks").exists()
    }

    pub fn inspect_kindle_directory<P: AsRef<Path>>(
        path: P,
        device_type: &str,
        connection_mode: &str,
    ) -> Option<DeviceInfo> {
        let p = path.as_ref();
        if !p.exists() {
            return None;
        }

        let has_clippings = p.join("documents").join("My Clippings.txt").exists()
            || p.join("My Clippings.txt").exists();

        let has_vocab = p.join("system").join("vocabulary").join("vocab.db").exists()
            || p.join("vocab.db").exists();

        let has_notebooks = p.join(".notebooks").exists() || p.join("notebooks").exists();

        let mut actual_type = device_type.to_string();
        if has_notebooks && !actual_type.contains("Scribe") {
            actual_type = "Kindle Scribe".to_string();
        }

        let device_id = Self::get_or_create_dir_id(p);

        Some(DeviceInfo {
            device_id,
            device_type: actual_type,
            connection_mode: connection_mode.to_string(),
            mount_path: p.to_string_lossy().to_string(),
            has_clippings,
            has_vocab,
            has_notebooks,
            connected: true,
            status_message: None,
            is_registered: false,
            nickname: None,
        })
    }

    fn get_or_create_dir_id<P: AsRef<Path>>(path: P) -> String {
        let p = path.as_ref();
        let id_file = p.join(".kindle_sync_id");
        if id_file.is_file() {
            if let Ok(content) = std::fs::read_to_string(&id_file) {
                let trimmed = content.trim();
                if !trimmed.is_empty() {
                    return trimmed.to_string();
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            if let Some(uuid) = Self::get_volume_uuid(p) {
                return format!("vol_{}", uuid);
            }
        }

        // Fallback: SHA-256 hash of canonical path
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(p.to_string_lossy().as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        let new_id = format!("kindle_{}", &hash[..16]);

        // Attempt writing .kindle_sync_id for persistence across mounts if writable
        let _ = std::fs::write(&id_file, &new_id);
        new_id
    }

    #[cfg(target_os = "macos")]
    fn get_volume_uuid<P: AsRef<Path>>(path: P) -> Option<String> {
        let output = std::process::Command::new("diskutil")
            .args(["info", &path.as_ref().to_string_lossy()])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            if line.contains("Volume UUID:") || line.contains("Disk / Partition UUID:") {
                if let Some((_, val)) = line.split_once(':') {
                    let trimmed = val.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_has_kindle_signatures() {
        let temp_dir = std::env::temp_dir().join("test_kindle_sig_detection");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        assert!(!DeviceDetector::has_kindle_signatures(&temp_dir));

        let docs_dir = temp_dir.join("documents");
        fs::create_dir_all(&docs_dir).unwrap();

        // A plain "documents" directory without Kindle signature files must NOT be detected as a Kindle!
        assert!(!DeviceDetector::has_kindle_signatures(&temp_dir));

        let clippings_file = docs_dir.join("My Clippings.txt");
        fs::write(&clippings_file, "Sample clipping").unwrap();

        assert!(DeviceDetector::has_kindle_signatures(&temp_dir));

        let info = DeviceDetector::inspect_kindle_directory(&temp_dir, "Kindle Paperwhite", "UMS").unwrap();
        assert_eq!(info.connection_mode, "UMS");
        assert!(info.has_clippings);
        assert!(!info.has_vocab);
        assert!(!info.has_notebooks);

        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_inspect_scribe_notebooks_detection() {
        let temp_dir = std::env::temp_dir().join("test_scribe_sig_detection");
        let nb_dir = temp_dir.join(".notebooks");
        fs::create_dir_all(&nb_dir).unwrap();

        let info = DeviceDetector::inspect_kindle_directory(&temp_dir, "Kindle Device", "UMS").unwrap();
        assert_eq!(info.device_type, "Kindle Scribe");
        assert!(info.has_notebooks);

        fs::remove_dir_all(&temp_dir).ok();
    }
}
