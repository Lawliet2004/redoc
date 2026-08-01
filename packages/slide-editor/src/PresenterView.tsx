import { createSignal, For, Show, onMount, onCleanup } from "solid-js";
import { commands } from "@redoc/api-client";
import { loadPresenterSession } from "./presenterSession";
import "./SlideEditor.css";

type ElementEntrance = "none" | "fade";

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
    entrance: element.entrance === "fade" ? "fade" as const : "none" as const,
  };
  if (element.type && !element.kind) {
    return { ...base, type: element.type, ...element };
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
    return { ...base, type: shapeType, color };
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
    return {
      ...base,
      type: "chart",
      chartType: kind.Chart.chartType === "pie" ? "pie" : "bar",
      chartTitle: kind.Chart.title,
      chartData: kind.Chart.data,
      chartLabels: kind.Chart.labels,
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
  editable?: boolean;
  onChartTitleChange?: (title: string) => void;
}) {
  const el = props.el;
  const fadeHidden = props.hidden && el.entrance === "fade";
  const baseStyle = {
    position: "absolute" as const,
    left: `${el.x}px`,
    top: `${el.y}px`,
    width: `${el.width}px`,
    height: `${el.height}px`,
    transform: `rotate(${el.rotation || 0}deg)`,
    "transform-origin": "center center",
    opacity: fadeHidden ? 0 : 1,
    transition: el.entrance === "fade" ? "opacity 0.35s ease" : undefined,
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
      <svg style={baseStyle} width="100%" height="100%" viewBox="0 0 100 100" preserveAspectRatio="none">
        <line x1="0" y1="100" x2="100" y2="0" stroke={el.color || "#3b82f6"} stroke-width="3" />
        {el.type === "arrow" && <polygon points="96,0 100,0 100,4" fill={el.color || "#3b82f6"} />}
      </svg>
    );
  }

  if (el.type === "rect" || el.type === "ellipse") {
    return (
      <div
        style={{
          ...baseStyle,
          background: el.color || "#3b82f6",
          "border-radius": el.type === "ellipse" ? "50%" : "4px",
        }}
      />
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
  onClick?: () => void;
}) {
  const w = 960;
  const h = 540;
  const scale = props.scale ?? 1;
  const fadeEls = () => props.slide.elements.filter((e) => e.entrance === "fade");
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
          {(el, idx) => (
            <SlideElementView
              el={el}
              hidden={el.entrance === "fade" && idx() >= props.revealCount}
            />
          )}
        </For>
      </div>
    </div>
  );
}

export function PresenterView() {
  const session = loadPresenterSession();
  const deckRaw = normalizeDeck(session?.deck);
  const theme = () => deckRaw.theme;
  const slides = normalizeSlides(deckRaw);
  const initialIndex = Math.min(Math.max(0, session?.startIndex ?? 0), slides.length - 1);

  const [slideIndex, setSlideIndex] = createSignal(initialIndex);
  const [revealCount, setRevealCount] = createSignal(0);
  const [elapsedSeconds, setElapsedSeconds] = createSignal(0);
  const [animClass, setAnimClass] = createSignal("");

  const currentSlide = () => slides[slideIndex()] || slides[0];
  const nextSlide = () => slides[slideIndex() + 1];
  const fadeCount = () => currentSlide().elements.filter((e) => e.entrance === "fade").length;

  const applyTransition = (transition: string) => {
    if (transition === "fade") setAnimClass("g-slide-anim-fade");
    else if (transition === "slide-left") setAnimClass("g-slide-anim-left");
    else if (transition === "slide-right") setAnimClass("g-slide-anim-right");
    else setAnimClass("");
    if (transition !== "none") {
      window.setTimeout(() => setAnimClass(""), 450);
    }
  };

  const showSlide = (idx: number, sync = true) => {
    const clamped = Math.max(0, Math.min(slides.length - 1, idx));
    const slide = slides[clamped];
    setSlideIndex(clamped);
    setRevealCount(0);
    applyTransition(slide?.transition || "none");
    if (sync) void commands.presenterNav(clamped).catch(() => undefined);
  };

  const advance = () => {
    const pending = fadeCount() - revealCount();
    if (pending > 0) {
      setRevealCount((c) => c + 1);
      return;
    }
    if (slideIndex() < slides.length - 1) {
      showSlide(slideIndex() + 1);
    }
  };

  const back = () => {
    if (revealCount() > 0) {
      setRevealCount((c) => c - 1);
      return;
    }
    if (slideIndex() > 0) {
      showSlide(slideIndex() - 1);
    }
  };

  onMount(() => {
    applyTransition(currentSlide().transition || "none");
    const timer = setInterval(() => setElapsedSeconds((s) => s + 1), 1000);
    let unlisten: (() => void) | undefined;
    import("@tauri-apps/api/event").then((mod) => {
      mod.listen("slide_changed", (event: { payload?: { slideIndex?: number } }) => {
        const idx = event.payload?.slideIndex;
        if (typeof idx === "number" && idx !== slideIndex()) {
          showSlide(idx, false);
        }
      }).then((fn: () => void) => { unlisten = fn; });
    }).catch(() => undefined);

    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "ArrowRight" || e.key === " ") {
        e.preventDefault();
        advance();
      } else if (e.key === "ArrowLeft") {
        e.preventDefault();
        back();
      } else if (e.key === "Escape") {
        e.preventDefault();
        import("@tauri-apps/api/window").then(({ getCurrentWindow }) => {
          void getCurrentWindow().close();
        }).catch(() => window.close());
      }
    };
    window.addEventListener("keydown", onKeyDown);

    onCleanup(() => {
      clearInterval(timer);
      unlisten?.();
      window.removeEventListener("keydown", onKeyDown);
    });
  });

  const formatTimer = (secs: number) => {
    const m = String(Math.floor(secs / 60)).padStart(2, "0");
    const s = String(secs % 60).padStart(2, "0");
    return `${m}:${s}`;
  };

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
        <div style={{ display: "flex", gap: "16px", "font-size": "13px", color: "#94a3b8" }}>
          <span>Slide <strong style={{ color: "#f8fafc" }}>{slideIndex() + 1}</strong> / {slides.length}</span>
          <span>Elapsed <strong style={{ color: "#f8fafc" }}>{formatTimer(elapsedSeconds())}</strong></span>
        </div>
      </div>

      <div style={{ flex: 1, display: "flex", gap: "16px", "min-height": 0 }}>
        <div style={{ flex: 2, display: "flex", "flex-direction": "column", gap: "10px", "min-width": 0 }}>
          <div style={{ flex: 1, display: "flex", "align-items": "center", "justify-content": "center", background: "#020617", "border-radius": "6px", border: "1px solid #334155", padding: "12px" }}>
            <SlideStage slide={currentSlide()} theme={theme()} revealCount={revealCount()} animClass={animClass()} onClick={advance} />
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
            Space / click reveals fades, then next slide · ← previous · Esc close
          </div>
          <div style={{ display: "flex", gap: "8px", "margin-top": "8px" }}>
            <button type="button" style={{ padding: "6px 12px", background: "#334155", color: "#fff", border: "none", "border-radius": "4px", cursor: "pointer" }} onClick={back}>
              Previous
            </button>
            <button type="button" style={{ padding: "6px 12px", background: "#0284c7", color: "#fff", border: "none", "border-radius": "4px", cursor: "pointer" }} onClick={advance}>
              Next
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
