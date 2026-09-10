/**
 * Rasterize a slide to a PNG via a 2D canvas, mirroring the editor's DOM
 * rendering for the element kinds Redoc supports (text with bullets, shapes,
 * images, tables as simplified grids, and bar/line/pie charts). Used for
 * "Export slide as PNG".
 */
export type PngSlideElement = {
  type: string;
  x: number;
  y: number;
  width: number;
  height: number;
  rotation?: number;
  content?: string;
  color?: string;
  fontSize?: number;
  bold?: boolean;
  italic?: boolean;
  align?: string;
  bullets?: boolean;
  chartType?: "bar" | "line" | "pie";
  chartTitle?: string;
  chartData?: number[];
  chartLabels?: string[];
  tableData?: string[][];
  tableMerges?: Array<{ r: number; c: number; rowspan: number; colspan: number }>;
  tableHeaderRow?: boolean;
};

export type PngSlide = {
  elements: PngSlideElement[];
  bg?: string;
};

export const PNG_EXPORT_WIDTH = 1920;
export const PNG_EXPORT_HEIGHT = 1080;

function roundedRect(
  ctx: CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D,
  x: number,
  y: number,
  w: number,
  h: number,
  r: number,
) {
  const radius = Math.min(r, w / 2, h / 2);
  ctx.beginPath();
  ctx.moveTo(x + radius, y);
  ctx.arcTo(x + w, y, x + w, y + h, radius);
  ctx.arcTo(x + w, y + h, x, y + h, radius);
  ctx.arcTo(x, y + h, x, y, radius);
  ctx.arcTo(x, y, x + w, y, radius);
  ctx.closePath();
}

export function wrapText(
  ctx: { measureText: (text: string) => { width: number } },
  text: string,
  maxWidth: number,
): string[] {
  const lines: string[] = [];
  for (const paragraph of text.split("\n")) {
    let current = "";
    for (const word of paragraph.split(" ")) {
      const candidate = current ? `${current} ${word}` : word;
      if (ctx.measureText(candidate).width <= maxWidth || !current) {
        current = candidate;
      } else {
        lines.push(current);
        current = word;
      }
    }
    lines.push(current);
  }
  return lines;
}

/**
 * Render a slide to a PNG blob.
 * Uses OffscreenCanvas when available for non-blocking export,
 * falls back to regular canvas for broader compatibility.
 */
export async function renderSlideToPng(
  slide: PngSlide,
  opts: { width?: number; height?: number; canvasWidth?: number; canvasHeight?: number } = {},
): Promise<Blob> {
  const canvasWidth = opts.canvasWidth || 960;
  const canvasHeight = opts.canvasHeight || 540;
  const width = opts.width ?? PNG_EXPORT_WIDTH;
  const height = opts.height ?? Math.round((width / canvasWidth) * canvasHeight);
  const scale = width / canvasWidth;

  // Use OffscreenCanvas when available for non-blocking rendering
  const useOffscreen = typeof OffscreenCanvas !== "undefined";
  let canvas: HTMLCanvasElement | OffscreenCanvas;
  let ctx: CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D;

  if (useOffscreen) {
    canvas = new OffscreenCanvas(width, height);
    const offCtx = canvas.getContext("2d");
    if (!offCtx) throw new Error("Canvas 2D is unavailable in this environment");
    ctx = offCtx;
  } else {
    canvas = document.createElement("canvas");
    canvas.width = width;
    canvas.height = height;
    const htmlCtx = canvas.getContext("2d");
    if (!htmlCtx) throw new Error("Canvas 2D is unavailable in this environment");
    ctx = htmlCtx;
  }

  ctx.fillStyle = slide.bg || "#ffffff";
  ctx.fillRect(0, 0, width, height);
  ctx.scale(scale, scale);

  for (const element of slide.elements) {
    ctx.save();
    if (element.rotation) {
      ctx.translate(element.x + element.width / 2, element.y + element.height / 2);
      ctx.rotate((element.rotation * Math.PI) / 180);
      ctx.translate(-(element.x + element.width / 2), -(element.y + element.height / 2));
    }

    if (element.type === "text") {
      const fontSize = (element.fontSize || 16) * 0.75; // px → pt-ish canvas scale
      ctx.font = `${element.italic ? "italic " : ""}${element.bold ? "700 " : "400 "}${fontSize}px sans-serif`;
      ctx.fillStyle = element.color || "#202124";
      ctx.textBaseline = "top";
      const paragraphs = String(element.content ?? "").split("\n");
      const lines = element.bullets
        ? paragraphs.flatMap((paragraph) => wrapText(ctx, paragraph, element.width - 20).map((line) => `• ${line}`))
        : paragraphs.flatMap((paragraph) => wrapText(ctx, paragraph, element.width - 8));
      let offsetY = 4;
      for (const line of lines) {
        const lineW = ctx.measureText(line).width;
        const tx =
          element.align === "center"
            ? element.x + (element.width - lineW) / 2
            : element.align === "right"
              ? element.x + element.width - 4 - lineW
              : element.x + (element.bullets ? 20 : 4);
        ctx.fillText(line, tx, element.y + offsetY);
        offsetY += fontSize * 1.25;
      }
    } else if (element.type === "chart") {
      const data = element.chartData || [];
      const labels = element.chartLabels || [];
      const max = Math.max(...data, 1);
      ctx.fillStyle = "#f8fafc";
      ctx.fillRect(element.x, element.y, element.width, element.height);
      ctx.strokeStyle = "#cbd5e1";
      ctx.lineWidth = 1;
      ctx.strokeRect(element.x, element.y, element.width, element.height);
      const title = element.chartTitle || "Chart";
      ctx.font = "700 12px sans-serif";
      ctx.fillStyle = "#1e293b";
      ctx.textBaseline = "top";
      ctx.fillText(title, element.x + 8, element.y + 6);
      const chartTop = element.y + 28;
      const chartHeight = element.height - 36;
      const chartWidth = element.width - 16;
      if (element.chartType === "pie" && data.length) {
        const total = data.reduce((a, b) => a + b, 0) || 1;
        let angle = -Math.PI / 2;
        const r = Math.min(chartWidth, chartHeight) / 2;
        const cx = element.x + element.width / 2;
        const cy = chartTop + chartHeight / 2;
        for (let i = 0; i < data.length; i++) {
          const slice = (data[i] / total) * Math.PI * 2;
          ctx.beginPath();
          ctx.moveTo(cx, cy);
          ctx.arc(cx, cy, r, angle, angle + slice);
          ctx.closePath();
          ctx.fillStyle = `hsl(${(i * 47) % 360}, 65%, 55%)`;
          ctx.fill();
          angle += slice;
        }
      } else if (element.chartType === "line" && data.length) {
        const step = chartWidth / Math.max(1, data.length);
        ctx.strokeStyle = "#3b82f6";
        ctx.lineWidth = 3;
        ctx.beginPath();
        for (let i = 0; i < data.length; i++) {
          const px = element.x + 8 + step / 2 + i * step;
          const py = chartTop + chartHeight - (data[i] / max) * chartHeight;
          if (i === 0) ctx.moveTo(px, py);
          else ctx.lineTo(px, py);
        }
        ctx.stroke();
        for (let i = 0; i < data.length; i++) {
          const px = element.x + 8 + step / 2 + i * step;
          const py = chartTop + chartHeight - (data[i] / max) * chartHeight;
          ctx.beginPath();
          ctx.arc(px, py, 3, 0, Math.PI * 2);
          ctx.fillStyle = `hsl(${(i * 47) % 360}, 65%, 55%)`;
          ctx.fill();
        }
      } else {
        const step = chartWidth / Math.max(1, data.length);
        const barWidth = step * 0.7;
        for (let i = 0; i < data.length; i++) {
          const barH = (data[i] / max) * chartHeight;
          ctx.fillStyle = `hsl(${(i * 47) % 360}, 65%, 55%)`;
          ctx.fillRect(element.x + 8 + i * step + (step - barWidth) / 2, chartTop + chartHeight - barH, barWidth, barH);
        }
      }
      ctx.font = "9px sans-serif";
      ctx.fillStyle = "#64748b";
      for (let i = 0; i < labels.length; i++) {
        const step = chartWidth / Math.max(1, data.length);
        const px = element.x + 8 + i * step + step / 2;
        ctx.fillText(labels[i], px, chartTop + chartHeight + 2);
      }
    } else if (element.type === "table" && element.tableData?.length) {
      const data = element.tableData;
      const rows = data.length;
      const cols = data[0]?.length || 1;
      const rowH = element.height / rows;
      const colW = element.width / cols;
      const merges = element.tableMerges ?? [];
      const mergeAt = (r: number, c: number) =>
        merges.find((m) => r >= m.r && r < m.r + m.rowspan && c >= m.c && c < m.c + m.colspan) ?? null;
      ctx.fillStyle = "#ffffff";
      ctx.fillRect(element.x, element.y, element.width, element.height);
      ctx.strokeStyle = "#cbd5e1";
      ctx.lineWidth = 1;
      ctx.strokeRect(element.x, element.y, element.width, element.height);
      ctx.font = "11px sans-serif";
      ctx.textBaseline = "top";
      for (let r = 0; r < rows; r++) {
        for (let c = 0; c < cols; c++) {
          const merge = mergeAt(r, c);
          // Skip cells covered by a merge anchored elsewhere; the anchor
          // cell draws the merged rectangle below.
          if (merge && (merge.r !== r || merge.c !== c)) continue;
          const span = merge ?? { rowspan: 1, colspan: 1, r, c };
          const cellX = element.x + c * colW;
          const cellY = element.y + r * rowH;
          const cellW = colW * span.colspan;
          const cellH = rowH * span.rowspan;
          if (span.rowspan > 1 || span.colspan > 1) {
            ctx.fillStyle = "#ffffff";
            ctx.fillRect(cellX, cellY, cellW, cellH);
            ctx.strokeStyle = "#cbd5e1";
            ctx.strokeRect(cellX, cellY, cellW, cellH);
          } else if (r > 0) {
            ctx.beginPath();
            ctx.moveTo(element.x, cellY);
            ctx.lineTo(element.x + element.width, cellY);
            ctx.stroke();
          }
          if (c > 0 && span.colspan === 1) {
            ctx.beginPath();
            ctx.moveTo(cellX, cellY);
            ctx.lineTo(cellX, cellY + cellH);
            ctx.stroke();
          }
          if (element.tableHeaderRow && r === 0) {
            ctx.fillStyle = "#e2e8f0";
            ctx.fillRect(cellX, cellY, cellW, cellH);
          }
          ctx.fillStyle = "#1e293b";
          ctx.fillText(String(data[r][c] ?? ""), cellX + 4, cellY + 3);
        }
      }
    } else if (element.type === "image") {
      const src = String(element.content ?? "");
      if (src.startsWith("data:")) {
        try {
          const img = await new Promise<HTMLImageElement>((resolve, reject) => {
            const image = new Image();
            image.onload = () => resolve(image);
            image.onerror = () => reject(new Error("image decode failed"));
            image.src = src;
          });
          ctx.drawImage(img, element.x, element.y, element.width, element.height);
        } catch {
          ctx.fillStyle = "#e2e8f0";
          ctx.fillRect(element.x, element.y, element.width, element.height);
        }
      } else {
        ctx.fillStyle = element.color || "#e2e8f0";
        ctx.fillRect(element.x, element.y, element.width, element.height);
      }
    } else {
      // Shapes; line/arrow draw a diagonal stroke from the box's top-left to
      // bottom-right corner, everything else a filled silhouette matching
      // the editor's shape approximation.
      if (element.type === "line" || element.type === "arrow") {
        ctx.strokeStyle = element.color || "#334155";
        ctx.lineWidth = 3;
        ctx.beginPath();
        ctx.moveTo(element.x, element.y);
        ctx.lineTo(element.x + element.width, element.y + element.height);
        ctx.stroke();
      } else {
        ctx.fillStyle = element.color || "#3b82f6";
        if (element.type === "ellipse") {
          ctx.beginPath();
          ctx.ellipse(
            element.x + element.width / 2,
            element.y + element.height / 2,
            element.width / 2,
            element.height / 2,
            0,
            0,
            Math.PI * 2,
          );
          ctx.fill();
        } else {
          roundedRect(ctx, element.x, element.y, element.width, element.height, element.type === "roundedRect" ? 16 : 0);
          ctx.fill();
        }
      }
    }
    ctx.restore();
  }

  // OffscreenCanvas has convertToBlob (returns Promise), HTMLCanvasElement has toBlob (callback)
  if (useOffscreen && canvas instanceof OffscreenCanvas) {
    return canvas.convertToBlob({ type: "image/png" });
  }

  return new Promise<Blob>((resolve, reject) => {
    (canvas as HTMLCanvasElement).toBlob((blob) => {
      if (blob) resolve(blob);
      else reject(new Error("PNG encoding failed"));
    }, "image/png");
  });
}

/** Trigger a browser download for the given blob. */
export function downloadBlob(blob: Blob, filename: string) {
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  anchor.click();
  window.setTimeout(() => URL.revokeObjectURL(url), 5000);
}
