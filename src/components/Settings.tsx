import React, { useState, useEffect } from "react";
import packageJson from "../../package.json";
import { SyncConfig, DeviceProfile } from "../types";
import {
  Folder,
  Save,
  CheckCircle2,
  Sliders,
  HardDrive,
  ExternalLink,
  RefreshCw,
  Layers,
  Tablet,
  Trash2,
  Edit3,
  FolderOpen,
} from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  saveConfig,
  openFolder,
  reexportAll,
  getAllDeviceProfiles,
  deleteDeviceProfile,
  getAutostartStatus,
  setAutostart,
} from "../api";
import { DeviceSetupModal } from "./DeviceSetupModal";

interface Props {
  config: SyncConfig;
  onConfigUpdated: (newConfig: SyncConfig) => void;
}

export const Settings: React.FC<Props> = ({
  config,
  onConfigUpdated,
}) => {
  const [formData, setFormData] = useState<SyncConfig>(config);
  const [saving, setSaving] = useState(false);
  const [savedMessage, setSavedMessage] = useState(false);
  const [reexporting, setReexporting] = useState(false);
  const [reexportMessage, setReexportMessage] = useState<string | null>(null);
  const [deviceProfiles, setDeviceProfiles] = useState<DeviceProfile[]>([]);
  const [editingProfile, setEditingProfile] = useState<DeviceProfile | null>(null);
  const [autostartEnabled, setAutostartEnabled] = useState<boolean>(false);
  const [autostartLoading, setAutostartLoading] = useState<boolean>(false);

  const loadProfiles = async () => {
    try {
      const profiles = await getAllDeviceProfiles();
      setDeviceProfiles(profiles);
    } catch (err) {
      console.error("Failed to load device profiles:", err);
    }
  };

  const loadAutostart = async () => {
    try {
      const enabled = await getAutostartStatus();
      setAutostartEnabled(enabled);
    } catch (err) {
      console.error("Failed to load autostart status:", err);
    }
  };

  const handleToggleAutostart = async (enabled: boolean) => {
    setAutostartLoading(true);
    try {
      await setAutostart(enabled);
      setAutostartEnabled(enabled);
    } catch (err) {
      console.error("Failed to update autostart:", err);
      alert(`自動起動の設定更新に失敗しました: ${err}`);
    } finally {
      setAutostartLoading(false);
    }
  };

  const handleDeleteProfile = async (deviceId: string, nickname: string) => {
    if (
      !confirm(
        `端末「${nickname}」の設定プロファイルを削除しますか？\n\n※端末内のデータや同期済みファイルは削除されません。`
      )
    ) {
      return;
    }
    try {
      await deleteDeviceProfile(deviceId);
      await loadProfiles();
    } catch (err) {
      console.error("Failed to delete profile:", err);
    }
  };

  useEffect(() => {
    loadProfiles();
    loadAutostart();
  }, []);

  const handlePickVault = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        defaultPath: formData.vault_path || undefined,
        title: "同期出力先フォルダを選択",
      });
      if (selected && typeof selected === "string") {
        setFormData((prev) => ({ ...prev, vault_path: selected }));
      }
    } catch (e) {
      console.error(e);
    }
  };

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    setSaving(true);
    try {
      await saveConfig(formData);
      onConfigUpdated(formData);
      setSavedMessage(true);
      setTimeout(() => setSavedMessage(false), 3000);
    } catch (err) {
      console.error(err);
    } finally {
      setSaving(false);
    }
  };

  const handleReexport = async () => {
    setReexporting(true);
    setReexportMessage(null);
    try {
      // First save current form config if changed
      await saveConfig(formData);
      onConfigUpdated(formData);
      const res = await reexportAll();
      setReexportMessage(res.message);
      setTimeout(() => setReexportMessage(null), 5000);
    } catch (err) {
      console.error("Reexport failed:", err);
      setReexportMessage(`再出力に失敗しました: ${err}`);
    } finally {
      setReexporting(false);
    }
  };

  return (
    <form onSubmit={handleSave} className="space-y-6 max-w-4xl">
      {/* Page Header Notice */}
      <div className="bg-zinc-900/40 border border-zinc-800/60 rounded-2xl p-5">
        <h2 className="text-lg font-bold text-zinc-100 mb-1">
          アプリ全体設定 & ライブラリ管理
        </h2>
        <p className="text-xs text-zinc-400 leading-relaxed">
          未登録の新しいKindle端末を接続した際に使用されるデフォルトの保存先や同期項目、登録済み端末プロファイルの一覧管理、ライブラリ全体の再出力を行います。
        </p>
        <p className="text-xs text-indigo-400/90 mt-2 flex items-center gap-1.5">
          <span>💡</span>
          <span>接続中のKindle固有の同期先やニックネームは、ホーム画面の端末カード内「この端末の同期設定を変更」から直接設定できます。</span>
        </p>
      </div>

      {/* Vault Path Setting (Global Default) */}
      <div className="bg-zinc-900/60 border border-zinc-800/80 rounded-2xl p-6">
        <div className="flex items-center gap-3 mb-4">
          <div className="p-2 rounded-xl bg-indigo-500/10 text-indigo-400">
            <Folder className="w-5 h-5" />
          </div>
          <div>
            <h3 className="text-base font-semibold text-zinc-100">新規端末用デフォルト同期先フォルダ</h3>
            <p className="text-xs text-zinc-400">
              未登録のKindleを接続した際に初期設定として提案されるフォルダです (Obsidian Vault を指定可能)
            </p>
          </div>
        </div>

        <div className="space-y-4">
          <div>
            <label className="block text-xs font-medium text-zinc-400 mb-1.5">
              保存先ディレクトリ
            </label>
            <div className="flex gap-2">
              <input
                type="text"
                value={formData.vault_path}
                onChange={(e) =>
                  setFormData((prev) => ({ ...prev, vault_path: e.target.value }))
                }
                className="flex-1 px-3.5 py-2 rounded-xl bg-zinc-950 border border-zinc-700/60 text-sm text-zinc-200 focus:outline-none focus:border-indigo-500 font-mono"
                placeholder="/Users/username/Documents/KindleGlean"
                required
              />
              <button
                type="button"
                onClick={handlePickVault}
                className="px-4 py-2 rounded-xl bg-zinc-800 hover:bg-zinc-700 text-zinc-200 text-sm font-medium border border-zinc-700/50 transition-colors shrink-0"
              >
                フォルダ選択
              </button>
              <button
                type="button"
                onClick={() => openFolder()}
                title="現在の同期先フォルダをFinder/Explorerで開く"
                className="p-2 rounded-xl bg-zinc-800 hover:bg-zinc-700 text-indigo-400 border border-zinc-700/50 transition-colors shrink-0"
              >
                <ExternalLink className="w-5 h-5" />
              </button>
            </div>
          </div>

          <div>
            <label className="block text-xs font-medium text-zinc-400 mb-1.5">
              サブフォルダ名 (省略可)
            </label>
            <input
              type="text"
              value={formData.subfolder}
              onChange={(e) =>
                setFormData((prev) => ({ ...prev, subfolder: e.target.value }))
              }
              className="w-full max-w-xs px-3.5 py-2 rounded-xl bg-zinc-950 border border-zinc-700/60 text-sm text-zinc-200 focus:outline-none focus:border-indigo-500"
              placeholder="(空欄で直下に出力、または Kindle)"
            />
            <p className="text-xs text-zinc-500 mt-1">
              指定したフォルダ配下に Books/, Vocabulary/, Notebooks/ が作成されます。空欄の場合は出力先直下に作成されます。手書きノートはフォルダごとにMarkdownと画像が自己完結して配置されます。
            </p>
          </div>
        </div>
      </div>

      {/* Sync Items Toggle */}
      <div className="bg-zinc-900/60 border border-zinc-800/80 rounded-2xl p-6">
        <div className="flex items-center gap-3 mb-4">
          <div className="p-2 rounded-xl bg-indigo-500/10 text-indigo-400">
            <Sliders className="w-5 h-5" />
          </div>
          <div>
            <h3 className="text-base font-semibold text-zinc-100">新規端末用デフォルト同期対象</h3>
            <p className="text-xs text-zinc-400">未登録のKindle接続時に初期選択されるコンテンツです</p>
          </div>
        </div>

        <div className="space-y-3">
          <label className="flex items-center justify-between p-3.5 rounded-xl bg-zinc-950/60 border border-zinc-800/80 hover:border-zinc-700 transition-colors cursor-pointer">
            <div>
              <div className="text-sm font-medium text-zinc-200">
                書籍ハイライト & メモ (`My Clippings.txt`)
              </div>
              <div className="text-xs text-zinc-400">
                書籍ごとにMarkdownファイルを生成・差分追記します
              </div>
            </div>
            <input
              type="checkbox"
              checked={formData.sync_clippings}
              onChange={(e) =>
                setFormData((prev) => ({ ...prev, sync_clippings: e.target.checked }))
              }
              className="w-4 h-4 rounded text-indigo-600 focus:ring-indigo-500 accent-indigo-600"
            />
          </label>

          <label className="flex items-center justify-between p-3.5 rounded-xl bg-zinc-950/60 border border-zinc-800/80 hover:border-zinc-700 transition-colors cursor-pointer">
            <div>
              <div className="text-sm font-medium text-zinc-200">
                語彙ログ & 単語帳 (`vocab.db`)
              </div>
              <div className="text-xs text-zinc-400">
                Kindleで調べた単語・原形・例文をまとめます
              </div>
            </div>
            <input
              type="checkbox"
              checked={formData.sync_vocab}
              onChange={(e) =>
                setFormData((prev) => ({ ...prev, sync_vocab: e.target.checked }))
              }
              className="w-4 h-4 rounded text-indigo-600 focus:ring-indigo-500 accent-indigo-600"
            />
          </label>

          <label className="flex items-center justify-between p-3.5 rounded-xl bg-zinc-950/60 border border-zinc-800/80 hover:border-zinc-700 transition-colors cursor-pointer">
            <div>
              <div className="text-sm font-medium text-zinc-200">
                手書きノート (`.nbk` - Kindle Scribe)
              </div>
              <div className="text-xs text-zinc-400">
                手書きノートをベクター画像として抽出・Obsidian埋め込みリンク化します
              </div>
            </div>
            <input
              type="checkbox"
              checked={formData.sync_notebooks}
              onChange={(e) =>
                setFormData((prev) => ({ ...prev, sync_notebooks: e.target.checked }))
              }
              className="w-4 h-4 rounded text-indigo-600 focus:ring-indigo-500 accent-indigo-600"
            />
          </label>
        </div>
      </div>

      {/* Automation Options */}
      <div className="bg-zinc-900/60 border border-zinc-800/80 rounded-2xl p-6">
        <div className="flex items-center gap-3 mb-4">
          <div className="p-2 rounded-xl bg-indigo-500/10 text-indigo-400">
            <HardDrive className="w-5 h-5" />
          </div>
          <div>
            <h3 className="text-base font-semibold text-zinc-100">全体デフォルト動作設定</h3>
            <p className="text-xs text-zinc-400">USB接続時の自動同期や取り外しのデフォルト動作です</p>
          </div>
        </div>

        <div className="space-y-3">
          <label className="flex items-center justify-between p-3.5 rounded-xl bg-zinc-950/60 border border-zinc-800/80 hover:border-zinc-700 transition-colors cursor-pointer">
            <div>
              <div className="text-sm font-medium text-zinc-200">
                USB接続時の自動同期
              </div>
              <div className="text-xs text-zinc-400">
                Kindleが接続されるとバックグラウンドで自動同期を実行します
              </div>
            </div>
            <input
              type="checkbox"
              checked={formData.auto_sync}
              onChange={(e) =>
                setFormData((prev) => ({ ...prev, auto_sync: e.target.checked }))
              }
              className="w-4 h-4 rounded text-indigo-600 focus:ring-indigo-500 accent-indigo-600"
            />
          </label>

          <label className="flex items-center justify-between p-3.5 rounded-xl bg-zinc-950/60 border border-zinc-800/80 hover:border-zinc-700 transition-colors cursor-pointer">
            <div>
              <div className="text-sm font-medium text-zinc-200">
                同期完了後の自動アンマウント (自動イジェクト)
              </div>
              <div className="text-xs text-zinc-400">
                同期終了後にOS側で安全に取り外せる状態にします
              </div>
            </div>
            <input
              type="checkbox"
              checked={formData.auto_eject}
              onChange={(e) =>
                setFormData((prev) => ({ ...prev, auto_eject: e.target.checked }))
              }
              className="w-4 h-4 rounded text-indigo-600 focus:ring-indigo-500 accent-indigo-600"
            />
          </label>

          <label className="flex items-center justify-between p-3.5 rounded-xl bg-zinc-950/60 border border-zinc-800/80 hover:border-zinc-700 transition-colors cursor-pointer">
            <div>
              <div className="text-sm font-medium text-zinc-200 flex items-center gap-2">
                <span>PC起動時の自動起動 (バックグラウンド常駐)</span>
                {autostartLoading && (
                  <span className="text-[11px] text-indigo-400 animate-pulse">更新中...</span>
                )}
              </div>
              <div className="text-xs text-zinc-400">
                PCログイン時にアプリを自動起動してトレイに常駐させ、Kindle接続時の自動同期に備えます
              </div>
            </div>
            <input
              type="checkbox"
              checked={autostartEnabled}
              disabled={autostartLoading}
              onChange={(e) => handleToggleAutostart(e.target.checked)}
              className="w-4 h-4 rounded text-indigo-600 focus:ring-indigo-500 accent-indigo-600 disabled:opacity-50"
            />
          </label>
        </div>
      </div>

      {/* Registered Devices Management */}
      <div className="bg-zinc-900/60 border border-zinc-800/80 rounded-2xl p-6">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-3">
            <div className="p-2 rounded-xl bg-amber-500/10 text-amber-400">
              <Tablet className="w-5 h-5" />
            </div>
            <div>
              <h3 className="text-base font-semibold text-zinc-100">
                登録済みKindleデバイスプロファイル
              </h3>
              <p className="text-xs text-zinc-400">
                過去に初期設定を行ったKindle端末の一覧です。保存先や設定の編集・解除が行えます
              </p>
            </div>
          </div>
          {deviceProfiles.length > 0 && (
            <span className="text-xs px-2.5 py-1 rounded-full bg-zinc-800 border border-zinc-700/60 text-zinc-300 font-mono">
              {deviceProfiles.length} 台登録済み
            </span>
          )}
        </div>

        {deviceProfiles.length === 0 ? (
          <div className="p-5 rounded-xl bg-zinc-950/60 border border-zinc-800/60 text-center">
            <Tablet className="w-8 h-8 text-zinc-600 mx-auto mb-2 opacity-60" />
            <p className="text-xs text-zinc-400 font-medium">登録されている個別デバイスはありません</p>
            <p className="text-[11px] text-zinc-500 mt-1">
              KindleをUSB接続した際に初期設定を行うと、デバイスごとのプロファイルが自動作成されます。
            </p>
          </div>
        ) : (
          <div className="space-y-3">
            {deviceProfiles.map((p) => (
              <div
                key={p.device_id}
                className="p-4 rounded-xl bg-zinc-950/80 border border-zinc-800/80 hover:border-zinc-700 transition-all flex flex-col sm:flex-row sm:items-center justify-between gap-4"
              >
                <div className="space-y-1.5 flex-1 min-w-0">
                  <div className="flex items-center gap-2 flex-wrap">
                    <span className="text-sm font-semibold text-zinc-100">{p.nickname}</span>
                    <span className="px-2 py-0.5 rounded-md bg-amber-500/10 border border-amber-500/30 text-amber-300 text-[11px] font-medium">
                      {p.device_type}
                    </span>
                    <span className="text-xs text-zinc-500 font-mono">ID: {p.device_id}</span>
                  </div>

                  <div className="text-xs text-zinc-400 flex items-center gap-1.5 truncate">
                    <Folder className="w-3.5 h-3.5 text-zinc-500 shrink-0" />
                    <span className="font-mono text-zinc-300 truncate">
                      {p.vault_path}
                      {p.subfolder ? ` / ${p.subfolder}` : ""}
                    </span>
                    <button
                      type="button"
                      onClick={() =>
                        openFolder(p.subfolder ? `${p.vault_path}/${p.subfolder}` : p.vault_path)
                      }
                      title="この端末の保存先フォルダを開く"
                      className="p-1 rounded hover:bg-zinc-800 text-indigo-400 hover:text-indigo-300 transition-colors shrink-0"
                    >
                      <FolderOpen className="w-3.5 h-3.5" />
                    </button>
                  </div>

                  <div className="flex items-center gap-2 text-[11px] text-zinc-500 flex-wrap">
                    <span className={p.sync_clippings ? "text-indigo-400" : "text-zinc-600 line-through"}>
                      ハイライト
                    </span>
                    <span>・</span>
                    <span className={p.sync_vocab ? "text-indigo-400" : "text-zinc-600 line-through"}>
                      単語帳
                    </span>
                    <span>・</span>
                    <span className={p.sync_notebooks ? "text-indigo-400" : "text-zinc-600 line-through"}>
                      ノート
                    </span>
                    <span>・</span>
                    <span className={p.auto_sync ? "text-emerald-400" : "text-zinc-600"}>
                      自動同期: {p.auto_sync ? "ON" : "OFF"}
                    </span>
                    <span>・</span>
                    <span className={p.auto_eject ? "text-emerald-400" : "text-zinc-600"}>
                      自動取外: {p.auto_eject ? "ON" : "OFF"}
                    </span>
                    {p.last_connected_at && (
                      <>
                        <span className="hidden sm:inline">・</span>
                        <span className="text-zinc-500">
                          最終接続: {new Date(p.last_connected_at).toLocaleDateString("ja-JP")}
                        </span>
                      </>
                    )}
                  </div>
                </div>

                <div className="flex items-center gap-2 shrink-0 self-end sm:self-center">
                  <button
                    type="button"
                    onClick={() => setEditingProfile(p)}
                    className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-zinc-800 hover:bg-zinc-700 text-zinc-200 text-xs font-medium border border-zinc-700/50 transition-colors"
                  >
                    <Edit3 className="w-3.5 h-3.5 text-zinc-400" />
                    編集
                  </button>
                  <button
                    type="button"
                    onClick={() => handleDeleteProfile(p.device_id, p.nickname)}
                    title="端末プロファイルを削除"
                    className="p-1.5 rounded-lg bg-zinc-800/60 hover:bg-rose-500/20 text-zinc-400 hover:text-rose-400 border border-zinc-700/50 hover:border-rose-500/30 transition-colors"
                  >
                    <Trash2 className="w-3.5 h-3.5" />
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Re-export / Resync Card */}
      <div className="bg-zinc-900/60 border border-zinc-800/80 rounded-2xl p-6">
        <div className="flex items-center gap-3 mb-3">
          <div className="p-2 rounded-xl bg-indigo-500/10 text-indigo-400">
            <Layers className="w-5 h-5" />
          </div>
          <div>
            <h3 className="text-base font-semibold text-zinc-100">ライブラリの再出力</h3>
            <p className="text-xs text-zinc-400">
              同期先フォルダを変更した際や、既存のハイライト・単語・ノートを現在の保存先フォルダへ一括再生成したい場合に実行します
            </p>
          </div>
        </div>

        <div className="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-3 pt-2">
          <button
            type="button"
            onClick={handleReexport}
            disabled={reexporting}
            className="flex items-center gap-2 px-5 py-2.5 rounded-xl bg-zinc-800 hover:bg-zinc-700 text-zinc-100 text-sm font-medium border border-zinc-700/60 transition-colors"
          >
            <RefreshCw className={`w-4 h-4 text-indigo-400 ${reexporting ? "animate-spin" : ""}`} />
            {reexporting ? "出力中..." : "現在の保存先へ全コンテンツを一括出力"}
          </button>

          {reexportMessage && (
            <div className="flex items-center gap-1.5 text-xs text-emerald-400">
              <CheckCircle2 className="w-4 h-4" />
              {reexportMessage}
            </div>
          )}
        </div>
      </div>

      {/* Save Button */}
      <div className="flex items-center gap-4">
        <button
          type="submit"
          disabled={saving}
          className="flex items-center gap-2 px-6 py-2.5 rounded-xl bg-indigo-600 hover:bg-indigo-500 text-white text-sm font-medium transition-all shadow-lg shadow-indigo-600/20"
        >
          <Save className="w-4 h-4" />
          {saving ? "保存中..." : "全体デフォルト設定を保存"}
        </button>

        {savedMessage && (
          <div className="flex items-center gap-1.5 text-sm text-emerald-400">
            <CheckCircle2 className="w-4 h-4" />
            設定を保存しました
          </div>
        )}
      </div>

      {/* App Version Info */}
      <div className="pt-4 border-t border-zinc-900 text-center text-xs text-zinc-500 font-mono">
        Kindle Glean v{packageJson.version}
      </div>

      {/* Profile Edit Modal */}
      {editingProfile && (
        <DeviceSetupModal
          device={{
            device_id: editingProfile.device_id,
            device_type: editingProfile.device_type,
            connection_mode: "USB",
            mount_path: "",
            has_clippings: editingProfile.sync_clippings,
            has_vocab: editingProfile.sync_vocab,
            has_notebooks: editingProfile.sync_notebooks,
            connected: false,
            is_registered: true,
            nickname: editingProfile.nickname,
          }}
          defaultConfig={{
            vault_path: editingProfile.vault_path,
            subfolder: editingProfile.subfolder,
            sync_clippings: editingProfile.sync_clippings,
            sync_vocab: editingProfile.sync_vocab,
            sync_notebooks: editingProfile.sync_notebooks,
            auto_sync: editingProfile.auto_sync,
            auto_eject: editingProfile.auto_eject,
          }}
          isOpen={!!editingProfile}
          onClose={() => setEditingProfile(null)}
          onSaved={async () => {
            await loadProfiles();
            setEditingProfile(null);
          }}
        />
      )}
    </form>
  );
};
