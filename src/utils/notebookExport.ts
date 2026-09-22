import { jsPDF } from "jspdf";
import { save, open } from "@tauri-apps/plugin-dialog";
import { readImageBase64, saveBinaryFile, exportNotebookPages } from "../api";

/**
 * Loads an image from a Data URL asynchronously
 */
function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.onload = () => resolve(img);
    img.onerror = (e) => reject(e);
    img.src = src;
  });
}

/**
 * Exports multiple notebook pages to a single PDF document.
 */
export async function exportNotebookToPdf(
  pagePaths: string[],
  notebookTitle: string,
  onProgress?: (current: number, total: number) => void
): Promise<{ success: boolean; filePath?: string }> {
  if (pagePaths.length === 0) {
    throw new Error("エクスポート対象のページがありません");
  }

  // 1. Open save dialog to let user choose target PDF destination
  const defaultFileName = `${notebookTitle.replace(/[\\/:*?"<>|]/g, "_")}.pdf`;
  const selectedPath = await save({
    title: "手書きノートをPDFとして保存",
    defaultPath: defaultFileName,
    filters: [
      {
        name: "PDF ドキュメント",
        extensions: ["pdf"],
      },
    ],
  });

  if (!selectedPath) {
    return { success: false };
  }

  let doc: jsPDF | null = null;
  const canvas = document.createElement("canvas");
  const ctx = canvas.getContext("2d");
  if (!ctx) {
    throw new Error("Canvasコンテキストの初期化に失敗しました");
  }

  // 2. Process each page sequentially
  for (let i = 0; i < pagePaths.length; i++) {
    onProgress?.(i + 1, pagePaths.length);
    const pagePath = pagePaths[i];

    try {
      const dataUrl = await readImageBase64(pagePath);
      const img = await loadImage(dataUrl);

      // Render onto high-res canvas (width normalized to 2048px for crisp stroke rendering)
      const targetW = 2048;
      const naturalW = img.naturalWidth || targetW;
      const naturalH = img.naturalHeight || Math.round(targetW * 1.333);
      const targetH = Math.round(targetW * (naturalH / naturalW));

      canvas.width = targetW;
      canvas.height = targetH;

      // Solid white background (ensures transparent SVGs render cleanly in PDF)
      ctx.fillStyle = "#ffffff";
      ctx.fillRect(0, 0, targetW, targetH);
      ctx.drawImage(img, 0, 0, targetW, targetH);

      const jpegData = canvas.toDataURL("image/jpeg", 0.92);

      // Page dimensions in mm (standard 210mm width with matching aspect ratio)
      const pdfWidth = 210;
      const pdfHeight = Number((pdfWidth * (targetH / targetW)).toFixed(2));

      if (!doc) {
        doc = new jsPDF({
          orientation: pdfHeight > pdfWidth ? "portrait" : "landscape",
          unit: "mm",
          format: [pdfWidth, pdfHeight],
          compress: true,
        });
      } else {
        doc.addPage([pdfWidth, pdfHeight], pdfHeight > pdfWidth ? "portrait" : "landscape");
      }

      doc.addImage(jpegData, "JPEG", 0, 0, pdfWidth, pdfHeight, undefined, "FAST");
    } catch (err) {
      console.error(`Failed to process page ${i + 1}:`, err);
    }
  }

  if (!doc) {
    throw new Error("PDFの生成に失敗しました");
  }

  // 3. Write PDF binary to selected file path via Tauri Rust command
  const arrayBuffer = doc.output("arraybuffer");
  const uint8Array = new Uint8Array(arrayBuffer);
  await saveBinaryFile(selectedPath, uint8Array);

  return { success: true, filePath: selectedPath };
}

/**
 * Copies selected or all notebook page image files to a chosen directory.
 */
export async function exportNotebookImagesToDirectory(
  pagePaths: string[],
  notebookTitle: string,
  selectedIndices?: number[]
): Promise<{ success: boolean; count: number; targetDir?: string }> {
  const targetPaths =
    selectedIndices && selectedIndices.length > 0
      ? selectedIndices
          .map((idx) => pagePaths[idx])
          .filter((p): p is string => Boolean(p))
      : pagePaths;

  if (targetPaths.length === 0) {
    throw new Error("エクスポート対象のページが選択されていません");
  }

  // 1. Open directory selection dialog
  const selectedDir = await open({
    title: "画像の保存先ディレクトリを選択",
    directory: true,
    multiple: false,
  });

  if (!selectedDir || typeof selectedDir !== "string") {
    return { success: false, count: 0 };
  }

  // 2. Call Rust backend to copy images into [targetDir]/[notebookTitle]
  const count = await exportNotebookPages(targetPaths, selectedDir, notebookTitle);

  return {
    success: true,
    count,
    targetDir: `${selectedDir}/${notebookTitle}`,
  };
}
