import { createSignal, For, Show, onMount, onCleanup } from "solid-js";
import { commands } from "@redoc/api-client";
import { loadPresenterSession } from "./presenterSession";
import { advancePresenter, backPresenter, clampPresenterIndex, formatPresenterTimer } from "./presenterControls";
import { normalizeMasters, slideChromeOverlays } from "./deckNormalize";
import { ShapeBody, isFilledShape, mergeAt } from "./shapeUtils";
import "./SlideEditor.css";

type ElementEntrance = "none" | "fade" | "zoom";
type ElementExit = "none" | "fade";

type PresenterElement = {
  id: string;
  type: string;
  x: number;
  y: number;
  width: number;
  height: number;
  content?: string;
  fontSize?: number;
  color?: string;
  rotation?: number;
  align?: "left" | "center" | "right";
  bullets?: boolean;
  entrance?: ElementEntrance;
  entranceDelayMs?: number;
  entranceDurationMs?: number;
  entranceOrder?: number;
  exit?: ElementExit;
  exitDurationMs?: number;
  fontFamily?: string;
  bold?: boolean;
  italic?: boolean;
  underline?: boolean;
  tableRows?: number;
  tableCols?: number;
  tableData?: string[][];
  tableMerges?: Array<{ r: number; c: number; rowspan: number; colspan: number }>;
  tableHeaderRow?: boolean;
  chartType?: "bar" | "pie" | "line";
  chartTitle?: string;
  chartData?: number[];
  chartLabels?: string[];
  lineColor?: string;
  lineWidth?: number;
  fillGradient?: { from: string; to: string; angle?: number };
  shadow?: boolean;
};

const PRESENTER_FONT = 'system-ui, -apple-system, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif';

export type PresenterSlide = {
  id: string;
  title?: string;
  notes?: string;
  transition?: string;
  bgOverride?: string | null;
  elements: PresenterElement[];
  masterId?: string;
  showHeader?: boolean;
  showFooter?: boolean;
  showDate?: boolean;
  showSlideNumber?: boolean;
  index?: number;
};

export type PresenterDeck = {
  slides: PresenterSlide[];
  theme?: { bgColor?: string; textColor?: string; accentColor?: string; fontFamily?: string };
  canvasWidth?: number;
  canvasHeight?: number;
  masters?: unknown;
};

function normalizeDeck(raw: unknown): PresenterDeck {
  const deck = raw as PresenterDeck;
  if (!deck?.slides?.length) {
    return { slides: [{ id: "slide-1", elements: [], notes: "" }] };
  }
  return deck;
}

function parseElement(element: any): PresenterElement | null {
  const kind = element.kind || {};
  const base = {
    id: element.id || `el-${Math.random()}`,
    x: element.x ?? 0,
    y: element.y ?? 0,
    width: element.width ?? 200,
    height: element.height ?? 100,
    rotation: element.rotation ?? 0,
    entrance: element.entrance === "fade" || element.entrance === "zoom"
      ? element.entrance
      : "none" as const,
    entranceDelayMs: Number.isFinite(element.entranceDelayMs) ? Math.max(0, Number(element.entranceDelayMs)) : 0,
    entranceDurationMs: Number.isFinite(element.entranceDurationMs) ? Math.max(50, Number(element.entranceDurationMs)) : 350,
    entranceOrder: Number.isFinite(element.entranceOrder) ? Math.max(0, Number(element.entranceOrder)) : 0,
    exit: (element.exit === "fade" ? "fade" : "none") as ElementExit,
    exitDurationMs: Number.isFinite(element.exitDurationMs) ? Math.max(50, Number(element.exitDurationMs)) : 350,
  };
  if (element.type && !element.kind) {
    // Preserve the sanitized timing fields even when the flat element carries
    // untrusted/raw values alongside its presentation metadata.
    return {
      ...base,
      ...element,
      type: element.type,
      entrance: base.entrance,
      entranceDelayMs: base.entranceDelayMs,
      entranceDurationMs: base.entranceDurationMs,
      entranceOrder: base.entranceOrder,
      exit: base.exit,
      exitDurationMs: base.exitDurationMs,
    };
  }
  if (kind.Text) {
    return {
      ...base,
      type: "text",
      content: kind.Text.text ?? "",
      fontSize: kind.Text.fontSize,
      color: kind.Text.color,
      align: kind.Text.align === "left" || kind.Text.align === "right" ? kind.Text.align : "center",
      bullets: Boolean(kind.Text.bullets),
      fontFamily: kind.Text.fontFamily,
      bold: Boolean(kind.Text.bold),
      italic: Boolean(kind.Text.italic),
      underline: Boolean(kind.Text.underline),
    };
  }
  if (kind.Shape) {
    const shapeType = kind.Shape.shapeType || "rect";
    const stroke = kind.Shape.strokeColor;
    const isLine = shapeType === "line" || shapeType === "arrow";
    const color = isLine
      ? (stroke && stroke !== "transparent" ? stroke : kind.Shape.fillColor)
      : kind.Shape.fillColor;
    const strokeWidth = Number(kind.Shape.strokeWidth);
    const gradient = kind.Shape.fillGradient;
    return {
      ...base,
      type: shapeType,
      color,
      content: kind.Shape.text || "",
      lineColor: !isLine && typeof stroke === "string" && stroke !== "transparent" ? stroke : undefined,
      lineWidth: Number.isFinite(strokeWidth) ? Math.max(0, Math.min(24, strokeWidth)) : undefined,
      fillGradient:
        gradient && typeof gradient === "object" && typeof gradient.from === "string" && typeof gradient.to === "string"
          ? { from: gradient.from, to: gradient.to, angle: Number.isFinite(gradient.angle) ? gradient.angle : 90 }
          : undefined,
      shadow: kind.Shape.shadow === true,
      fontFamily: typeof kind.Shape.fontFamily === "string" ? kind.Shape.fontFamily : undefined,
      bold: Boolean(kind.Shape.bold),
      italic: Boolean(kind.Shape.italic),
      underline: Boolean(kind.Shape.underline),
    };
  }
  if (kind.Image) {
    return { ...base, type: "image", content: kind.Image.assetHash ?? "" };
  }
  if (kind.Table) {
    return {
      ...base,
      type: "table",
      tableRows: kind.Table.rows,
      tableCols: kind.Table.cols,
      tableData: kind.Table.data,
      tableMerges: kind.Table.merges ?? [],
      tableHeaderRow: Boolean(kind.Table.headerRow),
    };
  }
  if (kind.Chart) {
    const title = kind.Chart.labels?.[0] || "Chart";
    const labels = kind.Chart.labels?.slice(1) || [];
    return {
      ...base,
      type: "chart",
      chartType:
        kind.Chart.chartType === "pie" || kind.Chart.chartType === "line"
          ? kind.Chart.chartType
          : "bar",
      chartTitle: title,
      chartData: kind.Chart.data,
      chartLabels: labels,
    };
  }
  return null;
}

/** Normalize raw deck JSON into presenter-renderable slides (shared with the
 * in-window audience slideshow). */
export function normalizePresenterSlides(raw: unknown): PresenterSlide[] {
  const deck = normalizeDeck(raw);
  return normalizeSlides(deck);
}

function normalizeSlides(deck: PresenterDeck): PresenterSlide[] {
  return deck.slides.map((slide: any, index: number) => ({
    id: slide.id,
    title: slide.title,
    notes: slide.notes || "",
    transition: slide.transition || "none",
    bgOverride: slide.bgOverride ?? slide.bg_override ?? null,
    masterId: typeof slide.masterId === "string" ? slide.masterId : undefined,
    showHeader: typeof slide.showHeader === "boolean" ? slide.showHeader : undefined,
    showFooter: typeof slide.showFooter === "boolean" ? slide.showFooter : undefined,
    showDate: typeof slide.showDate === "boolean" ? slide.showDate : undefined,
    showSlideNumber: typeof slide.showSlideNumber === "boolean" ? slide.showSlideNumber : undefined,
    elements: (slide.elements || []).map(parseElement).filter(Boolean) as PresenterElement[],
    index,
  }));
}

function slideBg(slide: PresenterSlide, theme: PresenterDeck["theme"]) {
  return slide.bgOverride || theme?.bgColor || "#ffffff";
}

function SlideElementView(props: {
  el: PresenterElement;
  hidden?: boolean;
  exiting?: boolean;
  editable?: boolean;
  onChartTitleChange?: (title: string) => void;
}) {
  const el = props.el;
  const entranceHidden = props.hidden && el.entrance !== "none";
  const zoomHidden = entranceHidden && el.entrance === "zoom";
  const exitMs = Math.max(50, el.exitDurationMs || 350);
  const fadeExit = Boolean(props.exiting) && el.exit === "fade";
  const hasEntranceMotion = el.entrance === "zoom" || el.entrance === "fade";
  const baseStyle = {
    position: "absolute" as const,
    left: `${el.x}px`,
    top: `${el.y}px`,
    width: `${el.width}px`,
    height: `${el.height}px`,
    transform: `rotate(${el.rotation || 0}deg)${zoomHidden ? " scale(0.7)" : ""}`,
    "transform-origin": "center center",
    opacity: entranceHidden ? 0 : fadeExit ? 0 : 1,
    transition: fadeExit
      ? `opacity ${exitMs}ms ease`
      : el.entrance === "zoom"
        ? `opacity ${Math.max(50, el.entranceDurationMs || 350)}ms ease, transform ${Math.max(50, el.entranceDurationMs || 350)}ms ease`
        : el.entrance === "fade"
          ? `opacity ${Math.max(50, el.entranceDurationMs || 350)}ms ease`
          : undefined,
    "transition-delay": hasEntranceMotion && !fadeExit ? `${Math.max(0, el.entranceDelayMs || 0)}ms` : undefined,
  };

  if (el.type === "text") {
    const align = el.align || "center";
    return (
      <div
        style={{
          ...baseStyle,
          display: "flex",
          "align-items": "center",
          "justify-content": align === "left" ? "flex-start" : align === "right" ? "flex-end" : "center",
          "font-family": el.fontFamily || PRESENTER_FONT,
          "font-weight": el.bold ? "bold" : "normal",
          "font-style": el.italic ? "italic" : "normal",
          "text-decoration": el.underline ? "underline" : "none",
          "font-size": `${el.fontSize || 20}px`,
          color: el.color || "#1e293b",
          "text-align": align,
          "white-space": "pre-wrap",
          padding: el.bullets ? "0 0 0 18px" : "0",
        }}
      >
        {el.bullets
          ? el.content?.split("\n").map((line) => `• ${line}`).join("\n")
          : el.content}
      </div>
    );
  }

  if (el.type === "image" && el.content) {
    return <img src={el.content} alt="" style={{ ...baseStyle, "object-fit": "contain" }} />;
  }

  if (el.type === "line" || el.type === "arrow") {
    return (
      <div style={baseStyle}>
        <ShapeBody type={el.type} color={el.color || "#3b82f6"} id={el.id} strokeColor={el.lineColor} strokeWidth={el.lineWidth} />
      </div>
    );
  }

  if (isFilledShape(el.type)) {
    return (
      <div style={baseStyle}>
        <ShapeBody type={el.type} color={el.color || "#3b82f6"} id={el.id} strokeColor={el.lineColor} strokeWidth={el.lineWidth} gradient={el.fillGradient} shadow={el.shadow} />
        <div style={{ position: "absolute", inset: 0, display: "flex", "align-items": "center", "justify-content": "center", "text-align": "center", "font-family": el.fontFamily || PRESENTER_FONT, "font-weight": el.bold ? "bold" : "normal", "font-style": el.italic ? "italic" : "normal", "text-decoration": el.underline ? "underline" : "none", "font-size": `${el.fontSize || 20}px`, color: el.color && el.color.toLowerCase() === "#ffffff" ? "#000" : "#fff", "white-space": "pre-wrap", padding: "8px" }}>
          {el.content}
        </div>
      </div>
    );
  }

  if (el.type === "table" && el.tableData) {
    const rows = el.tableRows || el.tableData.length;
    const cols = el.tableCols || el.tableData[0]?.length || 0;
    return (
      <div style={{ ...baseStyle, overflow: "auto", background: "#fff" }}>
        <table style={{ width: "100%", height: "100%", "border-collapse": "collapse", "font-size": "12px" }}>
          <tbody>
            <For each={el.tableData.slice(0, rows)}>
              {(row, ri) => (
                <tr>
                  <For each={row.slice(0, cols)}>
                    {(cell, ci) => {
                      // Mirror the editor's merge rendering so presenter view
                      // doesn't visually split merged cells.
                      const merge = mergeAt(el.tableMerges ?? [], ri(), ci());
                      if (merge && (merge.r !== ri() || merge.c !== ci())) {
                        return <td style={{ display: "none" }} />;
                      }
                      const span = merge ?? { rowspan: 1, colspan: 1 };
                      return (
                        <td
                          rowSpan={span.rowspan}
                          colSpan={span.colspan}
                          contentEditable={props.editable}
                          style={{ border: "1px solid #cbd5e1", padding: "4px", "min-width": "24px" }}
                          onInput={(e) => {
                            if (!props.editable) return;
                            const r = el.tableData!;
                            const cellIndex = el.tableData!.indexOf(row);
                            const colIndex = row.indexOf(cell);
                            if (cellIndex >= 0 && colIndex >= 0) r[cellIndex][colIndex] = e.currentTarget.textContent || "";
                          }}
                        >
                          {cell}
                        </td>
                      );
                    }}
                  </For>
                </tr>
              )}
            </For>
          </tbody>
        </table>
      </div>
    );
  }

  if (el.type === "chart") {
    const data = el.chartData || [3, 5, 2, 8];
    const labels = el.chartLabels || data.map((_, i) => String(i + 1));
    const max = Math.max(...data, 1);
    const title = el.chartTitle || "Chart";
    if (el.chartType === "pie") {
      const total = data.reduce((a, b) => a + b, 0) || 1;
      let angle = 0;
      const slices = data.map((v, i) => {
        const slice = (v / total) * 360;
        const start = angle;
        angle += slice;
        const hue = (i * 47) % 360;
        return { start, slice, color: `hsl(${hue}, 65%, 55%)` };
      });
      return (
        <div style={{ ...baseStyle, background: "#f8fafc", display: "flex", "flex-direction": "column", padding: "8px" }}>
          <input
            type="text"
            value={title}
            readOnly={!props.editable}
            onInput={(e) => props.onChartTitleChange?.(e.currentTarget.value)}
            style={{ "font-size": "14px", "font-weight": "600", border: "none", background: "transparent", "margin-bottom": "4px" }}
          />
          <svg viewBox="0 0 100 100" style={{ flex: 1 }}>
            <For each={slices}>
              {(slice, i) => {
                const r = 40;
                const cx = 50;
                const cy = 50;
                const a1 = (slice.start - 90) * Math.PI / 180;
                const a2 = (slice.start + slice.slice - 90) * Math.PI / 180;
                const x1 = cx + r * Math.cos(a1);
                const y1 = cy + r * Math.sin(a1);
                const x2 = cx + r * Math.cos(a2);
                const y2 = cy + r * Math.sin(a2);
                const large = slice.slice > 180 ? 1 : 0;
                if (slice.slice >= 359.9) {
                  return <circle cx="50" cy="50" r="40" fill={slice.color} />;
                }
                return (
                  <path
                    d={`M ${cx} ${cy} L ${x1} ${y1} A ${r} ${r} 0 ${large} 1 ${x2} ${y2} Z`}
                    fill={slice.color}
                  />
                );
              }}
            </For>
          </svg>
          <div style={{ "font-size": "10px", color: "#64748b" }}>
            <For each={labels}>{(label, i) => <span style={{ "margin-right": "6px" }}>{label}: {data[i()]}</span>}</For>
          </div>
        </div>
      );
    }
    return (
      <div style={{ ...baseStyle, background: "#f8fafc", display: "flex", "flex-direction": "column", padding: "8px" }}>
        <input
          type="text"
          value={title}
          readOnly={!props.editable}
          onInput={(e) => props.onChartTitleChange?.(e.currentTarget.value)}
          style={{ "font-size": "14px", "font-weight": "600", border: "none", background: "transparent", "margin-bottom": "4px" }}
        />
        {el.chartType === "line" ? (
          <>
            <svg viewBox={`0 0 ${data.length * 40} 100`} preserveAspectRatio="none" style={{ flex: 1, width: "100%" }}>
              <For each={data.slice(0, -1)}>
                {(_, i) => {
                  const x1 = i() * 40 + 20;
                  const x2 = (i() + 1) * 40 + 20;
                  const y1 = 100 - (data[i()] / max) * 100;
                  const y2 = 100 - (data[i() + 1] / max) * 100;
                  return <line x1={x1} y1={y1} x2={x2} y2={y2} stroke="#3b82f6" stroke-width="3" vector-effect="non-scaling-stroke" />;
                }}
              </For>
              <For each={data}>
                {(v, i) => {
                  const cx = i() * 40 + 20;
                  const cy = 100 - (v / max) * 100;
                  return <circle cx={cx} cy={cy} r="3" fill={`hsl(${(i() * 47) % 360}, 65%, 55%)`} />;
                }}
              </For>
            </svg>
            <div style={{ display: "flex", "font-size": "9px", color: "#64748b", "justify-content": "space-around" }}>
              <For each={labels}>{(label) => <span>{label}</span>}</For>
            </div>
          </>
        ) : (
          <div style={{ flex: 1, display: "flex", "align-items": "flex-end", gap: "4px", padding: "4px 0" }}>
            <For each={data}>
              {(v, i) => (
                <div style={{ flex: 1, display: "flex", "flex-direction": "column", "align-items": "center", gap: "2px" }}>
                  <div
                    style={{
                      width: "100%",
                      height: `${(v / max) * 100}%`,
                      "min-height": "2px",
                      background: `hsl(${(i() * 47) % 360}, 65%, 55%)`,
                    }}
                  />
                  <span style={{ "font-size": "9px", color: "#64748b" }}>{labels[i()]}</span>
                </div>
              )}
            </For>
          </div>
        )}
      </div>
    );
  }

  return null;
}

export function SlideStage(props: {
  slide: PresenterSlide;
  theme: PresenterDeck["theme"];
  revealCount: number;
  animClass?: string;
  scale?: number;
  exiting?: boolean;
  onClick?: () => void;
  canvasWidth?: number;
  canvasHeight?: number;
  masters?: unknown;
  slideIndex?: number;
}) {
  const w = props.canvasWidth || 960;
  const h = props.canvasHeight || 540;
  const scale = props.scale ?? 1;
  const entranceEls = () =>
    props.slide.elements
      .filter((e) => e.entrance !== "none")
      .slice()
      .sort((a, b) => (a.entranceOrder ?? 0) - (b.entranceOrder ?? 0));
  const chromeOverlays = () => slideChromeOverlays(
    {
      id: props.slide.id,
      title: props.slide.title ?? "",
      layout: "blank",
      elements: [],
      notes: "",
      transition: "none",
      masterId: props.slide.masterId,
      showHeader: props.slide.showHeader,
      showFooter: props.slide.showFooter,
      showDate: props.slide.showDate,
      showSlideNumber: props.slide.showSlideNumber,
    },
    normalizeMasters(props.masters),
    props.slideIndex ?? 0,
    w,
    h,
  );
  return (
    <div
      class={props.animClass || ""}
      onClick={() => props.onClick?.()}
      style={{
        width: `${w * scale}px`,
        height: `${h * scale}px`,
        background: slideBg(props.slide, props.theme),
        position: "relative",
        overflow: "hidden",
        "border-radius": "6px",
        "box-shadow": "0 2px 8px rgba(0,0,0,.35), 0 16px 48px rgba(0,0,0,.45)",
        cursor: props.onClick ? "pointer" : "default",
      }}
    >
      <div style={{ width: `${w}px`, height: `${h}px`, transform: `scale(${scale})`, "transform-origin": "top left", position: "relative" }}>
        <For each={props.slide.elements}>
          {(el, idx) => {
            let entranceIndex = -1;
            if (el.entrance !== "none") {
              entranceIndex = entranceEls().findIndex((candidate) => candidate.id === el.id);
              // IDs should be stable, but retain a deterministic fallback for
              // malformed decks that contain duplicate or missing IDs.
              if (entranceIndex < 0) {
                entranceIndex = props.slide.elements.slice(0, idx()).filter((e) => e.entrance !== "none").length;
              }
            }
            return (
              <SlideElementView
                el={el}
                hidden={el.entrance !== "none" && entranceIndex >= props.revealCount}
                exiting={props.exiting}
              />
            );
          }}
        </For>
        <For each={chromeOverlays()}>
          {(overlay) => (
            <div
              style={{
                position: "absolute",
                left: `${overlay.x}px`,
                top: `${overlay.y}px`,
                width: `${overlay.width}px`,
                "font-size": `${overlay.fontSize}px`,
                color: "#94a3b8",
                "text-align": overlay.align,
                "pointer-events": "none",
                "user-select": "none",
              }}
            >
              {overlay.text}
            </div>
          )}
        </For>
      </div>
    </div>
  );
}

export function PresenterView() {
  const session = loadPresenterSession();
  // Live deck signal: seeded from the opening hand-off, then updated by
  // presenter-sync events whenever the editor changes the deck.
  const [deckSignal, setDeckSignal] = createSignal(normalizeDeck(session?.deck));
  const deckRaw = () => deckSignal();
  const theme = () => deckRaw().theme;
  const slides = () => normalizeSlides(deckRaw());
  const initialIndex = clampPresenterIndex(session?.startIndex ?? 0, slides().length);

  const [slideIndex, setSlideIndex] = createSignal(initialIndex);
  const [revealCount, setRevealCount] = createSignal(0);
  const [elapsedSeconds, setElapsedSeconds] = createSignal(0);
  const [timerRunning, setTimerRunning] = createSignal(true);
  const [animClass, setAnimClass] = createSignal("");
  const [exiting, setExiting] = createSignal(false);
  let exitTimer: number | undefined;

  const currentSlide = () => slides()[slideIndex()] || slides()[0];
  const nextSlide = () => slides()[slideIndex() + 1];
  const fadeCount = () => currentSlide().elements.filter((e) => e.entrance !== "none").length;
  const exitMs = () =>
    Math.max(
      ...currentSlide()
        .elements.filter((e) => e.exit === "fade")
        .map((e) => Math.max(50, e.exitDurationMs || 350)),
      0,
    );

  const applyTransition = (transition: string) => {
    if (transition === "fade") setAnimClass("g-slide-anim-fade");
    else if (transition === "slide-left") setAnimClass("g-slide-anim-left");
    else if (transition === "slide-right") setAnimClass("g-slide-anim-right");
    else if (transition === "wipe-left") setAnimClass("g-slide-anim-wipe-left");
    else if (transition === "wipe-right") setAnimClass("g-slide-anim-wipe-right");
    else if (transition === "zoom") setAnimClass("g-slide-anim-zoom");
    else if (transition === "dissolve") setAnimClass("g-slide-anim-dissolve");
    else if (transition === "morph") setAnimClass("g-slide-anim-morph");
    else setAnimClass("");
    if (transition !== "none") {
      window.setTimeout(() => setAnimClass(""), transition === "dissolve" ? 550 : 450);
    }
  };

  const showSlide = (idx: number, sync = true) => {
    const clamped = clampPresenterIndex(idx, slides().length);
    const slide = slides()[clamped];
    setSlideIndex(clamped);
    setRevealCount(0);
    setExiting(false);
    applyTransition(slide?.transition || "none");
    if (sync) void commands.presenterNav(clamped).catch(() => undefined);
  };

  /** Leave the slide; if fade-exit elements exist, play them first. */
  const leaveSlide = (idx: number) => {
    const duration = exitMs();
    if (duration > 0) {
      setExiting(true);
      if (exitTimer !== undefined) window.clearTimeout(exitTimer);
      exitTimer = window.setTimeout(() => {
        exitTimer = undefined;
        showSlide(idx);
      }, duration);
    } else {
      showSlide(idx);
    }
  };

  const advance = () => {
    const next = advancePresenter(
      { slideIndex: slideIndex(), revealCount: revealCount() },
      slides().length,
      fadeCount(),
    );
    if (next.slideIndex !== slideIndex()) leaveSlide(next.slideIndex);
    else setRevealCount(next.revealCount);
  };

  const back = () => {
    const next = backPresenter(
      { slideIndex: slideIndex(), revealCount: revealCount() },
      fadeCount(),
    );
    if (next.slideIndex !== slideIndex()) {
      leaveSlide(next.slideIndex);
      setRevealCount(next.revealCount);
    } else setRevealCount(next.revealCount);
  };

  const revealAll = () => setRevealCount(fadeCount());

  const restartPresentation = () => {
    showSlide(0);
    setElapsedSeconds(0);
    setTimerRunning(true);
  };

  onMount(() => {
    applyTransition(currentSlide().transition || "none");
    const timer = window.setInterval(() => {
      if (timerRunning()) setElapsedSeconds((s) => s + 1);
    }, 1000);
    let unlisten: (() => void) | undefined;
    let unlistenSync: (() => void) | undefined;
    import("@tauri-apps/api/event").then((mod) => {
      mod.listen("slide_changed", (event: { payload?: { slideIndex?: number } }) => {
        const idx = event.payload?.slideIndex;
        if (typeof idx === "number" && idx !== slideIndex()) {
          showSlide(idx, false);
        }
      }).then((fn: () => void) => { unlisten = fn; })
        .catch(() => undefined);
      // Live deck updates broadcast by the editor window.
      mod.listen("presenter-deck-changed", (event: { payload?: { deck?: unknown; slideIndex?: number } }) => {
        const payload = event.payload;
        if (payload?.deck) {
          setDeckSignal(normalizeDeck(payload.deck));
          const idx = payload.slideIndex;
          if (typeof idx === "number") {
            setSlideIndex(clampPresenterIndex(idx, slides().length));
          }
        }
      }).then((fn: () => void) => { unlistenSync = fn; })
        .catch(() => undefined);
    }).catch(() => undefined);

    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "ArrowRight" || e.key === " ") {
        e.preventDefault();
        advance();
      } else if (e.key === "ArrowLeft") {
        e.preventDefault();
        back();
      } else if (e.key === "Home") {
        e.preventDefault();
        showSlide(0);
      } else if (e.key === "End") {
        e.preventDefault();
        showSlide(slides().length - 1);
        revealAll();
      } else if (e.key === "PageDown") {
        e.preventDefault();
        showSlide(slideIndex() + 1);
      } else if (e.key === "PageUp") {
        e.preventDefault();
        showSlide(slideIndex() - 1);
      } else if (e.key.toLowerCase() === "r") {
        e.preventDefault();
        restartPresentation();
      } else if (e.key === "Escape") {
        e.preventDefault();
        import("@tauri-apps/api/window").then(({ getCurrentWindow }) => {
          void getCurrentWindow().close();
        }).catch(() => window.close());
      }
    };
    window.addEventListener("keydown", onKeyDown);

    onCleanup(() => {
      window.clearInterval(timer);
      if (exitTimer !== undefined) window.clearTimeout(exitTimer);
      unlisten?.();
      unlistenSync?.();
      window.removeEventListener("keydown", onKeyDown);
    });
  });

  return (
    <div class="slide-presenter">
      <div class="slide-presenter-top">
        <div class="slide-presenter-brand">
          <span class="slide-presenter-brand-mark" aria-hidden="true" />
          <span>Redoc <span style={{ color: "#818cf8" }}>Presenter</span></span>
        </div>
        <div class="slide-presenter-meta">
          <span class="slide-pill">Slide <strong>{slideIndex() + 1}</strong> / {slides().length}</span>
          <span class="slide-pill slide-pill--timer" aria-label="Elapsed presentation time">
            {formatPresenterTimer(elapsedSeconds())}
          </span>
          <button type="button" class="slide-btn slide-btn--ghost" aria-pressed={timerRunning()} aria-label={timerRunning() ? "Pause presenter timer" : "Resume presenter timer"} onClick={() => setTimerRunning((running) => !running)}>
            {timerRunning() ? "Pause" : "Resume"}
          </button>
          <button type="button" class="slide-btn slide-btn--ghost" aria-label="Restart presentation" onClick={restartPresentation}>
            Restart
          </button>
        </div>
      </div>

      <div class="slide-presenter-main">
        <div class="slide-presenter-stagecol">
          <div class="slide-presenter-stageframe">
            <SlideStage slide={currentSlide()} theme={theme()} revealCount={revealCount()} animClass={animClass()} exiting={exiting()} onClick={advance} canvasWidth={deckRaw().canvasWidth} canvasHeight={deckRaw().canvasHeight} masters={deckRaw().masters} slideIndex={slideIndex()} />
          </div>
          <Show when={nextSlide()}>
            <div class="slide-presenter-next">
              <span class="slide-presenter-next-label">Up next</span>
              <div class="slide-presenter-nextframe">
                <SlideStage slide={nextSlide()!} theme={theme()} revealCount={999} scale={0.22} canvasWidth={deckRaw().canvasWidth} canvasHeight={deckRaw().canvasHeight} masters={deckRaw().masters} slideIndex={slideIndex() + 1} />
              </div>
            </div>
          </Show>
        </div>

        <div class="slide-presenter-panel">
          <h3 class="slide-presenter-panel-title">Speaker notes</h3>
          <div class="slide-presenter-notes">
            {currentSlide().notes || "No notes for this slide."}
          </div>
          <div class="slide-presenter-hint">
            Space / click reveals fades, then next slide · Home/End jump · R restart · Esc close
          </div>
          <div class="slide-presenter-controls">
            <button type="button" class="slide-btn" onClick={back}>
              ← Prev
            </button>
            <button type="button" class="slide-btn slide-btn--primary" style={{ flex: 1 }} onClick={advance}>
              Next →
            </button>
            <button type="button" class="slide-btn" onClick={revealAll}>
              Reveal all
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
