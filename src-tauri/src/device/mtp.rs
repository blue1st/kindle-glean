use rusb::{Context, DeviceHandle, UsbContext};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct MtpClient;

impl MtpClient {
    pub const SCRIBE_VID: u16 = 0x1949;
    pub const SCRIBE_PID: u16 = 0x9981;

    /// Pulls Kindle files from Scribe via MTP into a local cache directory
    pub fn pull_scribe_files<P: AsRef<Path>>(
        cache_dir: P,
        on_progress: Option<&dyn Fn(crate::models::SyncProgress)>,
    ) -> Result<PathBuf, String> {
        let cache_path = cache_dir.as_ref().to_path_buf();
        fs::create_dir_all(&cache_path).map_err(|e| e.to_string())?;

        #[cfg(target_os = "windows")]
        {
            return Self::pull_scribe_files_windows(&cache_path, on_progress);
        }

        #[cfg(not(target_os = "windows"))]
        {
            Self::pull_scribe_files_usb(&cache_path, on_progress)
        }
    }

    /// Windows native MTP extraction using Windows Shell COM (Shell.Application)
    /// This avoids USB interface locking conflicts with the Windows WPD driver.
    pub fn pull_scribe_files_windows(
        cache_path: &Path,
        on_progress: Option<&dyn Fn(crate::models::SyncProgress)>,
    ) -> Result<PathBuf, String> {
        if let Some(cb) = on_progress {
            cb(crate::models::SyncProgress {
                step: "connecting".to_string(),
                message: "Kindle に接続中 (Windows MTP)...".to_string(),
                percentage: 10,
                current_item: None,
            });
        }

        let ps_script = r#"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

try {
    $dest = $env:KINDLE_SYNC_DEST
    if ([string]::IsNullOrWhiteSpace($dest) -and $args.Count -gt 0) {
        $dest = $args[0]
    }
    if ([string]::IsNullOrWhiteSpace($dest)) {
        Write-Output "ERROR:同期先ディレクトリパス（KINDLE_SYNC_DEST）が指定されていません。"
        exit 1
    }
    if (-not (Test-Path $dest)) {
        New-Item -ItemType Directory -Path $dest -Force | Out-Null
    }

    Write-Output "PROGRESS:connecting:Kindle端末を検索中 (Windows Shell)...:15:"

    $shell = New-Object -ComObject Shell.Application
    $pc = $shell.Namespace(17)
    if ($pc -eq $null) {
        Write-Output "ERROR:Windows Shell (This PC) を開けませんでした。"
        exit 1
    }

    $kindle = $null

    try {
        $pnp = Get-PnpDevice -PresentOnly -ErrorAction SilentlyContinue | Where-Object { $_.InstanceId -like "*VID_1949*" } | Select-Object -First 1
        if ($pnp -and $pnp.FriendlyName) {
            foreach ($item in $pc.Items()) {
                if ($item.Name -eq $pnp.FriendlyName -or $item.Name -like "*$($pnp.FriendlyName)*" -or $pnp.FriendlyName -like "*$($item.Name)*") {
                    $kindle = $item
                    break
                }
            }
        }
    } catch {}

    if ($kindle -eq $null) {
        foreach ($item in $pc.Items()) {
            if ($item.Name -like "*Kindle*" -or $item.Name -like "*Scribe*" -or $item.Type -like "*Kindle*") {
                $kindle = $item
                break
            }
        }
    }

    if ($kindle -eq $null) {
        Write-Output "ERROR:Kindle端末が見つかりませんでした。エクスプローラーの「PC」配下にKindleが表示されているか、画面ロックが解除されているか確認してください。"
        exit 1
    }

    Write-Output "PROGRESS:scanning:Kindleストレージをスキャン中...:25:"

    $kindleFolder = $null
    try {
        $kindleFolder = $kindle.GetFolder
    } catch {}

    if ($kindleFolder -eq $null) {
        Write-Output "ERROR:Kindleのストレージを開けませんでした。Kindle Scribeの画面ロック（パスコード）を解除してください。"
        exit 1
    }

    $copiedCount = 0

    function Copy-ShellItem($srcItem, $targetLocalDir, $expectedName) {
        if (-not (Test-Path $targetLocalDir)) {
            New-Item -ItemType Directory -Path $targetLocalDir -Force | Out-Null
        }
        $resolvedDir = [System.IO.Path]::GetFullPath($targetLocalDir)
        $destFolder = $shell.Namespace($resolvedDir)
        if ($destFolder -eq $null) {
            Write-Output "DEBUG: Failed to get shell namespace for '$resolvedDir'"
            return $false
        }

        try {
            $destFolder.CopyHere($srcItem, 16)
        } catch {
            try {
                $destFolder.CopyHere($srcItem)
            } catch {
                Write-Output "DEBUG: CopyHere failed: $_"
                return $false
            }
        }

        $destFilePath = [System.IO.Path]::Combine($resolvedDir, $expectedName)
        $timeout = [DateTime]::Now.AddSeconds(20)
        while (-not (Test-Path $destFilePath) -and [DateTime]::Now -lt $timeout) {
            Start-Sleep -Milliseconds 200
        }
        if (Test-Path $destFilePath) {
            $prevSize = -1
            while ([DateTime]::Now -lt $timeout) {
                try {
                    $curSize = (Get-Item $destFilePath).Length
                    if ($curSize -eq $prevSize -and $curSize -ge 0) {
                        break
                    }
                    $prevSize = $curSize
                } catch {}
                Start-Sleep -Milliseconds 200
            }
            Write-Output "DEBUG: Copied $expectedName ($curSize bytes)"
            return $true
        }
        Write-Output "DEBUG: Timeout waiting for $expectedName"
        return $false
    }

    function Find-Folder-ByName($parentFolder, $targetName, $maxDepth = 3) {
        if ($parentFolder -eq $null -or $maxDepth -le 0) { return $null }
        try {
            foreach ($item in $parentFolder.Items()) {
                if ($item.IsFolder) {
                    if ($item.Name.ToLower() -eq $targetName.ToLower()) {
                        $f = $item.GetFolder
                        if ($f -ne $null) { return $f }
                    }
                    $subF = $item.GetFolder
                    if ($subF -ne $null) {
                        $found = Find-Folder-ByName $subF $targetName ($maxDepth - 1)
                        if ($found -ne $null) { return $found }
                    }
                }
            }
        } catch {}
        return $null
    }

    # 1. My Clippings.txt
    Write-Output "PROGRESS:clippings:My Clippings.txt を取得中...:40:My Clippings.txt"
    $docsDest = [System.IO.Path]::Combine($dest, "documents")
    $clippingsCopied = $false

    $docFolder = Find-Folder-ByName $kindleFolder "documents" 3
    if ($docFolder -ne $null) {
        Write-Output "DEBUG: Found documents folder in Kindle storage"
        try {
            foreach ($f in $docFolder.Items()) {
                if ($f.Name.ToLower() -eq "my clippings.txt") {
                    if (Copy-ShellItem $f $docsDest "My Clippings.txt") {
                        $clippingsCopied = $true
                        $copiedCount++
                    }
                    break
                }
            }
        } catch {}
    }

    # Fallback: check root items for My Clippings.txt
    if (-not $clippingsCopied) {
        try {
            foreach ($f in $kindleFolder.Items()) {
                if ($f.Name.ToLower() -eq "my clippings.txt") {
                    if (Copy-ShellItem $f $docsDest "My Clippings.txt") {
                        $clippingsCopied = $true
                        $copiedCount++
                    }
                    break
                }
            }
        } catch {}
    }

    # 2. vocab.db
    Write-Output "PROGRESS:vocab:vocab.db を取得中...:60:vocab.db"
    $vocabDest = [System.IO.Path]::Combine([System.IO.Path]::Combine($dest, "system"), "vocabulary")

    $sysFolder = Find-Folder-ByName $kindleFolder "system" 3
    if ($sysFolder -ne $null) {
        $vFolder = Find-Folder-ByName $sysFolder "vocabulary" 2
        $searchFolder = if ($vFolder -ne $null) { $vFolder } else { $sysFolder }
        try {
            foreach ($f in $searchFolder.Items()) {
                if ($f.Name.ToLower() -eq "vocab.db") {
                    if (Copy-ShellItem $f $vocabDest "vocab.db") {
                        $copiedCount++
                    }
                    break
                }
            }
        } catch {}
    }

    # 3. Notebooks (.notebooks or notebooks)
    Write-Output "PROGRESS:notebooks:Scribe手書きノートを取得中...:80:notebooks"
    $nbFolder = Find-Folder-ByName $kindleFolder ".notebooks" 3
    if ($nbFolder -eq $null) {
        $nbFolder = Find-Folder-ByName $kindleFolder "notebooks" 3
    }

    if ($nbFolder -ne $null) {
        Write-Output "DEBUG: Found notebooks folder in Kindle storage"
        $nbDest = [System.IO.Path]::Combine($dest, ".notebooks")
        if (-not (Test-Path $nbDest)) {
            New-Item -ItemType Directory -Path $nbDest -Force | Out-Null
        }
        function Copy-NotebookFolder($srcShellFolder, $destLocalPath) {
            if (-not (Test-Path $destLocalPath)) {
                New-Item -ItemType Directory -Path $destLocalPath -Force | Out-Null
            }
            try {
                foreach ($item in $srcShellFolder.Items()) {
                    if ($item.IsFolder) {
                        $subF = $item.GetFolder
                        if ($subF -ne $null) {
                            Copy-NotebookFolder $subF ([System.IO.Path]::Combine($destLocalPath, $item.Name))
                        }
                    } else {
                        $iname = $item.Name.ToLower()
                        if ($iname.EndsWith(".nbk") -or $iname.EndsWith(".png") -or $iname.EndsWith(".jpg")) {
                            if (Copy-ShellItem $item $destLocalPath $item.Name) {
                                $global:copiedCount++
                            }
                        }
                    }
                }
            } catch {}
        }
        Copy-NotebookFolder $nbFolder $nbDest
    }

    if ($copiedCount -eq 0) {
        Write-Output "ERROR:Kindle端末からデータを取得できませんでした。Kindle Scribeの画面ロック（パスコード）を解除し、画面を点灯させた状態でUSB接続を確認してください。"
        exit 1
    }

    Write-Output "PROGRESS:transfer_complete:端末からのデータ取得が完了しました:90:"
    Write-Output "SUCCESS:OK"

} catch {
    $errMsg = $_.Exception.Message
    Write-Output "ERROR:$errMsg"
    exit 1
}
"#;

        let mut cmd = std::process::Command::new("powershell");
        cmd.args([
            "-Sta",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            ps_script,
        ]);
        cmd.env("KINDLE_SYNC_DEST", cache_path);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = cmd.spawn().map_err(|e| {
            format!("PowerShell の起動に失敗しました: {}", e)
        })?;

        let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
        let reader = std::io::BufReader::new(stdout);
        use std::io::BufRead;

        let mut last_error = String::new();
        for line_res in reader.lines() {
            if let Ok(line) = line_res {
                let line_str = line.trim();
                if line_str.starts_with("PROGRESS:") {
                    let parts: Vec<&str> = line_str.splitn(5, ':').collect();
                    if parts.len() >= 4 {
                        let step = parts[1].to_string();
                        let msg = parts[2].to_string();
                        let pct = parts[3].parse::<u32>().unwrap_or(50);
                        let item = if parts.len() >= 5 && !parts[4].is_empty() {
                            Some(parts[4].to_string())
                        } else {
                            None
                        };
                        if let Some(cb) = on_progress {
                            cb(crate::models::SyncProgress {
                                step,
                                message: msg,
                                percentage: pct,
                                current_item: item,
                            });
                        }
                    }
                } else if line_str.starts_with("DEBUG:") {
                    log::debug!("{}", line_str);
                } else if line_str.starts_with("ERROR:") {
                    last_error = line_str.trim_start_matches("ERROR:").trim().to_string();
                }
            }
        }

        let status = child.wait().map_err(|e| format!("PowerShell 終了待ちエラー: {}", e))?;
        if !status.success() {
            let mut stderr_msg = String::new();
            if let Some(mut stderr) = child.stderr.take() {
                use std::io::Read;
                let _ = stderr.read_to_string(&mut stderr_msg);
            }

            // Prioritize last_error from stdout, which has clean message
            let err_detail = if !last_error.is_empty() {
                last_error
            } else {
                let mut fallback_msg = String::new();
                for line in stderr_msg.lines() {
                    let trimmed = line.trim();
                    if (trimmed.contains("Kindle") || trimmed.contains("エラー"))
                        && !trimmed.starts_with('+')
                        && !trimmed.starts_with('~')
                        && !trimmed.contains("CategoryInfo")
                        && !trimmed.contains("FullyQualifiedErrorId")
                        && !trimmed.contains("ErrorActionPreference")
                    {
                        fallback_msg = trimmed.to_string();
                        break;
                    }
                }
                if !fallback_msg.is_empty() {
                    fallback_msg
                } else {
                    "Kindle端末からデータを取得できませんでした。Kindle端末の画面ロックを解除してUSB接続を確認してください。".to_string()
                }
            };

            return Err(err_detail);
        }

        Ok(cache_path.to_path_buf())
    }

    /// Pulls Kindle files from Scribe via direct USB / rusb MTP (macOS / Linux)
    pub fn pull_scribe_files_usb(
        cache_path: &Path,
        on_progress: Option<&dyn Fn(crate::models::SyncProgress)>,
    ) -> Result<PathBuf, String> {
        if let Some(cb) = on_progress {
            cb(crate::models::SyncProgress {
                step: "connecting".to_string(),
                message: "Kindle に接続中 (MTPセッション確立)...".to_string(),
                percentage: 10,
                current_item: None,
            });
        }

        let context = Context::new().map_err(|e| format!("rusb context error: {}", e))?;
        let devices = context.devices().map_err(|e| format!("rusb devices error: {}", e))?;

        let mut scribe_dev = None;
        for dev in devices.iter() {
            if let Ok(desc) = dev.device_descriptor() {
                if desc.vendor_id() == Self::SCRIBE_VID && desc.product_id() == Self::SCRIBE_PID {
                    scribe_dev = Some(dev);
                    break;
                }
            }
        }

        let dev = scribe_dev.ok_or_else(|| "Kindle がUSB接続されていません".to_string())?;
        let mut handle = dev.open().map_err(|e| {
            format!(
                "Kindle を開けませんでした: {}. ロックを解除して再試行してください。",
                e
            )
        })?;

        if handle.kernel_driver_active(0).unwrap_or(false) {
            handle.detach_kernel_driver(0).ok();
        }

        let mut claimed = false;
        let mut last_err = String::new();
        for _ in 0..6 {
            match handle.claim_interface(0) {
                Ok(_) => {
                    claimed = true;
                    break;
                }
                Err(e) => {
                    last_err = e.to_string();
                    std::thread::sleep(Duration::from_millis(250));
                }
            }
        }

        if !claimed {
            return Err(format!(
                "USBインターフェースの専有に失敗しました: {}. 端末の画面ロックを解除して再試行してください。",
                last_err
            ));
        }

        let result = Self::do_pull(&mut handle, cache_path, on_progress);
        handle.release_interface(0).ok();

        result.map(|_| cache_path.to_path_buf())
    }

    fn do_pull(
        handle: &mut DeviceHandle<Context>,
        out_dir: &Path,
        on_progress: Option<&dyn Fn(crate::models::SyncProgress)>,
    ) -> Result<(), String> {
        // 1. Reset PTP Device using Class Specific Control Request (bRequest 0x66)
        let _ = handle.write_control(0x21, 0x66, 0, 0, &[], Duration::from_millis(500));
        std::thread::sleep(Duration::from_millis(200));

        // Clear endpoint halts
        let _ = handle.clear_halt(0x01);
        let _ = handle.clear_halt(0x81);

        // 2. OpenSession (Opcode 0x1002), TransactionID MUST BE 0
        let _ = Self::transact(handle, 0x1002, 0, &[1]);

        // 3. GetStorageIDs (Opcode 0x1004)
        let mut tx_id = 1;
        let storages_res = Self::transact(handle, 0x1004, tx_id, &[])?;
        tx_id += 1;

        let mut storage_ids = Vec::new();
        if storages_res.data.len() >= 4 {
            let count = u32::from_le_bytes(storages_res.data[0..4].try_into().unwrap()) as usize;
            for i in 0..count {
                let idx = 4 + i * 4;
                if idx + 4 <= storages_res.data.len() {
                    storage_ids.push(u32::from_le_bytes(storages_res.data[idx..idx + 4].try_into().unwrap()));
                }
            }
        }

        let sid = storage_ids.first().copied().unwrap_or(0x00010001);

        // 4. List all object handles
        let handles_res = Self::transact(handle, 0x1007, tx_id, &[sid, 0, 0])?;
        tx_id += 1;

        if handles_res.data.len() < 4 {
            let _ = Self::transact(handle, 0x1003, tx_id, &[]);
            return Err("MTPオブジェクトを取得できませんでした".to_string());
        }

        let count = u32::from_le_bytes(handles_res.data[0..4].try_into().unwrap()) as usize;
        let mut handles = Vec::new();
        for i in 0..count {
            let idx = 4 + i * 4;
            if idx + 4 <= handles_res.data.len() {
                handles.push(u32::from_le_bytes(handles_res.data[idx..idx + 4].try_into().unwrap()));
            }
        }

        log::info!("MTP: Total objects on Kindle Scribe = {}", handles.len());

        // Prepare destination directories
        let docs_dir = out_dir.join("documents");
        let vocab_dir = out_dir.join("system").join("vocabulary");
        let nb_dir = out_dir.join(".notebooks");
        fs::create_dir_all(&docs_dir).ok();
        fs::create_dir_all(&vocab_dir).ok();
        fs::create_dir_all(&nb_dir).ok();

        if let Some(cb) = on_progress {
            cb(crate::models::SyncProgress {
                step: "scanning".to_string(),
                message: "端末内のファイル構造をスキャン中...".to_string(),
                percentage: 20,
                current_item: None,
            });
        }

        let mut object_map: std::collections::HashMap<u32, MtpFileInfo> = std::collections::HashMap::new();
        for h in &handles {
            if let Ok(info) = Self::get_object_info(handle, *h, &mut tx_id) {
                object_map.insert(*h, info);
            }
        }

        // Diagnostic log: write all non-image or folder objects
        let mut debug_lines = Vec::new();
        for (h, info) in &object_map {
            if info.is_folder || !info.filename.ends_with(".jpg") {
                debug_lines.push(format!(
                    "0x{:08x} parent=0x{:08x} dir={:<5} sz={:<8} name='{}'",
                    h, info.parent_handle, info.is_folder, info.size, info.filename
                ));
            }
        }
        debug_lines.sort();
        let _ = fs::write("/tmp/kindle_mtp_objects.txt", debug_lines.join("\n"));
        log::info!("MTP: Written {} objects to /tmp/kindle_mtp_objects.txt", debug_lines.len());

        let get_ancestry = |start_h: u32, map: &std::collections::HashMap<u32, MtpFileInfo>| -> Vec<MtpFileInfo> {
            let mut chain = Vec::new();
            let mut curr = start_h;
            let mut visited = std::collections::HashSet::new();
            while curr != 0 && visited.insert(curr) {
                if let Some(info) = map.get(&curr) {
                    chain.push(info.clone());
                    curr = info.parent_handle;
                } else {
                    break;
                }
            }
            chain.reverse();
            chain
        };

        let mut target_files: Vec<(u32, PathBuf)> = Vec::new();

        for (h, info) in &object_map {
            let lname = info.filename.to_lowercase();

            if lname == "my clippings.txt" {
                target_files.push((*h, docs_dir.join("My Clippings.txt")));
            } else if lname == "vocab.db" {
                target_files.push((*h, vocab_dir.join("vocab.db")));
            } else if lname.ends_with(".png") || lname.ends_with(".jpg") {
                let ancestry = get_ancestry(info.parent_handle, &object_map);
                let in_notebooks = ancestry.iter().any(|a| {
                    let lower = a.filename.to_lowercase();
                    lower == ".notebooks" || lower == "notebooks"
                });
                let in_thumbnails = ancestry.iter().any(|a| {
                    a.filename.to_lowercase() == "thumbnails"
                });
                if in_notebooks && in_thumbnails {
                    let thumb_dir = nb_dir.join("thumbnails");
                    fs::create_dir_all(&thumb_dir).ok();
                    target_files.push((*h, thumb_dir.join(&info.filename)));
                }
            } else if lname == "nbk" || lname.ends_with(".nbk") {
                let ancestry = get_ancestry(info.parent_handle, &object_map);

                let in_notebooks = ancestry.iter().any(|a| {
                    let lower = a.filename.to_lowercase();
                    lower == ".notebooks" || lower == "notebooks"
                });

                let in_system_or_templates = ancestry.iter().any(|a| {
                    let lower = a.filename.to_lowercase();
                    lower.contains("template") || lower == "system" || lower == ".trash"
                });

                let parent_name = ancestry.last().map(|a| a.filename.as_str()).unwrap_or("");
                let is_ebook_annotation = parent_name.contains("!!EBOK!!") || parent_name.starts_with("B0");

                if in_notebooks && !in_system_or_templates && !is_ebook_annotation {
                    // Reconstruct folder hierarchy between .notebooks and notebook UUID folder
                    let mut folder_segments = Vec::new();
                    let mut passed_notebooks = false;
                    for a in &ancestry {
                        let lower = a.filename.to_lowercase();
                        if lower == ".notebooks" || lower == "notebooks" {
                            passed_notebooks = true;
                            continue;
                        }
                        if passed_notebooks {
                            folder_segments.push(a.filename.clone());
                        }
                    }

                    let nb_folder_name = folder_segments.pop().unwrap_or_else(|| format!("notebook_{:x}", h));
                    let safe_name = nb_folder_name
                        .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");

                    let dest_folder = if folder_segments.is_empty() {
                        nb_dir.clone()
                    } else {
                        let mut p = nb_dir.clone();
                        for seg in folder_segments {
                            let safe_seg = seg.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
                            p = p.join(safe_seg);
                        }
                        p
                    };
                    fs::create_dir_all(&dest_folder).ok();
                    target_files.push((*h, dest_folder.join(format!("{}.nbk", safe_name))));
                }
            }
        }

        let total_files = target_files.len();
        log::info!("MTP: Downloading {} matched files...", total_files);

        for (idx, (obj_handle, dest_path)) in target_files.iter().enumerate() {
            let filename = dest_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let pct = 25 + ((idx as u32 * 40) / total_files.max(1) as u32);
            if let Some(cb) = on_progress {
                cb(crate::models::SyncProgress {
                    step: "downloading".to_string(),
                    message: format!("ファイルを転送中 ({}/{}): {}", idx + 1, total_files, filename),
                    percentage: pct,
                    current_item: Some(filename),
                });
            }
            if let Err(e) = Self::download_file(handle, *obj_handle, dest_path, &mut tx_id) {
                log::warn!("Failed to download 0x{:08x}: {}", obj_handle, e);
            }
        }

        if let Some(cb) = on_progress {
            cb(crate::models::SyncProgress {
                step: "transfer_complete".to_string(),
                message: "端末からのファイル転送が完了しました".to_string(),
                percentage: 65,
                current_item: None,
            });
        }

        // CloseSession
        let _ = Self::transact(handle, 0x1003, tx_id, &[]);
        Ok(())
    }

    fn download_file(
        handle: &mut DeviceHandle<Context>,
        obj_handle: u32,
        dest_path: &Path,
        tx_id: &mut u32,
    ) -> Result<(), String> {
        if let Some(p) = dest_path.parent() {
            fs::create_dir_all(p).ok();
        }

        let res = Self::transact(handle, 0x1009, *tx_id, &[obj_handle])?;
        *tx_id += 1;

        if res.code == 0x2001 || !res.data.is_empty() {
            fs::write(dest_path, &res.data).map_err(|e| e.to_string())?;
            log::info!(
                "Downloaded MTP file: {} ({} bytes)",
                dest_path.display(),
                res.data.len()
            );
            Ok(())
        } else {
            Err(format!(
                "GetObject 0x{:08x} failed with code 0x{:04x}",
                obj_handle, res.code
            ))
        }
    }

    fn get_object_info(
        handle: &mut DeviceHandle<Context>,
        obj_handle: u32,
        tx_id: &mut u32,
    ) -> Result<MtpFileInfo, String> {
        let res = Self::transact(handle, 0x1008, *tx_id, &[obj_handle])?;
        *tx_id += 1;

        if res.data.len() < 52 {
            return Err("ObjectInfo dataset too short".to_string());
        }

        let format_code = u16::from_le_bytes(res.data[4..6].try_into().unwrap());
        let size = u32::from_le_bytes(res.data[8..12].try_into().unwrap());
        let parent = u32::from_le_bytes(res.data[38..42].try_into().unwrap());

        let mut offset = 52;
        let filename = Self::parse_string(&res.data, &mut offset);
        let is_folder = format_code == 0x3001;

        Ok(MtpFileInfo {
            handle: obj_handle,
            filename,
            size,
            parent_handle: parent,
            is_folder,
        })
    }

    fn parse_string(data: &[u8], offset: &mut usize) -> String {
        if *offset >= data.len() {
            return String::new();
        }
        let num_chars = data[*offset] as usize;
        *offset += 1;
        if num_chars == 0 {
            return String::new();
        }
        let mut u16_chars = Vec::new();
        for _ in 0..num_chars.saturating_sub(1) {
            if *offset + 2 <= data.len() {
                let ch = u16::from_le_bytes(data[*offset..*offset + 2].try_into().unwrap());
                u16_chars.push(ch);
                *offset += 2;
            }
        }
        *offset += 2;
        String::from_utf16_lossy(&u16_chars)
    }

    fn transact(
        handle: &mut DeviceHandle<Context>,
        code: u16,
        tx_id: u32,
        params: &[u32],
    ) -> Result<MtpResponse, String> {
        let ep_out = 0x01;
        let ep_in = 0x81;

        let length = (12 + params.len() * 4) as u32;
        let mut cmd = Vec::with_capacity(length as usize);
        cmd.extend_from_slice(&length.to_le_bytes());
        cmd.extend_from_slice(&1u16.to_le_bytes()); // 1 = Command
        cmd.extend_from_slice(&code.to_le_bytes());
        cmd.extend_from_slice(&tx_id.to_le_bytes());
        for p in params {
            cmd.extend_from_slice(&p.to_le_bytes());
        }

        handle
            .write_bulk(ep_out, &cmd, Duration::from_secs(3))
            .map_err(|e| format!("write_bulk: {}", e))?;

        let mut data_payload = Vec::new();
        let resp_code;

        loop {
            let mut buf = vec![0u8; 16384];
            let n = handle
                .read_bulk(ep_in, &mut buf, Duration::from_secs(4))
                .map_err(|e| format!("read_bulk: {}", e))?;
            buf.truncate(n);

            if n < 12 {
                continue;
            }

            let container_len = u32::from_le_bytes(buf[0..4].try_into().unwrap()) as usize;
            let container_type = u16::from_le_bytes(buf[4..6].try_into().unwrap());
            let op_or_code = u16::from_le_bytes(buf[6..8].try_into().unwrap());

            if container_type == 2 {
                data_payload.extend_from_slice(&buf[12..n.min(container_len)]);
                let mut rem = container_len.saturating_sub(n);
                while rem > 0 {
                    let mut chunk = vec![0u8; rem.min(65536)];
                    let m = handle
                        .read_bulk(ep_in, &mut chunk, Duration::from_secs(5))
                        .map_err(|e| format!("data read chunk: {}", e))?;
                    data_payload.extend_from_slice(&chunk[..m]);
                    rem = rem.saturating_sub(m);
                }
            } else if container_type == 3 {
                resp_code = op_or_code;
                break;
            }
        }

        Ok(MtpResponse {
            code: resp_code,
            data: data_payload,
        })
    }
}

#[derive(Clone, Debug)]
pub struct MtpFileInfo {
    pub handle: u32,
    pub filename: String,
    pub size: u32,
    pub parent_handle: u32,
    pub is_folder: bool,
}

struct MtpResponse {
    code: u16,
    data: Vec<u8>,
}


