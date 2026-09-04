import { createSignal, For, Show, onMount, onCleanup } from "solid-js";
import { commands } from "@redoc/api-client";
import { loadPresenterSession } from "./presenterSession";
import { advancePresenter, backPresenter, clampPresenterIndex, formatPresenterTimer } from "./presenterControls";
import { ShapeBody, isFilledShape } from "./shapeUtils";
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
  chartType?: "bar" | "pie";
  chartTitle?: string;
  chartData?: number[];
  chartLabels?: string[];
};

type PresenterSlide = {
  id: string;
  title?: string;
  notes?: string;
  transition?: string;
  bgOverride?: string | null;
  elements: PresenterElement[];
};

type PresenterDeck = {
  slides: PresenterSlide[];
  theme?: { bgColor?: string; textColor?: string; accentColor?: string; fontFamily?: string };
  canvasWidth?: number;
  canvasHeight?: number;
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
    const color =
      shapeType === "line" || shapeType === "arrow"
        ? (stroke && stroke !== "transparent" ? stroke : kind.Shape.fillColor)
        : kind.Shape.fillColor;
    return { ...base, type: shapeType, color, content: kind.Shape.text || "" };
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
    };
  }
  if (kind.Chart) {
    const title = kind.Chart.labels?.[0] || "Chart";
    const labels = kind.Chart.labels?.slice(1) || [];
    return {
      ...base,
      type: "chart",
      chartType: kind.Chart.chartType === "pie" ? "pie" : "bar",
      chartTitle: title,
      chartData: kind.Chart.data,
      chartLabels: labels,
    };
  }
  return null;
}

function normalizeSlides(deck: PresenterDeck): PresenterSlide[] {
  return deck.slides.map((slide: any) => ({
    id: slide.id,
    title: slide.title,
    notes: slide.notes || "",
    transition: slide.transition || "none",
    bgOverride: slide.bgOverride ?? slide.bg_override ?? null,
    elements: (slide.elements || []).map(parseElement).filter(Boolean) as PresenterElement[],
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
          "font-family": el.fontFamily || "Inter, sans-serif",
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
        <ShapeBody type={el.type} color={el.color || "#3b82f6"} id={el.id} />
      </div>
    );
  }

  if (isFilledShape(el.type)) {
    return (
      <div style={baseStyle}>
        <ShapeBody type={el.type} color={el.color || "#3b82f6"} id={el.id} />
        <div style={{ position: "absolute", inset: 0, display: "flex", "align-items": "center", "justify-content": "center", "text-align": "center", "font-family": el.fontFamily || "Inter, sans-serif", "font-weight": el.bold ? "bold" : "normal", "font-style": el.italic ? "italic" : "normal", "text-decoration": el.underline ? "underline" : "none", "font-size": `${el.fontSize || 20}px`, color: el.color && el.color.toLowerCase() === "#ffffff" ? "#000" : "#fff", "white-space": "pre-wrap", padding: "8px" }}>
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
              {(row) => (
                <tr>
                  <For each={row.slice(0, cols)}>
                    {(cell) => (
                      <td
                        contentEditable={props.editable}
                        style={{ border: "1px solid #cbd5e1", padding: "4px", "min-width": "24px" }}
                        onInput={(e) => {
                          if (!props.editable) return;
                          const r = el.tableData!;
                          const ri = el.tableData!.indexOf(row);
                          const ci = row.indexOf(cell);
                          if (ri >= 0 && ci >= 0) r[ri][ci] = e.currentTarget.textContent || "";
                        }}
                      >
                        {cell}
                      </td>
                    )}
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
      </div>
    );
  }

  return null;
}

function SlideStage(props: {
  slide: PresenterSlide;
  theme: PresenterDeck["theme"];
  revealCount: number;
  animClass?: string;
  scale?: number;
  exiting?: boolean;
  onClick?: () => void;
}) {
  const w = 960;
  const h = 540;
  const scale = props.scale ?? 1;
  const entranceEls = () =>
    props.slide.elements
      .filter((e) => e.entrance !== "none")
      .slice()
      .sort((a, b) => (a.entranceOrder ?? 0) - (b.entranceOrder ?? 0));
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
        "box-shadow": "0 4px 24px rgba(0,0,0,.4)",
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
      }).then((fn: () => void) => { unlisten = fn; });
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
      }).then((fn: () => void) => { unlistenSync = fn; });
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
    <div style={{
      display: "flex",
      "flex-direction": "column",
      height: "100vh",
      background: "#0f172a",
      color: "#f8fafc",
      "font-family": "Inter, sans-serif",
      padding: "16px",
      "box-sizing": "border-box",
      gap: "12px",
    }}>
      <div style={{ display: "flex", "justify-content": "space-between", "align-items": "center", "border-bottom": "1px solid #334155", "padding-bottom": "10px" }}>
        <h2 style={{ margin: 0, "font-size": "17px", color: "#38bdf8" }}>Redoc Presenter</h2>
        <div style={{ display: "flex", gap: "10px", "align-items": "center", "font-size": "13px", color: "#94a3b8" }}>
          <span>Slide <strong style={{ color: "#f8fafc" }}>{slideIndex() + 1}</strong> / {slides().length}</span>
          <span>Elapsed <strong style={{ color: "#f8fafc" }}>{formatPresenterTimer(elapsedSeconds())}</strong></span>
          <button type="button" aria-pressed={timerRunning()} aria-label={timerRunning() ? "Pause presenter timer" : "Resume presenter timer"} style={{ padding: "4px 8px", background: "#334155", color: "#fff", border: "none", "border-radius": "4px", cursor: "pointer" }} onClick={() => setTimerRunning((running) => !running)}>
            {timerRunning() ? "Pause" : "Resume"}
          </button>
          <button type="button" aria-label="Restart presentation" style={{ padding: "4px 8px", background: "#334155", color: "#fff", border: "none", "border-radius": "4px", cursor: "pointer" }} onClick={restartPresentation}>
            Restart
          </button>
        </div>
      </div>

      <div style={{ flex: 1, display: "flex", gap: "16px", "min-height": 0 }}>
        <div style={{ flex: 2, display: "flex", "flex-direction": "column", gap: "10px", "min-width": 0 }}>
          <div style={{ flex: 1, display: "flex", "align-items": "center", "justify-content": "center", background: "#020617", "border-radius": "6px", border: "1px solid #334155", padding: "12px" }}>
            <SlideStage slide={currentSlide()} theme={theme()} revealCount={revealCount()} animClass={animClass()} exiting={exiting()} onClick={advance} />
          </div>
          <Show when={nextSlide()}>
            <div style={{ display: "flex", "align-items": "center", gap: "10px" }}>
              <span style={{ "font-size": "11px", color: "#94a3b8", width: "72px" }}>Next slide</span>
              <SlideStage slide={nextSlide()!} theme={theme()} revealCount={999} scale={0.22} />
            </div>
          </Show>
        </div>

        <div style={{ flex: 1, display: "flex", "flex-direction": "column", gap: "10px", background: "#1e293b", border: "1px solid #334155", "border-radius": "8px", padding: "14px", "min-width": "220px" }}>
          <h3 style={{ margin: 0, "font-size": "13px", color: "#94a3b8" }}>Speaker notes</h3>
          <div style={{ flex: 1, "font-size": "13px", color: "#cbd5e1", "line-height": "1.55", overflow: "auto", "white-space": "pre-wrap" }}>
            {currentSlide().notes || "No notes for this slide."}
          </div>
          <div style={{ "font-size": "11px", color: "#64748b" }}>
            Space / click reveals fades, then next slide · Home/End jump · R restart · Esc close
          </div>
          <div style={{ display: "flex", gap: "8px", "margin-top": "8px" }}>
            <button type="button" style={{ padding: "6px 12px", background: "#334155", color: "#fff", border: "none", "border-radius": "4px", cursor: "pointer" }} onClick={back}>
              Previous
            </button>
            <button type="button" style={{ padding: "6px 12px", background: "#0284c7", color: "#fff", border: "none", "border-radius": "4px", cursor: "pointer" }} onClick={advance}>
              Next
            </button>
            <button type="button" style={{ padding: "6px 12px", background: "#334155", color: "#fff", border: "none", "border-radius": "4px", cursor: "pointer" }} onClick={revealAll}>
              Reveal all
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
