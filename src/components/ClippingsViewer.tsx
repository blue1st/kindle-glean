import React, { useState } from "react";
import { Clipping } from "../types";
import { Search, Highlighter, Sparkles, Bookmark, FileSearch, BookOpen } from "lucide-react";
import { previewClippings } from "../api";
import { open } from "@tauri-apps/plugin-dialog";

export const ClippingsViewer: React.FC = () => {
  const [clippings, setClippings] = useState<Clipping[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedBook, setSelectedBook] = useState<string>("all");
  const [selectedType, setSelectedType] = useState<string>("all");
  const [loading, setLoading] = useState(false);
  const [loadedFile, setLoadedFile] = useState<string | null>(null);

  const handleOpenFile = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "Kindle Clippings", extensions: ["txt"] }],
        title: "My Clippings.txt を選択",
      });

      if (selected && typeof selected === "string") {
        setLoading(true);
        setLoadedFile(selected);
        const data = await previewClippings(selected);
        setClippings(data);
        setLoading(false);
      }
    } catch (err) {
      console.error(err);
      setLoading(false);
    }
  };

  const bookTitles = Array.from(new Set(clippings.map((c) => c.book_title)));

  const filtered = clippings.filter((c) => {
    const matchesQuery =
      c.content.toLowerCase().includes(searchQuery.toLowerCase()) ||
      c.book_title.toLowerCase().includes(searchQuery.toLowerCase()) ||
      (c.author && c.author.toLowerCase().includes(searchQuery.toLowerCase()));

    const matchesBook = selectedBook === "all" || c.book_title === selectedBook;
    const matchesType = selectedType === "all" || c.clipping_type.toLowerCase() === selectedType;

    return matchesQuery && matchesBook && matchesType;
  });

  return (
    <div className="space-y-6">
      {/* Top action / file loader */}
      <div className="bg-zinc-900/60 border border-zinc-800/80 rounded-2xl p-6 flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4">
        <div>
          <h3 className="text-base font-semibold text-zinc-100 flex items-center gap-2">
            <BookOpen className="w-5 h-5 text-indigo-400" />
            ハイライト & メモ プレビュー
          </h3>
          <p className="text-xs text-zinc-400 mt-1">
            {loadedFile ? `読み込み元: ${loadedFile}` : "端末またはPC内の My Clippings.txt を開いて内容を確認・検索できます"}
          </p>
        </div>

        <button
          onClick={handleOpenFile}
          disabled={loading}
          className="flex items-center gap-2 px-4 py-2.5 rounded-xl bg-zinc-800 hover:bg-zinc-700 text-zinc-200 text-sm font-medium border border-zinc-700/50 transition-colors shrink-0"
        >
          <FileSearch className="w-4 h-4 text-zinc-400" />
          {loading ? "読み込み中..." : "My Clippings.txt を開く"}
        </button>
      </div>

      {clippings.length > 0 && (
        <>
          {/* Filters */}
          <div className="flex flex-col md:flex-row gap-3">
            <div className="relative flex-1">
              <Search className="w-4 h-4 text-zinc-500 absolute left-3.5 top-1/2 -translate-y-1/2" />
              <input
                type="text"
                placeholder="ハイライト・メモ・書籍名で検索..."
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="w-full pl-10 pr-4 py-2 bg-zinc-900/80 border border-zinc-700/60 rounded-xl text-sm text-zinc-200 focus:outline-none focus:border-indigo-500"
              />
            </div>

            <select
              value={selectedBook}
              onChange={(e) => setSelectedBook(e.target.value)}
              className="px-3.5 py-2 bg-zinc-900/80 border border-zinc-700/60 rounded-xl text-sm text-zinc-200 focus:outline-none focus:border-indigo-500"
            >
              <option value="all">すべての書籍 ({bookTitles.length})</option>
              {bookTitles.map((title, idx) => (
                <option key={idx} value={title}>
                  {title}
                </option>
              ))}
            </select>

            <select
              value={selectedType}
              onChange={(e) => setSelectedType(e.target.value)}
              className="px-3.5 py-2 bg-zinc-900/80 border border-zinc-700/60 rounded-xl text-sm text-zinc-200 focus:outline-none focus:border-indigo-500"
            >
              <option value="all">すべての種類</option>
              <option value="highlight">ハイライト</option>
              <option value="note">メモ</option>
              <option value="bookmark">しおり</option>
            </select>
          </div>

          <div className="text-xs text-zinc-400">
            {filtered.length} 件のアイテムが見つかりました（全体 {clippings.length} 件）
          </div>

          {/* Clippings List */}
          <div className="space-y-3">
            {filtered.map((item) => (
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

                <div className="text-xs text-zinc-400 flex items-center gap-2">
                  <span className="font-semibold text-zinc-300">{item.book_title}</span>
                  {item.author && <span className="text-zinc-500">({item.author})</span>}
                </div>
              </div>
            ))}
          </div>
        </>
      )}
    </div>
  );
};
