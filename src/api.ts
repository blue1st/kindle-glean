import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import {
  DeviceInfo,
  DeviceProfile,
  SyncConfig,
  SyncStats,
  Clipping,
  NotebookSummary,
  VocabLookup,
  SyncProgress,
  SyncedCounts,
} from "./types";

export async function getConfig(): Promise<SyncConfig> {
  return await invoke<SyncConfig>("get_config");
}

export async function saveConfig(config: SyncConfig): Promise<void> {
  await invoke("save_config", { config });
}

export async function getDeviceProfile(deviceId: string): Promise<DeviceProfile | null> {
  return await invoke<DeviceProfile | null>("get_device_profile", { deviceId });
}

export async function saveDeviceProfile(profile: DeviceProfile): Promise<void> {
  await invoke("save_device_profile", { profile });
}

export async function getAllDeviceProfiles(): Promise<DeviceProfile[]> {
  return await invoke<DeviceProfile[]>("get_all_device_profiles");
}

export async function deleteDeviceProfile(deviceId: string): Promise<void> {
  await invoke("delete_device_profile", { deviceId });
}

export async function detectDevice(): Promise<DeviceInfo | null> {
  return await invoke<DeviceInfo | null>("detect_device");
}

export async function inspectFolder(folderPath: string): Promise<DeviceInfo | null> {
  return await invoke<DeviceInfo | null>("inspect_folder", { folderPath });
}

export async function getSyncHistory(): Promise<SyncStats[]> {
  return await invoke<SyncStats[]>("get_sync_history");
}

export async function syncNow(sourcePath?: string): Promise<SyncStats> {
  return await invoke<SyncStats>("sync_now", { sourcePath: sourcePath || null });
}

export async function previewClippings(filePath: string): Promise<Clipping[]> {
  return await invoke<Clipping[]>("preview_clippings", { filePath });
}

export async function ejectDevice(mountPath: string): Promise<boolean> {
  return await invoke<boolean>("eject_device", { mountPath });
}

export async function openFolder(path?: string): Promise<void> {
  await invoke("open_folder", { path: path || null });
}

export async function getSyncedCounts(): Promise<SyncedCounts> {
  return await invoke<SyncedCounts>("get_synced_counts");
}

export async function getSyncedClippings(): Promise<Clipping[]> {
  return await invoke<Clipping[]>("get_synced_clippings");
}

export async function getSyncedVocab(): Promise<VocabLookup[]> {
  return await invoke<VocabLookup[]>("get_synced_vocab");
}

export async function getSyncedNotebooks(): Promise<NotebookSummary[]> {
  return await invoke<NotebookSummary[]>("get_synced_notebooks");
}

export async function getNotebookPages(notebookTitle: string): Promise<string[]> {
  return await invoke<string[]>("get_notebook_pages", { notebookTitle });
}

export async function readImageBase64(filePath: string): Promise<string> {
  return await invoke<string>("read_image_base64", { filePath });
}

export async function exportNotebookPages(
  sourcePaths: string[],
  targetDir: string,
  folderName?: string
): Promise<number> {
  return await invoke<number>("export_notebook_pages", {
    sourcePaths,
    targetDir,
    folderName: folderName || null,
  });
}

export async function saveBinaryFile(filePath: string, data: Uint8Array): Promise<void> {
  await invoke("save_binary_file", {
    filePath,
    data: Array.from(data),
  });
}

export async function reexportAll(): Promise<SyncStats> {
  return await invoke<SyncStats>("reexport_all");
}

export async function getAutostartStatus(): Promise<boolean> {
  return await invoke<boolean>("get_autostart_status");
}

export async function setAutostart(enable: boolean): Promise<void> {
  await invoke("set_autostart", { enable });
}

export function onDeviceConnected(callback: (device: DeviceInfo) => void): Promise<UnlistenFn> {
  return listen<DeviceInfo>("device-connected", (event) => callback(event.payload));
}

export function onDeviceDisconnected(callback: () => void): Promise<UnlistenFn> {
  return listen("device-disconnected", () => callback());
}

export function onSyncCompleted(callback: (stats: SyncStats) => void): Promise<UnlistenFn> {
  return listen<SyncStats>("sync-completed", (event) => callback(event.payload));
}

export function onTriggerSync(callback: () => void): Promise<UnlistenFn> {
  return listen("trigger-sync", () => callback());
}

export function onSyncProgress(callback: (progress: SyncProgress) => void): Promise<UnlistenFn> {
  return listen<SyncProgress>("sync-progress", (event) => callback(event.payload));
}
