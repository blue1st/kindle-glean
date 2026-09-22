import React, { useState, useEffect } from "react";
import {
  BookOpen,
  ExternalLink,
  RefreshCw,
  CheckCircle2,
  AlertCircle,
  Sparkles,
  X,
  Download,
  Info,
  GitBranch,
} from "lucide-react";
import packageJson from "../../package.json";
import { openExternalUrl } from "../api";
import {
  GitHubRelease,
  fetchLatestRelease,
  isNewVersionAvailable,
} from "../utils/versionCheck";

interface AboutModalProps {
  isOpen: boolean;
  onClose: () => void;
  latestRelease?: GitHubRelease | null;
  onUpdateLatestRelease?: (release: GitHubRelease | null) => void;
}

const REPO_URL = "https://github.com/blue1st/kindle-glean";
const RELEASES_URL = "https://github.com/blue1st/kindle-glean/releases";
const ISSUES_URL = "https://github.com/blue1st/kindle-glean/issues";

export const AboutModal: React.FC<AboutModalProps> = ({
  isOpen,
  onClose,
  latestRelease: initialLatestRelease,
  onUpdateLatestRelease,
}) => {
  const [checking, setChecking] = useState(false);
  const [release, setRelease] = useState<GitHubRelease | null>(
    initialLatestRelease ?? null
  );
  const [error, setError] = useState<string | null>(null);
  const [lastChecked, setLastChecked] = useState<Date | null>(null);

  const checkUpdate = async (force = false) => {
    setChecking(true);
    setError(null);
    try {
      const data = await fetchLatestRelease(force);
      setRelease(data);
      setLastChecked(new Date());
      if (onUpdateLatestRelease) {
        onUpdateLatestRelease(data);
      }
      if (!data) {
        setError("リリース情報を取得できませんでした");
      }
    } catch {
      setError("更新確認中にエラーが発生しました");
    } finally {
      setChecking(false);
    }
  };

  useEffect(() => {
    if (isOpen) {
      if (!release) {
        checkUpdate(false);
      }
    }
  }, [isOpen]);

  // Escキーで閉じる
  useEffect(() => {
    if (!isOpen) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  const currentVersion = packageJson.version;
  const hasUpdate = release
    ? isNewVersionAvailable(currentVersion, release.tag_name)
    : false;

  const handleOpenUrl = (url: string) => {
    openExternalUrl(url);
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/80 backdrop-blur-sm p-4 overflow-y-auto animate-in fade-in duration-200"
      onClick={onClose}
    >
      <div
        className="bg-zinc-900 rounded-2xl shadow-2xl border border-zinc-800 max-w-lg w-full overflow-hidden animate-in zoom-in-95 duration-200 flex flex-col"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="bg-gradient-to-r from-indigo-950/60 via-zinc-900 to-zinc-900 px-6 py-5 border-b border-zinc-800 flex items-start justify-between">
          <div className="flex items-center gap-3">
            <div className="p-2.5 bg-gradient-to-tr from-indigo-600 to-indigo-500 text-white rounded-xl shadow-lg shadow-indigo-600/30">
              <BookOpen className="w-5 h-5" />
            </div>
            <div>
              <div className="flex items-center gap-2">
                <h3 className="text-base font-bold text-zinc-100">
                  Kindle Glean
                </h3>
                <span className="text-[10px] font-mono font-medium px-2 py-0.5 rounded bg-zinc-800 text-zinc-300 border border-zinc-700">
                  v{currentVersion}
                </span>
              </div>
              <p className="text-zinc-400 text-xs mt-0.5">
                Kindle ローカル同期・コンテンツビューア
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="text-zinc-400 hover:text-zinc-200 p-1.5 rounded-lg hover:bg-zinc-800 transition-colors"
            title="閉じる"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Modal Body */}
        <div className="p-6 space-y-5 max-h-[75vh] overflow-y-auto">
          {/* App Description */}
          <div className="text-xs leading-relaxed text-zinc-300 bg-zinc-800/40 p-3.5 rounded-xl border border-zinc-800">
            Kindle Scribeの手書きノート、読書ハイライト・メモ（My Clippings）、単語帳（vocab.db）をPCローカルに自動同期・蓄積し、美しく閲覧・エクスポートできるデスクトップアプリケーションです。
          </div>

          {/* Update Status Section */}
          <div className="p-4 rounded-xl bg-zinc-950/60 border border-zinc-800/80 space-y-3">
            <div className="flex items-center justify-between">
              <span className="text-xs font-semibold text-zinc-300 flex items-center gap-1.5">
                <Sparkles className="w-3.5 h-3.5 text-indigo-400" />
                アップデート状況
              </span>
              <button
                onClick={() => checkUpdate(true)}
                disabled={checking}
                className="flex items-center gap-1 text-[11px] text-zinc-400 hover:text-zinc-200 disabled:opacity-50 transition-colors px-2 py-1 rounded-md hover:bg-zinc-800"
                title="最新の更新を手動で確認"
              >
                <RefreshCw
                  className={`w-3 h-3 ${checking ? "animate-spin text-indigo-400" : ""}`}
                />
                <span>{checking ? "確認中..." : "更新を確認"}</span>
              </button>
            </div>

            {checking ? (
              <div className="flex items-center gap-2.5 py-2 text-xs text-zinc-400">
                <div className="w-4 h-4 border-2 border-indigo-500/30 border-t-indigo-500 rounded-full animate-spin" />
                <span>GitHub Releases から最新バージョンを確認しています...</span>
              </div>
            ) : hasUpdate && release ? (
              <div className="p-3 bg-amber-500/10 border border-amber-500/30 rounded-lg space-y-2.5">
                <div className="flex items-start justify-between gap-2">
                  <div className="flex items-center gap-2">
                    <span className="flex h-2 w-2 relative">
                      <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-amber-400 opacity-75"></span>
                      <span className="relative inline-flex rounded-full h-2 w-2 bg-amber-500"></span>
                    </span>
                    <span className="text-xs font-semibold text-amber-300">
                      新しいバージョン {release.tag_name} が利用可能です！
                    </span>
                  </div>
                  {release.published_at && (
                    <span className="text-[10px] text-zinc-400 font-mono">
                      {new Date(release.published_at).toLocaleDateString()}
                    </span>
                  )}
                </div>

                {release.body && (
                  <div className="text-[11px] text-zinc-300 bg-black/40 p-2.5 rounded border border-zinc-800/80 max-h-28 overflow-y-auto whitespace-pre-wrap font-mono leading-normal">
                    {release.body}
                  </div>
                )}

                <div className="pt-1 flex items-center gap-2">
                  <button
                    onClick={() => handleOpenUrl(release.html_url || RELEASES_URL)}
                    className="flex-1 flex items-center justify-center gap-1.5 px-3 py-2 rounded-lg bg-amber-600 hover:bg-amber-500 text-zinc-950 font-semibold text-xs transition-colors shadow-sm"
                  >
                    <Download className="w-3.5 h-3.5" />
                    ダウンロードページを開く
                  </button>
                </div>
              </div>
            ) : release ? (
              <div className="flex items-center justify-between py-1 text-xs">
                <div className="flex items-center gap-2 text-emerald-400">
                  <CheckCircle2 className="w-4 h-4" />
                  <span>最新バージョンを使用しています ({release.tag_name})</span>
                </div>
                {lastChecked && (
                  <span className="text-[10px] text-zinc-500">
                    確認済: {lastChecked.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
                  </span>
                )}
              </div>
            ) : error ? (
              <div className="space-y-1.5 py-1">
                <div className="flex items-center gap-2 text-xs text-amber-400/90">
                  <AlertCircle className="w-4 h-4 shrink-0" />
                  <span>{error}</span>
                </div>
                <div className="text-[11px] text-zinc-400 pl-6">
                  ネットワーク接続を確認するか、
                  <button
                    onClick={() => handleOpenUrl(RELEASES_URL)}
                    className="text-indigo-400 hover:underline inline-flex items-center gap-0.5 ml-1"
                  >
                    直接 Releases ページ
                    <ExternalLink className="w-2.5 h-2.5" />
                  </button>
                  をご確認ください。
                </div>
              </div>
            ) : null}
          </div>

          {/* Useful Links */}
          <div className="space-y-2">
            <span className="text-xs font-semibold text-zinc-400">リンク</span>
            <div className="grid grid-cols-1 gap-2">
              <button
                onClick={() => handleOpenUrl(REPO_URL)}
                className="w-full flex items-center justify-between px-3.5 py-2.5 rounded-xl bg-zinc-800/60 hover:bg-zinc-800 border border-zinc-700/60 text-zinc-200 transition-all text-xs group text-left"
              >
                <div className="flex items-center gap-2.5">
                  <GitBranch className="w-4 h-4 text-indigo-400 group-hover:scale-110 transition-transform" />
                  <div>
                    <div className="font-medium text-zinc-200 group-hover:text-indigo-300 transition-colors">
                      GitHub リポジトリ
                    </div>
                    <div className="text-[10px] text-zinc-400">
                      ソースコード・README・開発ドキュメント
                    </div>
                  </div>
                </div>
                <ExternalLink className="w-3.5 h-3.5 text-zinc-400 group-hover:text-zinc-200 transition-colors" />
              </button>

              <button
                onClick={() => handleOpenUrl(RELEASES_URL)}
                className="w-full flex items-center justify-between px-3.5 py-2.5 rounded-xl bg-zinc-800/60 hover:bg-zinc-800 border border-zinc-700/60 text-zinc-200 transition-all text-xs group text-left"
              >
                <div className="flex items-center gap-2.5">
                  <Download className="w-4 h-4 text-emerald-400 group-hover:scale-110 transition-transform" />
                  <div>
                    <div className="font-medium text-zinc-200 group-hover:text-emerald-300 transition-colors">
                      リリース一覧 (Releases)
                    </div>
                    <div className="text-[10px] text-zinc-400">
                      最新ビルド・各OS向けインストーラー・更新履歴
                    </div>
                  </div>
                </div>
                <ExternalLink className="w-3.5 h-3.5 text-zinc-400 group-hover:text-zinc-200 transition-colors" />
              </button>

              <button
                onClick={() => handleOpenUrl(ISSUES_URL)}
                className="w-full flex items-center justify-between px-3.5 py-2.5 rounded-xl bg-zinc-800/60 hover:bg-zinc-800 border border-zinc-700/60 text-zinc-200 transition-all text-xs group text-left"
              >
                <div className="flex items-center gap-2.5">
                  <Info className="w-4 h-4 text-amber-400 group-hover:scale-110 transition-transform" />
                  <div>
                    <div className="font-medium text-zinc-200 group-hover:text-amber-300 transition-colors">
                      Issues・フィードバック
                    </div>
                    <div className="text-[10px] text-zinc-400">
                      不具合報告や機能リクエストの送信
                    </div>
                  </div>
                </div>
                <ExternalLink className="w-3.5 h-3.5 text-zinc-400 group-hover:text-zinc-200 transition-colors" />
              </button>
            </div>
          </div>
        </div>

        {/* Footer */}
        <div className="bg-zinc-950/80 px-6 py-3.5 border-t border-zinc-800 flex items-center justify-between text-xs text-zinc-500">
          <span>MIT License © 2026 blue1st</span>
          <button
            onClick={onClose}
            className="px-3.5 py-1.5 rounded-lg bg-zinc-800 hover:bg-zinc-700 text-zinc-300 text-xs font-medium transition-colors"
          >
            閉じる
          </button>
        </div>
      </div>
    </div>
  );
};
