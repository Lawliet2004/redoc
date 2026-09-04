/**
 * Rasterize a slide to a PNG via a 2D canvas, mirroring the editor's DOM
 * rendering for the element kinds Redoc supports (text, shapes, images,
 * tables as simplified grids). Used for "Export slide as PNG".
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
};

export type PngSlide = {
  elements: PngSlideElement[];
  bg?: string;
};

export const PNG_EXPORT_WIDTH = 1920;
export const PNG_EXPORT_HEIGHT = 1080;

function roundedRect(
  ctx: CanvasRenderingContext2D,
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

export async function renderSlideToPng(
  slide: PngSlide,
  opts: { width?: number; height?: number } = {},
): Promise<Blob> {
  const width = opts.width ?? PNG_EXPORT_WIDTH;
  const height = opts.height ?? PNG_EXPORT_HEIGHT;
  const scale = width / 960;
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("Canvas 2D is unavailable in this environment");

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
      const lines = wrapText(ctx, String(element.content ?? ""), element.width - 8);
      let offsetY = 4;
      for (const line of lines) {
        const lineW = ctx.measureText(line).width;
        const tx =
          element.align === "center"
            ? element.x + (element.width - lineW) / 2
            : element.align === "right"
              ? element.x + element.width - 4 - lineW
              : element.x + 4;
        ctx.fillText(line, tx, element.y + offsetY);
        offsetY += fontSize * 1.25;
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
      // Shapes; line/arrow draw a stroke, everything else a filled silhouette
      // matching the editor's shape approximation.
      if (element.type === "line" || element.type === "arrow") {
        ctx.strokeStyle = element.color || "#334155";
        ctx.lineWidth = 3;
        ctx.beginPath();
        ctx.moveTo(element.x, element.y);
        ctx.lineTo(element.x + element.width, element.y + (element.type === "arrow" ? 0 : element.height));
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

  return new Promise<Blob>((resolve, reject) => {
    canvas.toBlob((blob) => {
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
