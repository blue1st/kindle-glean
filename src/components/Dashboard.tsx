import React, { useState, useRef, useEffect } from "react";
import { DeviceInfo, SyncStats, SyncProgress, SyncConfig, DeviceProfile, SyncedCounts } from "../types";
import {
  HardDrive,
  RefreshCw,
  BookOpen,
  Highlighter,
  FileText,
  BookmarkCheck,
  CheckCircle2,
  AlertCircle,
  Clock,
  Sparkles,
  Settings2,
  ChevronRight,
  Folder,
} from "lucide-react";
import { syncNow, onSyncProgress, getDeviceProfile } from "../api";
import { DeviceSetupModal } from "./DeviceSetupModal";

interface Props {
  device: DeviceInfo | null;
  onRefreshDevice: () => void;
  syncHistory: SyncStats[];
  counts?: SyncedCounts | null;
  onSyncFinished: () => void;
  config?: SyncConfig;
  onNavigateToContent: (tab: "clippings" | "notebooks" | "vocab", filterType?: "all" | "highlight" | "note") => void;
}

export const Dashboard: React.FC<Props> = ({
  device,
  onRefreshDevice,
  syncHistory,
  counts,
  onSyncFinished,
  config,
  onNavigateToContent,
}) => {
  const [syncing, setSyncing] = useState(false);
  const [syncMessage, setSyncMessage] = useState<{ text: string; isError: boolean } | null>(null);
  const [progress, setProgress] = useState<SyncProgress | null>(null);
  const [showSetupModal, setShowSetupModal] = useState(false);
  const [profile, setProfile] = useState<DeviceProfile | null>(null);
  const isSyncingRef = useRef(false);
  const hasAutoPromptedRef = useRef<string | null>(null);

  useEffect(() => {
    if (device?.connected && device.device_id) {
      getDeviceProfile(device.device_id)
        .then(setProfile)
        .catch(() => setProfile(null));
    } else {
      setProfile(null);
    }
  }, [device]);

  useEffect(() => {
    if (device?.connected && !device.is_registered) {
      if (hasAutoPromptedRef.current !== device.device_id) {
        hasAutoPromptedRef.current = device.device_id;
        setShowSetupModal(true);
      }
    }
  }, [device]);

  useEffect(() => {
    const unlistenPromise = onSyncProgress((p) => {
      setProgress(p);
    });
    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

  const handleSyncNow = async (path?: string) => {
    if (isSyncingRef.current) return;
    isSyncingRef.current = true;
    setSyncing(true);
    setProgress({
      step: "starting",
      message: "同期を開始しています...",
      percentage: 5,
    });
    setSyncMessage(null);
    try {
      const stats = await syncNow(path);
      setSyncMessage({
        text: stats.message || "同期が完了しました",
        isError: !stats.success,
      });
      onSyncFinished();
    } catch (err: any) {
      setSyncMessage({
        text: typeof err === "string" ? err : err.message || "同期中にエラーが発生しました",
        isError: true,
      });
    } finally {
      isSyncingRef.current = false;
      setSyncing(false);
      setTimeout(() => {
        setProgress(null);
      }, 3000);
    }
  };

  // Synced content totals: Use actual unique items from DB if available, fallback to history sum
  const totalHighlights = counts != null ? counts.highlights : syncHistory.reduce((acc, h) => acc + h.highlights_added, 0);
  const totalNotes = counts != null ? counts.notes : syncHistory.reduce((acc, h) => acc + h.notes_added, 0);
  const totalVocab = counts != null ? counts.vocab : syncHistory.reduce((acc, h) => acc + h.vocab_added, 0);
  const totalNotebooks = counts != null ? counts.notebooks : syncHistory.reduce((acc, h) => acc + h.notebooks_added, 0);

  return (
    <div className="space-y-6">
      {/* Device Connection Status Banner */}
      <div
        className={`p-6 rounded-2xl border transition-all duration-300 ${
          device?.connected
            ? "bg-gradient-to-r from-emerald-500/10 via-teal-500/5 to-transparent border-emerald-500/30"
            : "bg-gradient-to-r from-zinc-800/40 via-zinc-800/20 to-transparent border-zinc-700/40"
        }`}
      >
        <div className="flex flex-col md:flex-row md:items-center justify-between gap-4">
          <div className="flex items-start gap-4">
            <button
              type="button"
              onClick={onRefreshDevice}
              title="クリックしてKindleの接続状態を再検出・更新"
              className={`p-3.5 rounded-xl transition-all cursor-pointer group relative text-left shrink-0 ${
                device?.connected
                  ? "bg-emerald-500/20 text-emerald-400 ring-1 ring-emerald-500/40 hover:bg-emerald-500/30 hover:ring-emerald-400/60 active:scale-95"
                  : "bg-zinc-800 text-zinc-400 border border-zinc-700/50 hover:bg-zinc-700 hover:text-zinc-200 active:scale-95"
              }`}
            >
              <HardDrive className="w-7 h-7 group-hover:scale-105 transition-transform" />
              <span
                className="absolute -bottom-1 -right-1 p-1 rounded-full bg-zinc-900 border border-zinc-700 text-zinc-400 group-hover:text-emerald-400 group-hover:border-emerald-500/50 shadow-sm transition-colors"
                title="接続を再検出"
              >
                <RefreshCw className="w-2.5 h-2.5 group-hover:rotate-180 transition-transform duration-500" />
              </span>
            </button>
            <div>
              <div className="flex flex-wrap items-center gap-2">
                <h2 className="text-xl font-bold text-zinc-100">
                  {device?.connected
                    ? device.nickname || device.device_type
                    : "Kindle端末未接続"}
                </h2>
                {device?.connected && device.is_registered && (
                  <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[11px] font-medium bg-indigo-500/20 text-indigo-300 border border-indigo-500/40">
                    登録済み
                  </span>
                )}
                {device?.connected && !device.is_registered && (
                  <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[11px] font-medium bg-amber-500/20 text-amber-300 border border-amber-500/40 animate-pulse">
                    未登録の端末
                  </span>
                )}
                <span
                  className={`inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full text-xs font-medium ${
                    device?.connected
                      ? "bg-emerald-500/20 text-emerald-300"
                      : "bg-zinc-700/40 text-zinc-400"
                  }`}
                >
                  <span
                    className={`w-1.5 h-1.5 rounded-full ${
                      device?.connected ? "bg-emerald-400 animate-pulse" : "bg-zinc-500"
                    }`}
                  />
                  {device?.connected ? `${device.connection_mode} 接続中` : "待機中"}
                </span>
              </div>
              <p className="text-sm text-zinc-400 mt-1">
                {device?.connected
                  ? `${device.nickname ? `${device.device_type} ・ ` : ""}ID: ${device.device_id} (${device.mount_path})`
                  : "USBケーブルでKindleを接続すると自動検出されます"}
              </p>

              {/* Sync destination path info when connected */}
              {device?.connected && (
                <div className="text-xs text-zinc-400 mt-1.5 flex items-center gap-1.5 font-mono truncate">
                  <Folder className="w-3.5 h-3.5 text-zinc-500 shrink-0" />
                  <span className="text-zinc-300 truncate">
                    同期先: {profile ? `${profile.vault_path}${profile.subfolder ? ` / ${profile.subfolder}` : ""}` : config?.vault_path}
                  </span>
                </div>
              )}

              {/* Per-device settings button (only when connected) */}
              {device?.connected && (
                <div className="mt-2.5 flex items-center gap-3">
                  {!device.is_registered ? (
                    <button
                      onClick={() => setShowSetupModal(true)}
                      className="inline-flex items-center gap-1.5 px-3.5 py-1.5 bg-gradient-to-r from-amber-500 to-orange-500 hover:from-amber-600 hover:to-orange-600 text-white rounded-xl text-xs font-medium shadow-sm transition-all active:scale-95"
                    >
                      <Sparkles className="w-3.5 h-3.5" />
                      端末の初期設定を行う（保存先指定）
                    </button>
                  ) : (
                    <button
                      onClick={() => setShowSetupModal(true)}
                      className="inline-flex items-center gap-1.5 px-3.5 py-1.5 rounded-xl bg-zinc-800 hover:bg-zinc-700 text-zinc-200 text-xs font-medium border border-zinc-700/60 hover:border-indigo-500/50 transition-all active:scale-95 shadow-sm"
                    >
                      <Settings2 className="w-3.5 h-3.5 text-indigo-400" />
                      この端末の同期設定を変更
                    </button>
                  )}
                </div>
              )}

              {device?.status_message && (
                <div className="mt-2.5 px-3 py-1.5 rounded-lg bg-amber-500/10 border border-amber-500/30 text-amber-300 text-xs flex items-center gap-1.5">
                  <span className="font-semibold">案内:</span>
                  <span>{device.status_message}</span>
                </div>
              )}

              {device?.connected && (
                <div className="flex flex-wrap gap-2 mt-3">
                  {device.has_clippings && (
                    <button
                      type="button"
                      onClick={() => onNavigateToContent("clippings")}
                      className="text-xs bg-zinc-800/80 hover:bg-zinc-800 text-zinc-300 hover:text-indigo-300 px-2.5 py-1 rounded-lg border border-zinc-700/50 hover:border-indigo-500/50 flex items-center gap-1.5 transition-all cursor-pointer group"
                      title="ハイライト & 読書メモを開く"
                    >
                      <BookmarkCheck className="w-3.5 h-3.5 text-indigo-400" />
                      <span>My Clippings.txt</span>
                      <ChevronRight className="w-3 h-3 text-zinc-500 group-hover:text-indigo-400 group-hover:translate-x-0.5 transition-all" />
                    </button>
                  )}
                  {device.has_vocab && (
                    <button
                      type="button"
                      onClick={() => onNavigateToContent("vocab")}
                      className="text-xs bg-zinc-800/80 hover:bg-zinc-800 text-zinc-300 hover:text-amber-300 px-2.5 py-1 rounded-lg border border-zinc-700/50 hover:border-amber-500/50 flex items-center gap-1.5 transition-all cursor-pointer group"
                      title="単語帳・語彙ログを開く"
                    >
                      <BookOpen className="w-3.5 h-3.5 text-amber-400" />
                      <span>vocab.db</span>
                      <ChevronRight className="w-3 h-3 text-zinc-500 group-hover:text-amber-400 group-hover:translate-x-0.5 transition-all" />
                    </button>
                  )}
                  {device.has_notebooks && (
                    <button
                      type="button"
                      onClick={() => onNavigateToContent("notebooks")}
                      className="text-xs bg-zinc-800/80 hover:bg-zinc-800 text-zinc-300 hover:text-purple-300 px-2.5 py-1 rounded-lg border border-zinc-700/50 hover:border-purple-500/50 flex items-center gap-1.5 transition-all cursor-pointer group"
                      title="Scribe手書きノート一覧を開く"
                    >
                      <FileText className="w-3.5 h-3.5 text-purple-400" />
                      <span>.notebooks (手書きノート)</span>
                      <ChevronRight className="w-3 h-3 text-zinc-500 group-hover:text-purple-400 group-hover:translate-x-0.5 transition-all" />
                    </button>
                  )}
                </div>
              )}
            </div>
          </div>

          <div className="flex items-center gap-3">
            <button
              disabled={syncing || !device?.connected}
              onClick={() => handleSyncNow()}
              className={`flex items-center gap-2 px-6 py-2.5 rounded-xl font-medium text-sm transition-all shadow-lg ${
                syncing
                  ? "bg-indigo-700/80 text-white/90 cursor-wait pointer-events-none opacity-90 ring-2 ring-indigo-400/30"
                  : device?.connected
                  ? "bg-indigo-600 hover:bg-indigo-500 text-white shadow-indigo-600/20 active:scale-95"
                  : "bg-zinc-800 text-zinc-500 cursor-not-allowed border border-zinc-700/30"
              }`}
            >
              <RefreshCw className={`w-4 h-4 ${syncing ? "animate-spin" : ""}`} />
              {syncing ? `同期中 (${progress?.percentage ?? 0}%)...` : "今すぐ同期"}
            </button>
          </div>
        </div>

        {/* Real-time Sync Progress Card */}
        {(syncing || progress) && (
          <div className="mt-5 p-4 rounded-xl bg-zinc-900/90 border border-indigo-500/40 backdrop-blur-sm space-y-3 shadow-lg">
            <div className="flex items-center justify-between text-sm">
              <div className="flex items-center gap-2.5">
                <RefreshCw className={`w-4 h-4 text-indigo-400 ${syncing ? "animate-spin" : ""}`} />
                <span className="font-semibold text-zinc-100">
                  {progress?.message || "同期処理を実行中..."}
                </span>
              </div>
              <span className="font-bold text-indigo-400 font-mono text-base">
                {progress?.percentage ?? 0}%
              </span>
            </div>

            {/* Visual Animated Progress Bar */}
            <div className="w-full bg-zinc-950 rounded-full h-3 p-0.5 border border-zinc-800">
              <div
                className="bg-gradient-to-r from-indigo-500 via-purple-500 to-emerald-400 h-full rounded-full transition-all duration-300 ease-out shadow-sm shadow-indigo-500/50"
                style={{ width: `${Math.min(100, Math.max(4, progress?.percentage ?? 0))}%` }}
              />
            </div>

            <div className="flex items-center justify-between text-xs text-zinc-400 pt-0.5">
              <span className="truncate max-w-lg">
                {progress?.current_item ? (
                  <>
                    <span className="text-zinc-500">対象: </span>
                    <span className="text-zinc-300 font-mono">{progress.current_item}</span>
                  </>
                ) : (
                  <span>Kindleとストレージの同期を実行しています...</span>
                )}
              </span>
              <span className="text-zinc-500 shrink-0 font-mono ml-2">
                {progress?.step ? `[${progress.step}]` : ""}
              </span>
            </div>
          </div>
        )}

        {/* Message Banner */}
        {syncMessage && (
          <div
            className={`mt-4 p-3 rounded-xl flex items-center gap-2 text-sm ${
              syncMessage.isError
                ? "bg-rose-500/10 border border-rose-500/30 text-rose-300"
                : "bg-emerald-500/10 border border-emerald-500/30 text-emerald-300"
            }`}
          >
            {syncMessage.isError ? (
              <AlertCircle className="w-4 h-4 shrink-0 text-rose-400" />
            ) : (
              <CheckCircle2 className="w-4 h-4 shrink-0 text-emerald-400" />
            )}
            <span>{syncMessage.text}</span>
          </div>
        )}
      </div>

      {/* Sync Stats Cards - Click to navigate to Content View */}
      <div className="space-y-2">
        <div className="flex items-center justify-between px-1">
          <span className="text-xs font-medium text-zinc-400">同期済みコンテンツ</span>
          <span className="text-[11px] text-zinc-500">カードをクリックするとビューアで詳細を閲覧できます</span>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          {/* Highlights & Notes (Clippings) */}
          <div
            onClick={() => onNavigateToContent("clippings", "all")}
            className="bg-zinc-900/60 border border-zinc-800/80 hover:border-indigo-500/60 p-5 rounded-2xl cursor-pointer group shadow-sm hover:shadow-indigo-500/10 transition-all active:scale-[0.99] flex flex-col justify-between"
          >
            <div>
              <div className="flex items-center justify-between text-zinc-400 mb-2">
                <span className="text-xs font-semibold uppercase tracking-wider text-zinc-400 group-hover:text-indigo-300 transition-colors">
                  ハイライト & 読書メモ
                </span>
                <div className="p-2 rounded-xl bg-indigo-500/10 text-indigo-400 group-hover:bg-indigo-500/20 transition-colors">
                  <Highlighter className="w-4 h-4" />
                </div>
              </div>
              <div className="text-2xl font-bold text-zinc-100 group-hover:text-white transition-colors">
                {totalHighlights + totalNotes}
              </div>
              <p className="text-xs text-zinc-500 mt-1">My Clippings 同期済みアイテム</p>

              {/* Sub-breakdown chips */}
              <div className="flex flex-wrap items-center gap-2 mt-3 pt-2.5 border-t border-zinc-800/60">
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    onNavigateToContent("clippings", "highlight");
                  }}
                  className="px-2.5 py-1 rounded-lg text-xs bg-zinc-800/80 hover:bg-indigo-500/20 text-zinc-300 hover:text-indigo-300 border border-zinc-700/50 hover:border-indigo-500/40 transition-all flex items-center gap-1.5"
                  title="ハイライトのみ絞り込んで表示"
                >
                  <Highlighter className="w-3 h-3 text-indigo-400" />
                  <span>ハイライト: <strong className="text-zinc-100">{totalHighlights}</strong></span>
                </button>
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    onNavigateToContent("clippings", "note");
                  }}
                  className="px-2.5 py-1 rounded-lg text-xs bg-zinc-800/80 hover:bg-emerald-500/20 text-zinc-300 hover:text-emerald-300 border border-zinc-700/50 hover:border-emerald-500/40 transition-all flex items-center gap-1.5"
                  title="読書メモのみ絞り込んで表示"
                >
                  <Sparkles className="w-3 h-3 text-emerald-400" />
                  <span>メモ: <strong className="text-zinc-100">{totalNotes}</strong></span>
                </button>
              </div>
            </div>
            <div className="pt-3 mt-3 border-t border-zinc-800/60 flex items-center justify-between text-xs text-indigo-400 font-medium">
              <span>一覧を見る</span>
              <ChevronRight className="w-3.5 h-3.5 group-hover:translate-x-1 transition-transform" />
            </div>
          </div>

          {/* Vocab */}
          <div
            onClick={() => onNavigateToContent("vocab")}
            className="bg-zinc-900/60 border border-zinc-800/80 hover:border-amber-500/60 p-5 rounded-2xl cursor-pointer group shadow-sm hover:shadow-amber-500/10 transition-all active:scale-[0.99] flex flex-col justify-between"
          >
            <div>
              <div className="flex items-center justify-between text-zinc-400 mb-2">
                <span className="text-xs font-semibold uppercase tracking-wider text-zinc-400 group-hover:text-amber-300 transition-colors">
                  単語帳・語彙
                </span>
                <div className="p-2 rounded-xl bg-amber-500/10 text-amber-400 group-hover:bg-amber-500/20 transition-colors">
                  <BookOpen className="w-4 h-4" />
                </div>
              </div>
              <div className="text-2xl font-bold text-zinc-100 group-hover:text-white transition-colors">
                {totalVocab}
              </div>
              <p className="text-xs text-zinc-500 mt-1">vocab.db 単語ログ</p>
            </div>
            <div className="pt-3 mt-3 border-t border-zinc-800/60 flex items-center justify-between text-xs text-amber-400 font-medium">
              <span>単語帳を開く</span>
              <ChevronRight className="w-3.5 h-3.5 group-hover:translate-x-1 transition-transform" />
            </div>
          </div>

          {/* Notebooks */}
          <div
            onClick={() => onNavigateToContent("notebooks")}
            className="bg-zinc-900/60 border border-zinc-800/80 hover:border-purple-500/60 p-5 rounded-2xl cursor-pointer group shadow-sm hover:shadow-purple-500/10 transition-all active:scale-[0.99] flex flex-col justify-between"
          >
            <div>
              <div className="flex items-center justify-between text-zinc-400 mb-2">
                <span className="text-xs font-semibold uppercase tracking-wider text-zinc-400 group-hover:text-purple-300 transition-colors">
                  手書きノート
                </span>
                <div className="p-2 rounded-xl bg-purple-500/10 text-purple-400 group-hover:bg-purple-500/20 transition-colors">
                  <FileText className="w-4 h-4" />
                </div>
              </div>
              <div className="text-2xl font-bold text-zinc-100 group-hover:text-white transition-colors">
                {totalNotebooks}
              </div>
              <p className="text-xs text-zinc-500 mt-1">Scribe 手書きノート</p>
            </div>
            <div className="pt-3 mt-3 border-t border-zinc-800/60 flex items-center justify-between text-xs text-purple-400 font-medium">
              <span>ノート一覧を見る</span>
              <ChevronRight className="w-3.5 h-3.5 group-hover:translate-x-1 transition-transform" />
            </div>
          </div>
        </div>
      </div>

      {/* Sync History Table */}
      <div className="bg-zinc-900/60 border border-zinc-800/80 rounded-2xl p-6">
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-base font-semibold text-zinc-100 flex items-center gap-2">
            <Clock className="w-4 h-4 text-zinc-400" />
            同期ログ履歴
          </h3>
          <span className="text-xs text-zinc-500">直近 {syncHistory.length} 件</span>
        </div>

        {syncHistory.length === 0 ? (
          <div className="text-center py-10 text-zinc-500 text-sm">
            まだ同期履歴がありません。「今すぐ同期」またはKindle接続で同期を実行してください。
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-left text-sm">
              <thead>
                <tr className="border-b border-zinc-800 text-zinc-400 text-xs uppercase">
                  <th className="pb-3 font-medium">日時</th>
                  <th className="pb-3 font-medium">端末</th>
                  <th className="pb-3 font-medium">ハイライト</th>
                  <th className="pb-3 font-medium">メモ</th>
                  <th className="pb-3 font-medium">単語</th>
                  <th className="pb-3 font-medium">ノート</th>
                  <th className="pb-3 font-medium text-right">結果</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-zinc-800/60">
                {syncHistory.map((item, idx) => {
                  const d = new Date(item.timestamp);
                  const formatted = isNaN(d.getTime())
                    ? item.timestamp
                    : d.toLocaleString("ja-JP", {
                        month: "2-digit",
                        day: "2-digit",
                        hour: "2-digit",
                        minute: "2-digit",
                      });

                  return (
                    <tr key={idx} className="hover:bg-zinc-800/30 transition-colors">
                      <td className="py-3 text-zinc-300">{formatted}</td>
                      <td className="py-3 text-zinc-400">{item.device_name}</td>
                      <td className="py-3 text-indigo-400 font-medium">
                        +{item.highlights_added}
                      </td>
                      <td className="py-3 text-emerald-400 font-medium">
                        +{item.notes_added}
                      </td>
                      <td className="py-3 text-amber-400 font-medium">
                        +{item.vocab_added}
                      </td>
                      <td className="py-3 text-purple-400 font-medium">
                        +{item.notebooks_added}
                      </td>
                      <td className="py-3 text-right">
                        <span
                          className={`inline-flex items-center px-2 py-0.5 rounded text-xs ${
                            item.success
                              ? "bg-emerald-500/10 text-emerald-400"
                              : "bg-rose-500/10 text-rose-400"
                          }`}
                        >
                          {item.success ? "成功" : "失敗"}
                        </span>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {device && config && (
        <DeviceSetupModal
          device={device}
          defaultConfig={config}
          isOpen={showSetupModal}
          onClose={() => setShowSetupModal(false)}
          onSaved={(_profile) => {
            setProfile(_profile);
            onRefreshDevice();
            handleSyncNow();
          }}
        />
      )}
    </div>
  );
};
