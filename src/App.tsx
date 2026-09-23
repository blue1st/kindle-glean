import { useState, useEffect } from "react";
import { DeviceInfo, SyncConfig, SyncStats, SyncedCounts } from "./types";
import {
  getConfig,
  detectDevice,
  getSyncHistory,
  getSyncedCounts,
  onDeviceConnected,
  onDeviceDisconnected,
  onSyncCompleted,
  onTriggerSync,
  syncNow,
} from "./api";
import { Dashboard } from "./components/Dashboard";
import { Settings } from "./components/Settings";
import { ContentViewer, ViewerTab } from "./components/ContentViewer";
import { AboutModal } from "./components/AboutModal";
import { BookOpen, Layers, Home, Settings as SettingsIcon } from "lucide-react";
import packageJson from "../package.json";
import {
  GitHubRelease,
  fetchLatestRelease,
  isNewVersionAvailable,
} from "./utils/versionCheck";
import "./App.css";

type ActiveView =
  | { type: "home" }
  | { type: "content"; tab: ViewerTab; filterType?: "all" | "highlight" | "note" }
  | { type: "settings" };

export function App() {
  const [activeView, setActiveView] = useState<ActiveView>({ type: "home" });
  const [contentRefreshKey, setContentRefreshKey] = useState(0);
  const [device, setDevice] = useState<DeviceInfo | null>(null);
  const [config, setConfig] = useState<SyncConfig | null>(null);
  const [syncHistory, setSyncHistory] = useState<SyncStats[]>([]);
  const [syncedCounts, setSyncedCounts] = useState<SyncedCounts | null>(null);
  const [loading, setLoading] = useState(true);
  const [isAboutOpen, setIsAboutOpen] = useState(false);
  const [latestRelease, setLatestRelease] = useState<GitHubRelease | null>(null);

  const refreshHistoryAndCounts = () => {
    getSyncHistory().then(setSyncHistory);
    getSyncedCounts().then(setSyncedCounts);
    setContentRefreshKey((k) => k + 1);
  };

  const loadAll = async () => {
    try {
      const [cfg, dev, hist, counts] = await Promise.all([
        getConfig(),
        detectDevice(),
        getSyncHistory(),
        getSyncedCounts(),
      ]);
      setConfig(cfg);
      setDevice(dev);
      setSyncHistory(hist);
      setSyncedCounts(counts);
    } catch (err) {
      console.error("Initialization error:", err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadAll();
    fetchLatestRelease().then((rel) => {
      if (rel) setLatestRelease(rel);
    });

    const unlistenConnected = onDeviceConnected((dev) => {
      setDevice(dev);
    });
    const unlistenDisconnected = onDeviceDisconnected(() => {
      setDevice(null);
    });
    const unlistenSyncCompleted = onSyncCompleted(() => {
      refreshHistoryAndCounts();
    });
    const unlistenTrigger = onTriggerSync(async () => {
      try {
        await syncNow();
        refreshHistoryAndCounts();
      } catch (err) {
        console.error("Tray sync failed:", err);
      }
    });

    return () => {
      unlistenConnected.then((f) => f());
      unlistenDisconnected.then((f) => f());
      unlistenSyncCompleted.then((f) => f());
      unlistenTrigger.then((f) => f());
    };
  }, []);

  const refreshDevice = async () => {
    const dev = await detectDevice();
    setDevice(dev);
  };

  if (loading || !config) {
    return (
      <div className="flex h-screen items-center justify-center bg-zinc-950 text-zinc-400">
        <div className="flex items-center gap-3">
          <Layers className="w-6 h-6 animate-pulse text-indigo-500" />
          <span className="text-sm font-medium">Kindle Glean を起動中...</span>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-zinc-950 text-zinc-100 flex flex-col font-sans">
      {/* App Navigation Bar */}
      <header className="border-b border-zinc-800/80 bg-zinc-900/40 backdrop-blur-md sticky top-0 z-50">
        <div className="max-w-6xl mx-auto px-6 h-16 flex items-center justify-between">
          {/* Logo & Brand: Clickable to return to Home */}
          <div
            onClick={() => setActiveView({ type: "home" })}
            className="flex items-center gap-3 cursor-pointer group select-none transition-opacity hover:opacity-95"
            title="ホーム画面に戻る"
          >
            <div className="p-2 rounded-xl bg-gradient-to-tr from-indigo-600 to-indigo-500 text-white shadow-md shadow-indigo-600/30 group-hover:scale-105 transition-transform">
              <BookOpen className="w-5 h-5" />
            </div>
            <div>
              <div className="flex items-center gap-2">
                <h1 className="text-base font-bold tracking-tight text-zinc-100 group-hover:text-indigo-300 transition-colors">
                  Kindle Glean
                </h1>
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    setIsAboutOpen(true);
                  }}
                  className="flex items-center gap-1.5 text-[10px] font-mono font-medium px-2 py-0.5 rounded bg-zinc-800/90 hover:bg-zinc-700 text-zinc-400 hover:text-zinc-200 border border-zinc-700/60 hover:border-indigo-500/50 transition-all cursor-pointer shadow-sm"
                  title="アプリ情報・更新確認"
                >
                  <span>v{packageJson.version}</span>
                  {latestRelease && isNewVersionAvailable(packageJson.version, latestRelease.tag_name) && (
                    <span className="flex h-1.5 w-1.5 relative">
                      <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-amber-400 opacity-75"></span>
                      <span className="relative inline-flex rounded-full h-1.5 w-1.5 bg-amber-500"></span>
                    </span>
                  )}
                </button>
                {activeView.type !== "home" && (
                  <span className="text-[11px] px-2.5 py-0.5 rounded-full bg-zinc-800 text-zinc-300 border border-zinc-700/60 font-medium">
                    {activeView.type === "settings"
                      ? "全体設定"
                      : activeView.tab === "clippings"
                      ? activeView.filterType === "highlight"
                        ? "ハイライト一覧"
                        : activeView.filterType === "note"
                        ? "読書メモ一覧"
                        : "ハイライト & メモ"
                      : activeView.tab === "notebooks"
                      ? "手書きノート"
                      : "単語帳・語彙"}
                  </span>
                )}
              </div>
              <div className="flex items-center gap-1.5 text-xs text-zinc-400">
                <span className="inline-block w-1.5 h-1.5 rounded-full bg-emerald-500" />
                バックグラウンド常駐中
              </div>
            </div>
          </div>

          {/* Right Header Navigation & Actions */}
          <div className="flex items-center gap-2">
            {activeView.type !== "home" && (
              <button
                onClick={() => setActiveView({ type: "home" })}
                className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl bg-zinc-800 hover:bg-zinc-700 text-zinc-200 text-xs font-medium border border-zinc-700/60 transition-colors shadow-sm"
              >
                <Home className="w-3.5 h-3.5 text-indigo-400" />
                ホームに戻る
              </button>
            )}

            <button
              onClick={() =>
                setActiveView((prev) =>
                  prev.type === "settings" ? { type: "home" } : { type: "settings" }
                )
              }
              title="アプリ全体設定"
              className={`flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-xs font-medium border transition-all ${
                activeView.type === "settings"
                  ? "bg-zinc-800 text-indigo-300 border-indigo-500/50 shadow-sm"
                  : "bg-zinc-900/80 hover:bg-zinc-800 text-zinc-400 hover:text-zinc-200 border-zinc-800"
              }`}
            >
              <SettingsIcon className="w-3.5 h-3.5" />
              <span className="hidden sm:inline">設定</span>
            </button>
          </div>
        </div>
      </header>

      {/* Main Content Area */}
      <main className="flex-1 max-w-6xl w-full mx-auto p-6 md:p-8">
        {activeView.type === "home" && (
          <Dashboard
            device={device}
            onRefreshDevice={refreshDevice}
            syncHistory={syncHistory}
            counts={syncedCounts}
            onSyncFinished={refreshHistoryAndCounts}
            config={config}
            onNavigateToContent={(tab, filterType) => setActiveView({ type: "content", tab, filterType })}
          />
        )}

        <div className={activeView.type === "content" ? "block" : "hidden"}>
          <ContentViewer
            config={config}
            initialTab={activeView.type === "content" ? activeView.tab : undefined}
            initialClipType={activeView.type === "content" ? activeView.filterType : undefined}
            refreshTrigger={contentRefreshKey}
            onDataChanged={refreshHistoryAndCounts}
          />
        </div>

        {activeView.type === "settings" && (
          <Settings
            config={config}
            onConfigUpdated={(newCfg) => setConfig(newCfg)}
            onOpenAbout={() => setIsAboutOpen(true)}
          />
        )}
      </main>

      <AboutModal
        isOpen={isAboutOpen}
        onClose={() => setIsAboutOpen(false)}
        latestRelease={latestRelease}
        onUpdateLatestRelease={setLatestRelease}
      />
    </div>
  );
}

export default App;
