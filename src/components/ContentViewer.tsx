import React, { useState, useEffect } from "react";
import {
  Clipping,
  NotebookSummary,
  VocabLookup,
  SyncConfig,
} from "../types";
import {
  getSyncedClippings,
  getSyncedNotebooks,
  getSyncedVocab,
  getNotebookPages,
  readImageBase64,
  previewClippings,
  openFolder,
} from "../api";
import {
  Highlighter,
  BookOpen,
  PenTool,
  Languages,
  Search,
  RefreshCw,
  Folder,
  FolderOpen,
  ChevronLeft,
  ChevronRight,
  X,
  Sparkles,
  Bookmark,
  FileSearch,
  Calendar,
  Layers,
  FileDown,
  FolderDown,
  LayoutGrid,
  Image as ImageIcon,
  Check,
  Loader2,
  AlertCircle,
  Download,
} from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  exportNotebookToPdf,
  exportNotebookImagesToDirectory,
} from "../utils/notebookExport";

export type ViewerTab = "clippings" | "notebooks" | "vocab";

interface Props {
  config: SyncConfig;
  initialTab?: ViewerTab;
  initialClipType?: "all" | "highlight" | "note";
}

export const ContentViewer: React.FC<Props> = ({
  config,
  initialTab = "clippings",
  initialClipType = "all",
}) => {
  const [currentTab, setCurrentTab] = useState<ViewerTab>(initialTab);

  useEffect(() => {
    if (initialTab) {
      setCurrentTab(initialTab);
    }
  }, [initialTab]);

  // Data states
  const [clippings, setClippings] = useState<Clipping[]>([]);
  const [notebooks, setNotebooks] = useState<NotebookSummary[]>([]);
  const [vocab, setVocab] = useState<VocabLookup[]>([]);
  const [loading, setLoading] = useState(false);

  // Clippings filters & manual file load
  const [clipSearch, setClipSearch] = useState("");
  const [clipSelectedBook, setClipSelectedBook] = useState("all");
  const [clipSelectedType, setClipSelectedType] = useState<string>(initialClipType);
  const [manualClippingsLoaded, setManualClippingsLoaded] = useState(false);

  useEffect(() => {
    if (initialClipType) {
      setClipSelectedType(initialClipType);
    }
  }, [initialClipType]);

  // Notebooks states
  const [nbSearch, setNbSearch] = useState("");
  const [selectedNotebook, setSelectedNotebook] = useState<NotebookSummary | null>(null);
  const [notebookPages, setNotebookPages] = useState<string[]>([]);
  const [activePageIndex, setActivePageIndex] = useState(0);
  const [activePageDataUrl, setActivePageDataUrl] = useState<string | null>(null);
  const [loadingPages, setLoadingPages] = useState(false);
  const [nbViewMode, setNbViewMode] = useState<"single" | "grid">("single");
  const [selectedPageIndices, setSelectedPageIndices] = useState<Set<number>>(new Set());
  const [isExporting, setIsExporting] = useState(false);
  const [exportProgress, setExportProgress] = useState<{ current: number; total: number } | null>(null);
  const [exportNotification, setExportNotification] = useState<{
    type: "success" | "error";
    message: string;
  } | null>(null);

  // Vocab filters
  const [vocabSearch, setVocabSearch] = useState("");
  const [vocabSelectedBook, setVocabSelectedBook] = useState("all");

  const showExportNotification = (type: "success" | "error", message: string) => {
    setExportNotification({ type, message });
    setTimeout(() => {
      setExportNotification(null);
    }, 5000);
  };

  const loadAllData = async () => {
    setLoading(true);
    try {
      const [clips, nbs, vcb] = await Promise.all([
        getSyncedClippings(),
        getSyncedNotebooks(),
        getSyncedVocab(),
      ]);
      setClippings(clips);
      setNotebooks(nbs);
      setVocab(vcb);
      setManualClippingsLoaded(false);
    } catch (err) {
      console.error("Failed to load synced contents:", err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadAllData();
  }, []);

  // Handle manual clippings file open
  const handleOpenClippingsFile = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "Kindle Clippings", extensions: ["txt"] }],
        title: "My Clippings.txt を開いてプレビュー",
      });

      if (selected && typeof selected === "string") {
        setLoading(true);
        const data = await previewClippings(selected);
        setClippings(data);
        setManualClippingsLoaded(true);
        setLoading(false);
      }
    } catch (err) {
      console.error(err);
      setLoading(false);
    }
  };

  // Open notebook viewer modal
  const handleSelectNotebook = async (nb: NotebookSummary) => {
    setSelectedNotebook(nb);
    setActivePageIndex(0);
    setActivePageDataUrl(null);
    setNbViewMode("single");
    setSelectedPageIndices(new Set());
    setExportNotification(null);
    setLoadingPages(true);

    try {
      const pages = await getNotebookPages(nb.title);
      setNotebookPages(pages);
      if (pages.length > 0) {
        const b64 = await readImageBase64(pages[0]);
        setActivePageDataUrl(b64);
      }
    } catch (err) {
      console.error("Failed to load notebook pages:", err);
    } finally {
      setLoadingPages(false);
    }
  };

  const handlePageChange = async (index: number) => {
    if (index < 0 || index >= notebookPages.length) return;
    setActivePageIndex(index);
    setActivePageDataUrl(null);
    try {
      const b64 = await readImageBase64(notebookPages[index]);
      setActivePageDataUrl(b64);
    } catch (err) {
      console.error("Failed to read page image:", err);
    }
  };

  // PDF Export
  const handleExportPdf = async () => {
    if (!selectedNotebook || notebookPages.length === 0) return;
    setIsExporting(true);
    setExportProgress({ current: 0, total: notebookPages.length });
    try {
      const res = await exportNotebookToPdf(
        notebookPages,
        selectedNotebook.display_title || selectedNotebook.title,
        (current, total) => setExportProgress({ current, total })
      );
      if (res.success && res.filePath) {
        const fileName = res.filePath.split(/[/\\]/).pop();
        showExportNotification("success", `PDFを保存しました: ${fileName}`);
      }
    } catch (err: any) {
      console.error("PDF export failed:", err);
      showExportNotification("error", `PDFの出力に失敗しました: ${err?.message || err}`);
    } finally {
      setIsExporting(false);
      setExportProgress(null);
    }
  };

  // All Images Export
  const handleExportAllImages = async () => {
    if (!selectedNotebook || notebookPages.length === 0) return;
    setIsExporting(true);
    try {
      const res = await exportNotebookImagesToDirectory(
        notebookPages,
        selectedNotebook.display_title || selectedNotebook.title
      );
      if (res.success) {
        showExportNotification("success", `全 ${res.count} ページの画像をフォルダに保存しました`);
      }
    } catch (err: any) {
      console.error("Images export failed:", err);
      showExportNotification("error", `画像の保存に失敗しました: ${err?.message || err}`);
    } finally {
      setIsExporting(false);
    }
  };

  // Selected Images Export
  const handleExportSelectedImages = async () => {
    if (!selectedNotebook || selectedPageIndices.size === 0) return;
    setIsExporting(true);
    try {
      const sortedIndices = Array.from(selectedPageIndices).sort((a, b) => a - b);
      const res = await exportNotebookImagesToDirectory(
        notebookPages,
        selectedNotebook.display_title || selectedNotebook.title,
        sortedIndices
      );
      if (res.success) {
        showExportNotification(
          "success",
          `選択した ${res.count} ページの画像をフォルダに保存しました`
        );
      }
    } catch (err: any) {
      console.error("Selected images export failed:", err);
      showExportNotification("error", `画像の保存に失敗しました: ${err?.message || err}`);
    } finally {
      setIsExporting(false);
    }
  };

  // Current Single Image Export
  const handleExportCurrentImage = async () => {
    if (!selectedNotebook || notebookPages.length === 0) return;
    setIsExporting(true);
    try {
      const res = await exportNotebookImagesToDirectory(
        notebookPages,
        selectedNotebook.display_title || selectedNotebook.title,
        [activePageIndex]
      );
      if (res.success) {
        showExportNotification("success", `ページ ${activePageIndex + 1} の画像をフォルダに保存しました`);
      }
    } catch (err: any) {
      console.error("Single page export failed:", err);
      showExportNotification("error", `画像の保存に失敗しました: ${err?.message || err}`);
    } finally {
      setIsExporting(false);
    }
  };

  const handleToggleSelectPage = (index: number) => {
    setSelectedPageIndices((prev) => {
      const next = new Set(prev);
      if (next.has(index)) {
        next.delete(index);
      } else {
        next.add(index);
      }
      return next;
    });
  };

  const handleSelectAllPages = () => {
    const all = new Set<number>();
    for (let i = 0; i < notebookPages.length; i++) {
      all.add(i);
    }
    setSelectedPageIndices(all);
  };

  const handleDeselectAllPages = () => {
    setSelectedPageIndices(new Set());
  };

  // Filters for Clippings
  const clippingBookTitles = Array.from(new Set(clippings.map((c) => c.book_title)));
  const filteredClippings = clippings.filter((c) => {
    const q = clipSearch.toLowerCase();
    const matchesQ =
      c.content.toLowerCase().includes(q) ||
      c.book_title.toLowerCase().includes(q) ||
      (c.author && c.author.toLowerCase().includes(q));
    const matchesBook = clipSelectedBook === "all" || c.book_title === clipSelectedBook;
    const matchesType = clipSelectedType === "all" || c.clipping_type.toLowerCase() === clipSelectedType;
    return matchesQ && matchesBook && matchesType;
  });

  // Filters for Notebooks
  const filteredNotebooks = notebooks.filter((nb) => {
    const q = nbSearch.toLowerCase();
    return (
      nb.display_title.toLowerCase().includes(q) ||
      nb.title.toLowerCase().includes(q)
    );
  });

  // Filters for Vocab
  const vocabBookTitles = Array.from(
    new Set(vocab.map((v) => v.book_title).filter((t): t is string => !!t))
  );
  const filteredVocab = vocab.filter((v) => {
    const q = vocabSearch.toLowerCase();
    const matchesQ =
      v.word.toLowerCase().includes(q) ||
      (v.stem && v.stem.toLowerCase().includes(q)) ||
      (v.usage && v.usage.toLowerCase().includes(q)) ||
      (v.book_title && v.book_title.toLowerCase().includes(q));
    const matchesBook = vocabSelectedBook === "all" || v.book_title === vocabSelectedBook;
    return matchesQ && matchesBook;
  });

  const getSubfolderPath = (folderName: string) => {
    const base = config.subfolder.trim().length > 0 ? `${config.subfolder}/${folderName}` : folderName;
    return `${config.vault_path}/${base}`;
  };

  return (
    <div className="space-y-6">
      {/* Content View Header */}
      <div className="flex items-center justify-between pb-1">
        <div className="flex items-center gap-3">
          {currentTab === "clippings" && (
            <>
              <div className="p-2.5 rounded-xl bg-indigo-500/10 text-indigo-400 border border-indigo-500/20">
                <Highlighter className="w-5 h-5" />
              </div>
              <div>
                <h2 className="text-lg font-bold text-zinc-100 flex items-center gap-2">
                  書籍ハイライト & メモ
                  <span className="text-xs font-normal text-zinc-400 px-2.5 py-0.5 rounded-full bg-zinc-900 border border-zinc-800 font-mono">
                    {clippings.length} 件
                  </span>
                </h2>
                <p className="text-xs text-zinc-500">Kindleで保存したハイライトと読書メモ</p>
              </div>
            </>
          )}

          {currentTab === "notebooks" && (
            <>
              <div className="p-2.5 rounded-xl bg-purple-500/10 text-purple-400 border border-purple-500/20">
                <PenTool className="w-5 h-5" />
              </div>
              <div>
                <h2 className="text-lg font-bold text-zinc-100 flex items-center gap-2">
                  手書きノート
                  <span className="text-xs font-normal text-zinc-400 px-2.5 py-0.5 rounded-full bg-zinc-900 border border-zinc-800 font-mono">
                    {notebooks.length} 冊
                  </span>
                </h2>
                <p className="text-xs text-zinc-500">Kindle Scribe の手書きノート（ベクター画像）</p>
              </div>
            </>
          )}

          {currentTab === "vocab" && (
            <>
              <div className="p-2.5 rounded-xl bg-amber-500/10 text-amber-400 border border-amber-500/20">
                <Languages className="w-5 h-5" />
              </div>
              <div>
                <h2 className="text-lg font-bold text-zinc-100 flex items-center gap-2">
                  単語帳・語彙ログ
                  <span className="text-xs font-normal text-zinc-400 px-2.5 py-0.5 rounded-full bg-zinc-900 border border-zinc-800 font-mono">
                    {vocab.length} 語
                  </span>
                </h2>
                <p className="text-xs text-zinc-500">読書中に辞書検索した英単語と例文ログ</p>
              </div>
            </>
          )}
        </div>

        <button
          onClick={loadAllData}
          disabled={loading}
          title="最新データを再読み込み"
          className="p-2 rounded-xl bg-zinc-900/80 hover:bg-zinc-800 text-zinc-400 hover:text-zinc-200 border border-zinc-800 transition-colors"
        >
          <RefreshCw className={`w-4 h-4 ${loading ? "animate-spin text-indigo-400" : ""}`} />
        </button>
      </div>

      {/* ======================================================== */}
      {/* TAB 1: CLIPPINGS & HIGHLIGHTS */}
      {/* ======================================================== */}
      {currentTab === "clippings" && (
        <div className="space-y-4">
          {/* Controls & Search */}
          <div className="flex flex-col md:flex-row gap-3">
            <div className="relative flex-1">
              <Search className="w-4 h-4 text-zinc-500 absolute left-3.5 top-1/2 -translate-y-1/2" />
              <input
                type="text"
                placeholder="ハイライト・メモ・書籍名・著者で検索..."
                value={clipSearch}
                onChange={(e) => setClipSearch(e.target.value)}
                className="w-full pl-10 pr-4 py-2 bg-zinc-900/80 border border-zinc-700/60 rounded-xl text-sm text-zinc-200 focus:outline-none focus:border-indigo-500"
              />
            </div>

            <select
              value={clipSelectedBook}
              onChange={(e) => setClipSelectedBook(e.target.value)}
              className="px-3.5 py-2 bg-zinc-900/80 border border-zinc-700/60 rounded-xl text-sm text-zinc-200 focus:outline-none focus:border-indigo-500 max-w-xs truncate"
            >
              <option value="all">すべての書籍 ({clippingBookTitles.length})</option>
              {clippingBookTitles.map((title, idx) => (
                <option key={idx} value={title}>
                  {title}
                </option>
              ))}
            </select>

            <select
              value={clipSelectedType}
              onChange={(e) => setClipSelectedType(e.target.value)}
              className="px-3.5 py-2 bg-zinc-900/80 border border-zinc-700/60 rounded-xl text-sm text-zinc-200 focus:outline-none focus:border-indigo-500"
            >
              <option value="all">すべての種類</option>
              <option value="highlight">ハイライト</option>
              <option value="note">メモ</option>
              <option value="bookmark">しおり</option>
            </select>

            <button
              onClick={handleOpenClippingsFile}
              title="別の My Clippings.txt を直接開く"
              className="flex items-center gap-1.5 px-3 py-2 rounded-xl bg-zinc-800/80 hover:bg-zinc-700 text-zinc-300 text-xs font-medium border border-zinc-700/50 transition-colors shrink-0"
            >
              <FileSearch className="w-4 h-4 text-zinc-400" />
              ファイルから開く
            </button>
          </div>

          <div className="flex items-center justify-between text-xs text-zinc-400 px-1">
            <span>
              {manualClippingsLoaded ? "外部ファイル読み込み: " : "同期済みデータ: "}
              <strong className="text-zinc-200">{filteredClippings.length}</strong> 件表示 / 全 {clippings.length} 件
            </span>
            <button
              onClick={() => openFolder(getSubfolderPath("Books"))}
              className="text-indigo-400 hover:text-indigo-300 flex items-center gap-1 hover:underline"
            >
              <Folder className="w-3.5 h-3.5" />
              Booksフォルダを開く
            </button>
          </div>

          {/* Clippings List */}
          {filteredClippings.length === 0 ? (
            <div className="bg-zinc-900/40 border border-zinc-800/80 rounded-2xl p-12 text-center text-zinc-500">
              <BookOpen className="w-8 h-8 mx-auto mb-3 opacity-40 text-zinc-400" />
              <p className="text-sm font-medium text-zinc-400">表示できるハイライト・メモがありません</p>
              <p className="text-xs text-zinc-500 mt-1">Kindleを接続して同期するか、「ファイルから開く」から txt ファイルを選択してください</p>
            </div>
          ) : (
            <div className="space-y-3">
              {filteredClippings.map((item) => (
                <div
                  key={item.id}
                  className="bg-zinc-900/60 border border-zinc-800/80 rounded-2xl p-5 hover:border-zinc-700/80 transition-all space-y-2.5"
                >
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      <span
                        className={`inline-flex items-center gap-1 px-2.5 py-0.5 rounded-md text-xs font-medium ${
                          item.clipping_type === "Highlight"
                            ? "bg-indigo-500/10 text-indigo-400 border border-indigo-500/20"
                            : item.clipping_type === "Note"
                            ? "bg-emerald-500/10 text-emerald-400 border border-emerald-500/20"
                            : "bg-amber-500/10 text-amber-400 border border-amber-500/20"
                        }`}
                      >
                        {item.clipping_type === "Highlight" ? (
                          <Highlighter className="w-3 h-3" />
                        ) : item.clipping_type === "Note" ? (
                          <Sparkles className="w-3 h-3" />
                        ) : (
                          <Bookmark className="w-3 h-3" />
                        )}
                        {item.clipping_type === "Highlight"
                          ? "ハイライト"
                          : item.clipping_type === "Note"
                          ? "メモ"
                          : "しおり"}
                      </span>
                      <span className="text-xs text-zinc-400">
                        {item.location && `位置: ${item.location}`}
                        {item.page && ` (ページ: ${item.page})`}
                      </span>
                    </div>

                    <span className="text-xs text-zinc-500">
                      {item.created_at
                        ? new Date(item.created_at).toLocaleString("ja-JP")
                        : ""}
                    </span>
                  </div>

                  <blockquote className="border-l-2 border-indigo-500/50 pl-3.5 text-sm text-zinc-200 leading-relaxed font-serif">
                    {item.content}
                  </blockquote>

                  <div className="text-xs text-zinc-400 flex items-center gap-2 pt-1 border-t border-zinc-800/40">
                    <span className="font-semibold text-zinc-300">{item.book_title}</span>
                    {item.author && <span className="text-zinc-500">({item.author})</span>}
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {/* ======================================================== */}
      {/* TAB 2: HANDWRITTEN NOTEBOOKS */}
      {/* ======================================================== */}
      {currentTab === "notebooks" && (
        <div className="space-y-4">
          <div className="flex flex-col sm:flex-row gap-3 items-center justify-between">
            <div className="relative flex-1 w-full">
              <Search className="w-4 h-4 text-zinc-500 absolute left-3.5 top-1/2 -translate-y-1/2" />
              <input
                type="text"
                placeholder="ノートのタイトルで検索..."
                value={nbSearch}
                onChange={(e) => setNbSearch(e.target.value)}
                className="w-full pl-10 pr-4 py-2 bg-zinc-900/80 border border-zinc-700/60 rounded-xl text-sm text-zinc-200 focus:outline-none focus:border-indigo-500"
              />
            </div>

            <button
              onClick={() => openFolder(getSubfolderPath("Notebooks"))}
              className="text-indigo-400 hover:text-indigo-300 text-xs flex items-center gap-1.5 px-3 py-2 rounded-xl bg-zinc-900 border border-zinc-800 hover:border-zinc-700 transition-colors shrink-0"
            >
              <Folder className="w-4 h-4" />
              Notebooks フォルダを開く
            </button>
          </div>

          <div className="text-xs text-zinc-400 px-1">
            手書きノート: <strong className="text-zinc-200">{filteredNotebooks.length}</strong> 冊
          </div>

          {filteredNotebooks.length === 0 ? (
            <div className="bg-zinc-900/40 border border-zinc-800/80 rounded-2xl p-12 text-center text-zinc-500">
              <PenTool className="w-8 h-8 mx-auto mb-3 opacity-40 text-zinc-400" />
              <p className="text-sm font-medium text-zinc-400">同期された手書きノートがありません</p>
              <p className="text-xs text-zinc-500 mt-1">
                Kindle Scribe をUSB接続し、同期を実行するとここにノートのプレビューが表示されます
              </p>
            </div>
          ) : (
            <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
              {filteredNotebooks.map((nb) => (
                <NotebookCard
                  key={nb.id}
                  notebook={nb}
                  onSelect={() => handleSelectNotebook(nb)}
                  onOpenFolder={() => openFolder(getSubfolderPath(`Notebooks/${nb.title}`))}
                />
              ))}
            </div>
          )}
        </div>
      )}

      {/* ======================================================== */}
      {/* TAB 3: VOCABULARY */}
      {/* ======================================================== */}
      {currentTab === "vocab" && (
        <div className="space-y-4">
          {/* Controls & Search */}
          <div className="flex flex-col md:flex-row gap-3">
            <div className="relative flex-1">
              <Search className="w-4 h-4 text-zinc-500 absolute left-3.5 top-1/2 -translate-y-1/2" />
              <input
                type="text"
                placeholder="英単語・原形・例文・書籍名で検索..."
                value={vocabSearch}
                onChange={(e) => setVocabSearch(e.target.value)}
                className="w-full pl-10 pr-4 py-2 bg-zinc-900/80 border border-zinc-700/60 rounded-xl text-sm text-zinc-200 focus:outline-none focus:border-indigo-500"
              />
            </div>

            <select
              value={vocabSelectedBook}
              onChange={(e) => setVocabSelectedBook(e.target.value)}
              className="px-3.5 py-2 bg-zinc-900/80 border border-zinc-700/60 rounded-xl text-sm text-zinc-200 focus:outline-none focus:border-indigo-500 max-w-xs truncate"
            >
              <option value="all">すべての書籍 ({vocabBookTitles.length})</option>
              {vocabBookTitles.map((title, idx) => (
                <option key={idx} value={title}>
                  {title}
                </option>
              ))}
            </select>

            <button
              onClick={() => openFolder(getSubfolderPath("Vocabulary"))}
              className="text-indigo-400 hover:text-indigo-300 text-xs flex items-center gap-1.5 px-3 py-2 rounded-xl bg-zinc-900 border border-zinc-800 hover:border-zinc-700 transition-colors shrink-0"
            >
              <Folder className="w-4 h-4" />
              Vocabulary フォルダを開く
            </button>
          </div>

          <div className="text-xs text-zinc-400 px-1">
            単語数: <strong className="text-zinc-200">{filteredVocab.length}</strong> 語 / 全 {vocab.length} 語
          </div>

          {filteredVocab.length === 0 ? (
            <div className="bg-zinc-900/40 border border-zinc-800/80 rounded-2xl p-12 text-center text-zinc-500">
              <Languages className="w-8 h-8 mx-auto mb-3 opacity-40 text-zinc-400" />
              <p className="text-sm font-medium text-zinc-400">同期された単語ログがありません</p>
              <p className="text-xs text-zinc-500 mt-1">Kindleで辞書検索した履歴 (`vocab.db`) が同期されるとここに一覧表示されます</p>
            </div>
          ) : (
            <div className="grid grid-cols-1 md:grid-cols-2 gap-3.5">
              {filteredVocab.map((item) => (
                <div
                  key={item.id}
                  className="bg-zinc-900/60 border border-zinc-800/80 rounded-2xl p-4 hover:border-zinc-700/80 transition-all space-y-2"
                >
                  <div className="flex items-start justify-between">
                    <div>
                      <span className="text-base font-bold text-indigo-300 tracking-wide">
                        {item.word}
                      </span>
                      {item.stem && item.stem !== item.word && (
                        <span className="ml-2 text-xs px-2 py-0.5 rounded bg-zinc-800 text-zinc-400 font-mono">
                          原形: {item.stem}
                        </span>
                      )}
                    </div>
                    {item.lang && (
                      <span className="text-[10px] uppercase font-semibold px-2 py-0.5 rounded bg-zinc-800 text-zinc-400">
                        {item.lang}
                      </span>
                    )}
                  </div>

                  {item.usage && (
                    <p className="text-xs text-zinc-300 leading-relaxed bg-zinc-950/40 p-2.5 rounded-xl border border-zinc-800/60 font-serif italic">
                      "{item.usage}"
                    </p>
                  )}

                  <div className="flex items-center justify-between text-[11px] text-zinc-500 pt-1 border-t border-zinc-800/40">
                    <span className="truncate max-w-[200px]" title={item.book_title || "不明な書籍"}>
                      📖 {item.book_title || "不明な書籍"}
                    </span>
                    <span>
                      {item.timestamp
                        ? new Date(item.timestamp).toLocaleDateString("ja-JP")
                        : ""}
                    </span>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {/* ======================================================== */}
      {/* ======================================================== */}
      {/* NOTEBOOK PAGES VIEWER MODAL */}
      {/* ======================================================== */}
      {selectedNotebook && (
        <div className="fixed inset-0 z-50 bg-black/80 backdrop-blur-sm flex items-center justify-center p-3 sm:p-6 animate-in fade-in duration-200">
          <div className="bg-zinc-900 border border-zinc-800 rounded-2xl max-w-5xl w-full max-h-[94vh] flex flex-col shadow-2xl overflow-hidden">
            {/* Modal Header */}
            <div className="p-4 sm:px-6 border-b border-zinc-800 flex flex-wrap items-center justify-between gap-3 bg-zinc-900/90">
              <div className="min-w-0 flex-1">
                <h3 className="text-base font-semibold text-zinc-100 flex items-center gap-2 truncate">
                  <PenTool className="w-4 h-4 text-indigo-400 shrink-0" />
                  <span className="truncate">{selectedNotebook.display_title}</span>
                </h3>
                <p className="text-xs text-zinc-400 mt-0.5">
                  全 {notebookPages.length} ページ ・ 同期日時: {new Date(selectedNotebook.synced_at).toLocaleString("ja-JP")}
                </p>
              </div>

              {/* Action Buttons in Header */}
              <div className="flex items-center gap-2 flex-wrap">
                {/* PDF Export Button */}
                <button
                  onClick={handleExportPdf}
                  disabled={isExporting || notebookPages.length === 0}
                  className="px-3 py-1.5 rounded-xl bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 text-white text-xs font-medium shadow-sm hover:shadow-indigo-500/20 transition-all flex items-center gap-1.5 cursor-pointer"
                  title="全ページを1つのPDFファイルとして出力"
                >
                  {isExporting && exportProgress ? (
                    <Loader2 className="w-3.5 h-3.5 animate-spin" />
                  ) : (
                    <FileDown className="w-3.5 h-3.5" />
                  )}
                  <span>
                    {isExporting && exportProgress
                      ? `PDF出力中 (${exportProgress.current}/${exportProgress.total})`
                      : "PDF出力"}
                  </span>
                </button>

                {/* All Images Export */}
                <button
                  onClick={handleExportAllImages}
                  disabled={isExporting || notebookPages.length === 0}
                  className="px-3 py-1.5 rounded-xl bg-zinc-800 hover:bg-zinc-700 disabled:opacity-50 text-zinc-200 text-xs font-medium border border-zinc-700/60 transition-colors flex items-center gap-1.5 cursor-pointer"
                  title="全ページの画像ファイルを指定したフォルダに保存"
                >
                  <FolderDown className="w-3.5 h-3.5 text-zinc-400" />
                  <span>全画像を保存</span>
                </button>

                {/* Selected Images Export (visible when pages are selected) */}
                {selectedPageIndices.size > 0 && (
                  <button
                    onClick={handleExportSelectedImages}
                    disabled={isExporting}
                    className="px-3 py-1.5 rounded-xl bg-purple-600 hover:bg-purple-500 disabled:opacity-50 text-white text-xs font-medium shadow-sm transition-all flex items-center gap-1.5 cursor-pointer animate-in fade-in duration-150"
                    title="選択したページの画像ファイルを指定フォルダに保存"
                  >
                    <Download className="w-3.5 h-3.5" />
                    <span>選択保存 ({selectedPageIndices.size})</span>
                  </button>
                )}

                {/* Open Vault Folder */}
                <button
                  onClick={() => openFolder(getSubfolderPath(`Notebooks/${selectedNotebook.title}`))}
                  className="px-2.5 py-1.5 rounded-xl bg-zinc-800 hover:bg-zinc-700 text-zinc-400 hover:text-zinc-200 text-xs font-medium border border-zinc-700/50 transition-colors flex items-center gap-1 cursor-pointer"
                  title="ノートの保存先フォルダを開く"
                >
                  <FolderOpen className="w-3.5 h-3.5" />
                </button>

                {/* Close Modal */}
                <button
                  onClick={() => setSelectedNotebook(null)}
                  className="p-1.5 rounded-xl hover:bg-zinc-800 text-zinc-400 hover:text-zinc-200 transition-colors ml-1 cursor-pointer"
                >
                  <X className="w-5 h-5" />
                </button>
              </div>
            </div>

            {/* Notification Toast Bar */}
            {exportNotification && (
              <div
                className={`px-4 py-2 text-xs flex items-center justify-between border-b transition-all animate-in slide-in-from-top duration-200 ${
                  exportNotification.type === "success"
                    ? "bg-emerald-950/60 text-emerald-300 border-emerald-800/60"
                    : "bg-rose-950/60 text-rose-300 border-rose-800/60"
                }`}
              >
                <div className="flex items-center gap-2">
                  {exportNotification.type === "success" ? (
                    <Check className="w-4 h-4 text-emerald-400 shrink-0" />
                  ) : (
                    <AlertCircle className="w-4 h-4 text-rose-400 shrink-0" />
                  )}
                  <span>{exportNotification.message}</span>
                </div>
                <button
                  onClick={() => setExportNotification(null)}
                  className="p-0.5 text-zinc-400 hover:text-zinc-200"
                >
                  <X className="w-3.5 h-3.5" />
                </button>
              </div>
            )}

            {/* View Mode & Selection Sub-Toolbar */}
            <div className="px-4 sm:px-6 py-2 border-b border-zinc-800/80 bg-zinc-950/40 flex items-center justify-between flex-wrap gap-2 text-xs">
              {/* View mode toggle */}
              <div className="flex items-center bg-zinc-900 rounded-lg p-0.5 border border-zinc-800">
                <button
                  onClick={() => setNbViewMode("single")}
                  className={`px-2.5 py-1 rounded-md flex items-center gap-1.5 transition-colors cursor-pointer ${
                    nbViewMode === "single"
                      ? "bg-zinc-800 text-zinc-100 font-medium shadow-sm"
                      : "text-zinc-400 hover:text-zinc-200"
                  }`}
                >
                  <ImageIcon className="w-3.5 h-3.5" />
                  <span>単一ビュー</span>
                </button>
                <button
                  onClick={() => setNbViewMode("grid")}
                  className={`px-2.5 py-1 rounded-md flex items-center gap-1.5 transition-colors cursor-pointer ${
                    nbViewMode === "grid"
                      ? "bg-zinc-800 text-zinc-100 font-medium shadow-sm"
                      : "text-zinc-400 hover:text-zinc-200"
                  }`}
                >
                  <LayoutGrid className="w-3.5 h-3.5" />
                  <span>一覧・選択ビュー</span>
                </button>
              </div>

              {/* Grid controls or Single controls */}
              {nbViewMode === "grid" ? (
                <div className="flex items-center gap-3">
                  <span className="text-zinc-400 text-xs">
                    選択中: <strong className="text-indigo-400 font-mono">{selectedPageIndices.size}</strong> / {notebookPages.length} ページ
                  </span>
                  <div className="flex items-center gap-1.5">
                    <button
                      onClick={handleSelectAllPages}
                      className="px-2.5 py-1 rounded-lg bg-zinc-800 hover:bg-zinc-700 text-zinc-300 text-[11px] font-medium transition-colors cursor-pointer"
                    >
                      すべて選択
                    </button>
                    <button
                      onClick={handleDeselectAllPages}
                      disabled={selectedPageIndices.size === 0}
                      className="px-2.5 py-1 rounded-lg bg-zinc-800 hover:bg-zinc-700 disabled:opacity-40 text-zinc-400 text-[11px] font-medium transition-colors cursor-pointer"
                    >
                      選択解除
                    </button>
                  </div>
                </div>
              ) : (
                <div className="flex items-center gap-2">
                  <button
                    onClick={handleExportCurrentImage}
                    disabled={isExporting || notebookPages.length === 0}
                    className="px-2.5 py-1 rounded-lg bg-zinc-850 hover:bg-zinc-800 text-zinc-300 border border-zinc-800 transition-colors flex items-center gap-1.5 text-[11px] cursor-pointer"
                    title="現在表示されているページ画像を指定フォルダに保存"
                  >
                    <Download className="w-3 h-3 text-zinc-400" />
                    <span>現在のページ画像を保存</span>
                  </button>
                </div>
              )}
            </div>

            {/* Modal Body / Canvas or Grid */}
            <div className="flex-1 overflow-y-auto p-4 sm:p-6 bg-zinc-950/60 min-h-[400px]">
              {loadingPages ? (
                <div className="h-full flex flex-col items-center justify-center gap-2 text-zinc-400 text-sm py-20">
                  <Layers className="w-6 h-6 animate-pulse text-indigo-500" />
                  <span>ページ画像を読み込み中...</span>
                </div>
              ) : notebookPages.length === 0 ? (
                <div className="h-full flex items-center justify-center text-center text-zinc-500 text-sm py-20">
                  ページ画像が見つかりませんでした
                </div>
              ) : nbViewMode === "single" ? (
                /* Single View Mode */
                <div className="flex flex-col items-center justify-center">
                  <div className="relative max-w-2xl w-full flex flex-col items-center">
                    <div className="bg-white rounded-xl shadow-xl overflow-hidden p-2 sm:p-4 max-h-[60vh] flex items-center justify-center border border-zinc-700">
                      {activePageDataUrl ? (
                        <img
                          src={activePageDataUrl}
                          alt={`ページ ${activePageIndex + 1}`}
                          className="max-h-[55vh] w-auto object-contain select-none"
                        />
                      ) : (
                        <div className="w-72 h-96 flex items-center justify-center text-zinc-400 text-xs">
                          読み込み中...
                        </div>
                      )}
                    </div>

                    {/* Navigation controls */}
                    <div className="flex items-center justify-between w-full max-w-xs mt-4">
                      <button
                        onClick={() => handlePageChange(activePageIndex - 1)}
                        disabled={activePageIndex === 0}
                        className="p-2 rounded-xl bg-zinc-800 hover:bg-zinc-700 disabled:opacity-30 disabled:hover:bg-zinc-800 text-zinc-200 transition-colors cursor-pointer"
                      >
                        <ChevronLeft className="w-5 h-5" />
                      </button>

                      <span className="text-xs font-medium text-zinc-300">
                        ページ {activePageIndex + 1} / {notebookPages.length}
                      </span>

                      <button
                        onClick={() => handlePageChange(activePageIndex + 1)}
                        disabled={activePageIndex === notebookPages.length - 1}
                        className="p-2 rounded-xl bg-zinc-800 hover:bg-zinc-700 disabled:opacity-30 disabled:hover:bg-zinc-800 text-zinc-200 transition-colors cursor-pointer"
                      >
                        <ChevronRight className="w-5 h-5" />
                      </button>
                    </div>
                  </div>
                </div>
              ) : (
                /* Grid View Mode */
                <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 gap-3.5">
                  {notebookPages.map((pagePath, idx) => (
                    <NotebookThumbnailCard
                      key={idx}
                      pagePath={pagePath}
                      pageNum={idx + 1}
                      isSelected={selectedPageIndices.has(idx)}
                      onToggleSelect={() => handleToggleSelectPage(idx)}
                      onViewSingle={() => {
                        handlePageChange(idx);
                        setNbViewMode("single");
                      }}
                    />
                  ))}
                </div>
              )}
            </div>

            {/* Modal Footer: Thumbnails carousel (shown only in Single view) */}
            {nbViewMode === "single" && notebookPages.length > 1 && (
              <div className="p-3 border-t border-zinc-800 bg-zinc-900/90 flex gap-2 overflow-x-auto justify-center">
                {notebookPages.map((_, idx) => (
                  <button
                    key={idx}
                    onClick={() => handlePageChange(idx)}
                    className={`px-3 py-1 rounded-lg text-xs font-medium transition-all cursor-pointer ${
                      activePageIndex === idx
                        ? "bg-indigo-600 text-white"
                        : "bg-zinc-800 text-zinc-400 hover:text-zinc-200"
                    }`}
                  >
                    p. {idx + 1}
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
};

// Subcomponent for notebook card with cover preview
const NotebookCard: React.FC<{
  notebook: NotebookSummary;
  onSelect: () => void;
  onOpenFolder: () => void;
}> = ({ notebook, onSelect, onOpenFolder }) => {
  const [coverUrl, setCoverUrl] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    if (notebook.cover_image_path) {
      readImageBase64(notebook.cover_image_path)
        .then((b64) => {
          if (active) setCoverUrl(b64);
        })
        .catch(() => {});
    }
    return () => {
      active = false;
    };
  }, [notebook.cover_image_path]);

  return (
    <div
      onClick={onSelect}
      className="bg-zinc-900/60 border border-zinc-800/80 hover:border-indigo-500/60 rounded-2xl overflow-hidden transition-all duration-200 flex flex-col cursor-pointer group shadow-sm hover:shadow-indigo-500/5"
    >
      {/* Cover / Thumbnail Preview */}
      <div className="h-44 bg-zinc-950/80 flex items-center justify-center p-3 relative overflow-hidden border-b border-zinc-800/80">
        {coverUrl ? (
          <img
            src={coverUrl}
            alt={notebook.display_title}
            className="max-h-full max-w-full object-contain rounded shadow-md group-hover:scale-105 transition-transform duration-300"
          />
        ) : (
          <div className="flex flex-col items-center gap-2 text-zinc-600">
            <PenTool className="w-8 h-8 opacity-50" />
            <span className="text-[11px]">手書きノート</span>
          </div>
        )}
      </div>

      {/* Info Details */}
      <div className="p-4 flex-1 flex flex-col justify-between space-y-3">
        <div>
          <h4 className="text-sm font-semibold text-zinc-100 group-hover:text-indigo-400 transition-colors line-clamp-1">
            {notebook.display_title}
          </h4>
          <p className="text-[11px] text-zinc-500 mt-1 flex items-center gap-1">
            <Calendar className="w-3 h-3" />
            同期: {new Date(notebook.synced_at).toLocaleDateString("ja-JP")}
          </p>
        </div>

        <div className="flex items-center justify-between pt-2 border-t border-zinc-800/50">
          <span className="text-xs text-indigo-400 font-medium group-hover:underline">
            ノートを開く →
          </span>
          <button
            onClick={(e) => {
              e.stopPropagation();
              onOpenFolder();
            }}
            title="ノートの保存先フォルダを開く"
            className="p-1.5 rounded-lg hover:bg-zinc-800 text-zinc-400 hover:text-zinc-200 transition-colors"
          >
            <FolderOpen className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>
    </div>
  );
};

// Subcomponent for notebook grid thumbnail item with select checkbox
const NotebookThumbnailCard: React.FC<{
  pagePath: string;
  pageNum: number;
  isSelected: boolean;
  onToggleSelect: () => void;
  onViewSingle: () => void;
}> = ({ pagePath, pageNum, isSelected, onToggleSelect, onViewSingle }) => {
  const [imgUrl, setImgUrl] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    readImageBase64(pagePath)
      .then((b64) => {
        if (active) setImgUrl(b64);
      })
      .catch((err) => console.error("Thumbnail load error:", err));
    return () => {
      active = false;
    };
  }, [pagePath]);

  return (
    <div
      onClick={onToggleSelect}
      className={`group relative rounded-xl border p-2 flex flex-col items-center bg-zinc-950/70 cursor-pointer transition-all duration-150 select-none ${
        isSelected
          ? "border-indigo-500 bg-indigo-950/20 ring-2 ring-indigo-500/40"
          : "border-zinc-800 hover:border-zinc-700 hover:bg-zinc-900/60"
      }`}
    >
      {/* Checkbox badge */}
      <div className="absolute top-3 left-3 z-10">
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onToggleSelect();
          }}
          className={`w-5 h-5 rounded-md flex items-center justify-center border transition-colors ${
            isSelected
              ? "bg-indigo-600 border-indigo-500 text-white"
              : "bg-zinc-900/90 border-zinc-700 text-transparent hover:border-zinc-500"
          }`}
        >
          <Check className="w-3.5 h-3.5 stroke-[3]" />
        </button>
      </div>

      {/* Thumbnail Image Container */}
      <div className="w-full h-44 sm:h-52 bg-white rounded-lg overflow-hidden flex items-center justify-center p-2 shadow-sm">
        {imgUrl ? (
          <img
            src={imgUrl}
            alt={`ページ ${pageNum}`}
            className="max-h-full max-w-full object-contain"
          />
        ) : (
          <div className="text-zinc-400 text-xs flex items-center gap-1.5">
            <Loader2 className="w-4 h-4 animate-spin text-zinc-500" />
            <span>読込中...</span>
          </div>
        )}
      </div>

      {/* Footer info & view button */}
      <div className="w-full mt-2.5 flex items-center justify-between text-xs text-zinc-400 px-1">
        <span className="font-medium font-mono text-zinc-300">p. {pageNum}</span>
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onViewSingle();
          }}
          className="text-[11px] text-indigo-400 hover:text-indigo-300 hover:underline px-2 py-0.5 rounded bg-zinc-850 hover:bg-zinc-800 border border-zinc-800 transition-colors"
        >
          拡大表示
        </button>
      </div>
    </div>
  );
};
