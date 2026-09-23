import React, { useState, useEffect } from "react";
import {
  Sparkles,
  Folder,
  Tablet,
  CheckCircle2,
  X,
  Sliders,
  HardDrive,
  Layers,
  FileText,
  BookOpen,
  Settings,
  ExternalLink,
} from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { DeviceInfo, DeviceProfile, SyncConfig } from "../types";
import { saveDeviceProfile, getDeviceProfile, openFolder } from "../api";

interface DeviceSetupModalProps {
  device: DeviceInfo;
  defaultConfig: SyncConfig;
  isOpen: boolean;
  onClose: () => void;
  onSaved: (profile: DeviceProfile) => void;
}

export const DeviceSetupModal: React.FC<DeviceSetupModalProps> = ({
  device,
  defaultConfig,
  isOpen,
  onClose,
  onSaved,
}) => {
  const [nickname, setNickname] = useState(device.nickname || device.device_type);
  const [vaultPath, setVaultPath] = useState(defaultConfig.vault_path);
  const [subfolder, setSubfolder] = useState(defaultConfig.subfolder || "");
  const [syncClippings, setSyncClippings] = useState(defaultConfig.sync_clippings);
  const [syncVocab, setSyncVocab] = useState(defaultConfig.sync_vocab);
  const [syncNotebooks, setSyncNotebooks] = useState(
    device.has_notebooks ? defaultConfig.sync_notebooks : false
  );
  const [autoSync, setAutoSync] = useState(defaultConfig.auto_sync);
  const [autoEject, setAutoEject] = useState(defaultConfig.auto_eject);
  const [isSaving, setIsSaving] = useState(false);

  useEffect(() => {
    if (!isOpen) return;

    // If profile exists for this device_id, fetch and populate
    const fetchExisting = async () => {
      try {
        const existing = await getDeviceProfile(device.device_id);
        if (existing) {
          setNickname(existing.nickname);
          setVaultPath(existing.vault_path);
          setSubfolder(existing.subfolder);
          setSyncClippings(existing.sync_clippings);
          setSyncVocab(existing.sync_vocab);
          setSyncNotebooks(existing.sync_notebooks);
          setAutoSync(existing.auto_sync);
          setAutoEject(existing.auto_eject);
          return;
        }
      } catch (e) {
        // Not existing or error, fall through
      }

      setNickname(device.nickname || device.device_type);
      setVaultPath(defaultConfig.vault_path);
      setSubfolder(defaultConfig.subfolder || "");
      setSyncClippings(defaultConfig.sync_clippings);
      setSyncVocab(defaultConfig.sync_vocab);
      setSyncNotebooks(device.has_notebooks ? defaultConfig.sync_notebooks : false);
      setAutoSync(defaultConfig.auto_sync);
      setAutoEject(defaultConfig.auto_eject);
    };

    fetchExisting();
  }, [isOpen, device, defaultConfig]);

  if (!isOpen) return null;

  const handleSelectFolder = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        defaultPath: vaultPath || undefined,
        title: `${nickname} の同期先フォルダを選択`,
      });
      if (selected && typeof selected === "string") {
        setVaultPath(selected);
      }
    } catch (err) {
      console.error("フォルダ選択エラー:", err);
    }
  };

  const handleSave = async () => {
    if (!nickname.trim() || !vaultPath.trim()) return;

    setIsSaving(true);
    try {
      const now = new Date().toISOString();
      const profile: DeviceProfile = {
        device_id: device.device_id,
        nickname: nickname.trim(),
        device_type: device.device_type,
        vault_path: vaultPath.trim(),
        subfolder: subfolder.trim(),
        sync_clippings: syncClippings,
        sync_vocab: syncVocab,
        sync_notebooks: syncNotebooks,
        auto_sync: autoSync,
        auto_eject: autoEject,
        created_at: now,
        last_connected_at: now,
      };

      await saveDeviceProfile(profile);
      onSaved(profile);
      onClose();
    } catch (err) {
      console.error("デバイスプロファイルの保存に失敗しました:", err);
    } finally {
      setIsSaving(false);
    }
  };

  const isEditing = device.is_registered;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/80 backdrop-blur-sm p-4 overflow-y-auto animate-in fade-in duration-200">
      <div className="bg-zinc-900 rounded-2xl shadow-2xl border border-zinc-800 max-w-xl w-full overflow-hidden animate-in zoom-in-95 duration-200 flex flex-col">
        {/* Header */}
        <div className="bg-gradient-to-r from-amber-600/30 via-orange-600/20 to-zinc-900 px-6 py-5 border-b border-zinc-800 flex items-start justify-between">
          <div className="flex items-center gap-3">
            <div className="p-2.5 bg-amber-500/20 text-amber-400 border border-amber-500/30 rounded-xl">
              {isEditing ? <Settings className="w-5 h-5" /> : <Sparkles className="w-5 h-5" />}
            </div>
            <div>
              <h3 className="text-base font-semibold text-zinc-100">
                {isEditing ? "Kindle端末の個別設定" : "新しいKindleを検出しました"}
              </h3>
              <p className="text-zinc-400 text-xs mt-0.5">
                {isEditing
                  ? "ニックネームや同期先フォルダ、同期対象の個別変更が可能です"
                  : "この端末専用のニックネームや保存先フォルダを設定できます"}
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="text-zinc-400 hover:text-zinc-200 p-1.5 rounded-lg hover:bg-zinc-800 transition-colors"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        <div className="p-6 space-y-5 max-h-[70vh] overflow-y-auto">
          {/* Detected Hardware Info */}
          <div className="flex items-center gap-3 p-3.5 bg-zinc-950/80 border border-zinc-800 rounded-xl">
            <Tablet className="w-5 h-5 text-amber-400 shrink-0" />
            <div className="text-xs">
              <span className="font-semibold text-zinc-200">{device.device_type}</span>
              <span className="text-zinc-500 ml-2 font-mono">ID: {device.device_id}</span>
              <div className="text-zinc-400 mt-0.5">
                検出機能:{" "}
                {[
                  device.has_clippings && "ハイライト",
                  device.has_vocab && "単語帳",
                  device.has_notebooks && "手書きノート",
                ]
                  .filter(Boolean)
                  .join(" / ") || "未検出"}
              </div>
            </div>
          </div>

          {/* Nickname */}
          <div>
            <label className="block text-xs font-medium text-zinc-300 mb-1.5">
              端末のニックネーム
            </label>
            <input
              type="text"
              value={nickname}
              onChange={(e) => setNickname(e.target.value)}
              placeholder="例: 書斎のKindle Scribe / 持ち歩き用Paperwhite"
              className="w-full text-sm px-3.5 py-2.5 bg-zinc-950 border border-zinc-700/80 rounded-xl text-zinc-100 placeholder-zinc-500 focus:ring-2 focus:ring-amber-500/50 focus:border-amber-500 outline-none transition-all"
            />
            <p className="text-[11px] text-zinc-500 mt-1">
              接続時の表示やデバイスごとの管理名として使用されます。
            </p>
          </div>

          {/* Vault Path */}
          <div>
            <label className="block text-xs font-medium text-zinc-300 mb-1.5 flex items-center justify-between">
              <span>この端末の同期先フォルダ</span>
              <span className="text-[11px] text-amber-400/80 font-normal">端末ごとに個別フォルダを指定可能</span>
            </label>
            <div className="flex gap-2">
              <input
                type="text"
                value={vaultPath}
                onChange={(e) => setVaultPath(e.target.value)}
                placeholder="/Users/username/Documents/KindleGlean"
                className="flex-1 text-sm font-mono px-3.5 py-2.5 bg-zinc-950 border border-zinc-700/80 rounded-xl text-zinc-100 placeholder-zinc-500 focus:ring-2 focus:ring-amber-500/50 focus:border-amber-500 outline-none transition-all"
                required
              />
              <button
                type="button"
                onClick={handleSelectFolder}
                className="px-3.5 py-2.5 bg-zinc-800 hover:bg-zinc-700 text-zinc-200 text-sm font-medium rounded-xl border border-zinc-700/60 transition-colors flex items-center gap-1.5 shrink-0 cursor-pointer"
              >
                <Folder className="w-4 h-4 text-amber-400" />
                参照
              </button>
              <button
                type="button"
                onClick={() => openFolder(vaultPath)}
                title="このフォルダをFinder/Explorerで開く"
                className="p-2.5 bg-zinc-800 hover:bg-zinc-700 text-indigo-400 rounded-xl border border-zinc-700/60 transition-colors shrink-0 cursor-pointer"
              >
                <ExternalLink className="w-4 h-4" />
              </button>
            </div>
            {vaultPath !== defaultConfig.vault_path && (
              <button
                type="button"
                onClick={() => setVaultPath(defaultConfig.vault_path)}
                className="mt-1.5 text-xs text-indigo-400 hover:text-indigo-300 hover:underline cursor-pointer flex items-center gap-1"
              >
                <span>↩ アプリのデフォルト保存先（{defaultConfig.vault_path}）を使用</span>
              </button>
            )}
          </div>

          {/* Subfolder */}
          <div>
            <label className="block text-xs font-medium text-zinc-300 mb-1.5">
              サブフォルダ（任意）
            </label>
            <div className="flex items-center gap-2">
              <span className="text-xs text-zinc-500 font-mono">同期先 /</span>
              <input
                type="text"
                value={subfolder}
                onChange={(e) => setSubfolder(e.target.value)}
                placeholder="例: ScribeNotes （空欄の場合は同期先直下）"
                className="flex-1 text-sm px-3.5 py-2 bg-zinc-950 border border-zinc-700/80 rounded-xl text-zinc-100 placeholder-zinc-500 focus:ring-2 focus:ring-amber-500/50 focus:border-amber-500 outline-none transition-all"
              />
            </div>
            <p className="text-[11px] text-zinc-500 mt-1">
              指定した場合、`{vaultPath || "同期先"}/{subfolder || "<サブフォルダ>"}/Books` のように整理されます。
            </p>
          </div>

          {/* Sync Options */}
          <div className="pt-2 border-t border-zinc-800/80">
            <label className="block text-xs font-medium text-zinc-300 mb-2.5">
              同期オプション
            </label>
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-2 text-xs">
              <label className="flex items-center gap-2.5 p-3 rounded-xl bg-zinc-950/60 border border-zinc-800/80 hover:border-zinc-700 transition-colors cursor-pointer">
                <input
                  type="checkbox"
                  checked={syncClippings}
                  onChange={(e) => setSyncClippings(e.target.checked)}
                  className="rounded text-amber-500 focus:ring-amber-500 accent-amber-500"
                />
                <BookOpen className="w-4 h-4 text-zinc-400" />
                <span className="text-zinc-200">ハイライト & メモ</span>
              </label>

              <label className="flex items-center gap-2.5 p-3 rounded-xl bg-zinc-950/60 border border-zinc-800/80 hover:border-zinc-700 transition-colors cursor-pointer">
                <input
                  type="checkbox"
                  checked={syncVocab}
                  onChange={(e) => setSyncVocab(e.target.checked)}
                  className="rounded text-amber-500 focus:ring-amber-500 accent-amber-500"
                />
                <FileText className="w-4 h-4 text-zinc-400" />
                <span className="text-zinc-200">語彙ログ (`vocab.db`)</span>
              </label>

              {device.has_notebooks && (
                <label className="flex items-center gap-2.5 p-3 rounded-xl bg-zinc-950/60 border border-zinc-800/80 hover:border-zinc-700 transition-colors cursor-pointer">
                  <input
                    type="checkbox"
                    checked={syncNotebooks}
                    onChange={(e) => setSyncNotebooks(e.target.checked)}
                    className="rounded text-amber-500 focus:ring-amber-500 accent-amber-500"
                  />
                  <Layers className="w-4 h-4 text-zinc-400" />
                  <span className="text-zinc-200">Scribe手書きノート</span>
                </label>
              )}

              <label className="flex items-center gap-2.5 p-3 rounded-xl bg-zinc-950/60 border border-zinc-800/80 hover:border-zinc-700 transition-colors cursor-pointer">
                <input
                  type="checkbox"
                  checked={autoSync}
                  onChange={(e) => setAutoSync(e.target.checked)}
                  className="rounded text-amber-500 focus:ring-amber-500 accent-amber-500"
                />
                <Sliders className="w-4 h-4 text-zinc-400" />
                <span className="text-zinc-200">接続時に自動同期</span>
              </label>

              {device.connection_mode !== "MTP" && (
                <label className="flex items-center gap-2.5 p-3 rounded-xl bg-zinc-950/60 border border-zinc-800/80 hover:border-zinc-700 transition-colors cursor-pointer">
                  <input
                    type="checkbox"
                    checked={autoEject}
                    onChange={(e) => setAutoEject(e.target.checked)}
                    className="rounded text-amber-500 focus:ring-amber-500 accent-amber-500"
                  />
                  <HardDrive className="w-4 h-4 text-zinc-400" />
                  <span className="text-zinc-200">同期後に安全に取り出す</span>
                </label>
              )}
            </div>
          </div>
        </div>

        {/* Footer */}
        <div className="bg-zinc-950 px-6 py-4 border-t border-zinc-800 flex items-center justify-between">
          <button
            type="button"
            onClick={onClose}
            className="text-xs text-zinc-400 hover:text-zinc-200 px-3 py-2 rounded-lg transition-colors"
          >
            {isEditing ? "キャンセル" : "スキップ（全体設定を使用）"}
          </button>

          <button
            type="button"
            onClick={handleSave}
            disabled={isSaving || !nickname.trim() || !vaultPath.trim()}
            className="flex items-center gap-2 px-5 py-2.5 bg-gradient-to-r from-amber-500 to-orange-500 hover:from-amber-600 hover:to-orange-600 text-white text-xs font-medium rounded-xl shadow-md transition-all disabled:opacity-50"
          >
            <CheckCircle2 className="w-4 h-4" />
            {isSaving
              ? "保存中..."
              : isEditing
              ? "設定を保存"
              : "この設定で保存して同期を開始"}
          </button>
        </div>
      </div>
    </div>
  );
};
