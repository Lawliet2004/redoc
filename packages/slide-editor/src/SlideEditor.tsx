import { createEffect, createSignal, For, Show, onMount, onCleanup, type JSX } from "solid-js";
import {
  ToolbarRow, ToolbarButton, ToolbarSep, ToolbarSelect, ToolbarColor, IconSidebar, type SidebarPanel,
  ContextMenu, type ContextMenuItem, EDITOR_COMMAND, type EditorCommandDetail,
  shortcutRegistry, emitEditorCommand,
} from "@redoc/editor-common";
import {
  IconNew, IconFolderOpen, IconSave, IconPdf, IconPrint,
  IconCut, IconCopy, IconPaste, IconUndo, IconRedo,
  IconTable, IconChart, IconImage, IconTextBox, IconSelect,
  IconLine, IconRect, IconEllipse, IconArrow,
  IconPlus, IconDuplicate, IconTrash, IconChevronUp, IconChevronDown,
  IconPresent, IconProperties, IconStyles, IconGallery, IconNavigator,
  IconTransition, IconAnimation, IconMaster, IconTextColor,
  IconBold, IconItalic, IconUnderline,
} from "@redoc/icons";
import { Dialog, showToast } from "@redoc/ui";
import { commands } from "@redoc/api-client";
import { snapElementPosition, alignElements, assignGroupId, clearGroupIds, visibleThumbRange, type AlignmentGuide } from "./geometry";
import {
  PLACEHOLDER_TITLE, PLACEHOLDER_BODY, defaultTheme, normalizeDeck, normalizeTransition, toDeck,
  normalizeCanvasSize, normalizeMasters, slideChromeOverlays, generateSlideId, CHART_TYPES,
  type Slide, type SlideElement, type SlideTransition, type ElementEntrance, type ElementExit, type ChartType,
} from "./deckNormalize";
import { stashPresenterDeck, stashPresenterDeckNow } from "./presenterSession";
import { AudienceSlideshow } from "./AudienceSlideshow";
import { downloadBlob, renderSlideToPng } from "./slidePngExport";
import { ShapeBody, isFilledShape, isShapeType, shapeClipPath, shapeBorderRadius, mergeTableCells, splitTableCell, isCoveredByMerge, mergeAt, type ShapeType } from "./shapeUtils";
import { normalizeSlideHyperlink, parseInternalSlideId, SLIDE_HYPERLINK_HINT } from "./hyperlinks";
import "./SlideEditor.css";

let clipboardBuffer: string | null = null;

type PrintPerPage = 1 | 2 | 4 | 6;

/** Fallback slide for chrome overlays when the deck is momentarily empty. */
const emptySlide: Slide = {
  id: "empty",
  title: "",
  layout: "blank",
  elements: [],
  notes: "",
  transition: "none",
};

function isPlaceholder(content: string) {
  return content === PLACEHOLDER_TITLE || content === PLACEHOLDER_BODY;
}

type ThumbTheme = { bgColor: string; textColor: string; accentColor: string };

/** Isolated filmstrip thumbnail. SolidJS memoizes each For item by reference,
 * so an untouched slide object never re-renders its subtree. */
function SlideThumb(props: {
  slide: Slide;
  active: boolean;
  theme: ThumbTheme;
  canvasWidth: number;
  canvasHeight: number;
}) {
  const thumbScale = () => 112 / props.canvasWidth;
  return (
    <div
      style={{
        width: "112px",
        height: `${Math.round(props.canvasHeight * thumbScale())}px`,
        background: props.slide.bgOverride || props.theme.bgColor || "white",
        border: props.active ? "2px solid var(--slide-accent)" : "1px solid var(--border-color)",
        padding: "0",
        overflow: "hidden",
        "box-shadow": props.active ? "0 0 0 1px var(--slide-accent)" : "0 1px 3px rgba(0,0,0,.35)",
        position: "relative",
        "flex-shrink": "0",
      }}
    >
      {/* Live miniature render of the slide scaled into the thumb. */}
      <div
        style={{
          width: `${props.canvasWidth}px`,
          height: `${props.canvasHeight}px`,
          transform: `scale(${thumbScale()})`,
          "transform-origin": "top left",
          position: "relative",
          background: props.slide.bgOverride || props.theme.bgColor || "white",
        }}
      >
        <For each={props.slide.elements}>
          {(element) => {
            const common = {
              position: "absolute" as const,
              left: `${element.x}px`,
              top: `${element.y}px`,
              width: `${element.width}px`,
              height: `${element.height}px`,
              transform: element.rotation ? `rotate(${element.rotation}deg)` : undefined,
            };
            if (element.type === "text") {
              return (
                <div style={{
                  ...common,
                  "font-size": `${Math.max(6, (element.fontSize || 16) * 0.6)}px`,
                  color: element.color || props.theme.textColor || "#202124",
                  "font-weight": (element as any).bold ? "700" : "400",
                  "font-style": (element as any).italic ? "italic" : "normal",
                  "text-align": ((element as any).align as any) || "left",
                  overflow: "hidden",
                  padding: "2px",
                  "line-height": "1.15",
                }}>
                  {String(element.content ?? "").slice(0, 220)}
                </div>
              );
            }
            if (element.type === "image") {
              const src = String(element.content ?? "");
              return (
                <div style={{
                  ...common,
                  ...(src.startsWith("data:")
                    ? { "background-image": `url(${src})`, "background-size": "cover", "background-position": "center" }
                    : { background: props.theme.accentColor || "#3b82f6" }),
                } as any} />
              );
            }
            return (
              <div style={{
                ...common,
                ...(element.type === "line" || element.type === "arrow"
                  ? { height: "3px", background: element.color || "#334155" }
                  : {
                      background: element.color || props.theme.accentColor || "#3b82f6",
                      "border-radius": element.type === "ellipse" ? "50%" : (element.type === "roundedRect" ? "12px" : "0"),
                    }),
              }} />
            );
          }}
        </For>
      </div>
    </div>
  );
}

interface SlideEditorProps {
  initialContent?: any;
  onChange?: (jsonContent: any) => void;
  onSlideInfoChange?: (info: string) => void;
  onRequestNew?: () => void;
  onRequestOpen?: () => void;
  onRequestSave?: () => void;
  onRequestExportPdf?: () => void;
  onRequestExportPptx?: () => void;
  zoomLevel?: number;
  onZoomChange?: (zoom: number) => void;
  /** Display name used to attribute slide comments. */
  authorName?: string;
}

type ResizeHandle = "nw" | "n" | "ne" | "e" | "se" | "s" | "sw" | "w";
type DrawTool = "select" | ShapeType | "text";

export function SlideEditor(props: SlideEditorProps) {
  const [slides, setSlides] = createSignal<Slide[]>(normalizeDeck(props.initialContent));
  const [theme, setTheme] = createSignal(props.initialContent?.theme || defaultTheme);
  const masters = () => normalizeMasters(props.initialContent?.masters);

  const [activeSlideIndex, setActiveSlideIndex] = createSignal(
    typeof props.initialContent?.activeSlideIndex === "number" ? props.initialContent.activeSlideIndex : 0,
  );
  const [sidebarPanel, setSidebarPanel] = createSignal<string | null>("properties");
  let canvasEl: HTMLDivElement | undefined;
  let dragPointerId: number | null = null;
  let dragStartClient = { x: 0, y: 0 };
  let dragMoved = false;
  let dragAltDuplicate = false;
  const DRAG_THRESHOLD = 5;
  const [selectedElementIds, setSelectedElementIds] = createSignal<string[]>([]);
  const selectedElementId = () => selectedElementIds()[0] ?? null;
  const setSelectedElementId = (id: string | null) => setSelectedElementIds(id ? [id] : []);
  const [dragging, setDragging] = createSignal(false);
  const [dragOffset, setDragOffset] = createSignal({ x: 0, y: 0 });
  const [resizing, setResizing] = createSignal<{
    id: string;
    handle: ResizeHandle;
    startX: number;
    startY: number;
    origX: number;
    origY: number;
    width: number;
    height: number;
  } | null>(null);
  const [localZoom, setLocalZoom] = createSignal(100);
  const zoom = () => props.zoomLevel ?? localZoom();
  const setZoom = (updater: number | ((z: number) => number)) => {
    const next = typeof updater === "function" ? updater(zoom()) : updater;
    if (props.onZoomChange) props.onZoomChange(next);
    else setLocalZoom(next);
  };
  const [filmstripDragIndex, setFilmstripDragIndex] = createSignal<number | null>(null);
  let filmstripEl: HTMLDivElement | undefined;
  const [filmstripScroll, setFilmstripScroll] = createSignal({ top: 0, height: 0 });
  const FILMSTRIP_ROW_HEIGHT = () => Math.round((canvasH() / canvasW()) * 112) + 4;
  const visibleRange = () => visibleThumbRange(
    filmstripScroll().top,
    filmstripScroll().height,
    slides().length,
    FILMSTRIP_ROW_HEIGHT(),
    4,
    activeSlideIndex(),
  );
  const windowedSlides = () => {
    const { first, last } = visibleRange();
    if (last < first) return [];
    return slides().slice(first, last + 1);
  };
  createEffect(() => {
    // Keep the active thumbnail in view when selection changes via keyboard.
    const idx = activeSlideIndex();
    const range = visibleRange();
    if (idx < range.first || idx > range.last) {
      filmstripEl?.scrollTo({ top: Math.max(0, idx * FILMSTRIP_ROW_HEIGHT() - 60), behavior: "auto" });
    }
  });
  const [alignmentGuides, setAlignmentGuides] = createSignal<AlignmentGuide[]>([]);
  const [drawTool, setDrawTool] = createSignal<DrawTool>("select");
  const [fillColor, setFillColor] = createSignal("#3b82f6");
  const [lineColor, setLineColor] = createSignal("#1e293b");
  const [slideAnimClass, setSlideAnimClass] = createSignal("");
  const [contextMenu, setContextMenu] = createSignal<{ x: number; y: number; items: ContextMenuItem[] } | null>(null);
  const [printDialogOpen, setPrintDialogOpen] = createSignal(false);
  const [printPerPage, setPrintPerPage] = createSignal<PrintPerPage>(1);
  const [printIncludeNotes, setPrintIncludeNotes] = createSignal(false);
  const [notesHeight, setNotesHeight] = createSignal(96);
  const [notesHistoryPushed, setNotesHistoryPushed] = createSignal(false);
  const [commentDraft, setCommentDraft] = createSignal("");
  const [slideshowOpen, setSlideshowOpen] = createSignal(false);
  const [slideshowDeck, setSlideshowDeck] = createSignal<unknown>(null);
  /** Cell (row, col) last interacted with in the selected table element. */
  const [tableContextCell, setTableContextCell] = createSignal<{ r: number; c: number } | null>(null);

  const applyTableMerge = (r: number, c: number, rowspan: number, colspan: number) => {
    const id = selectedElementId();
    const el = selectedElement();
    if (!id || el?.type !== "table") return;
    const next = mergeTableCells(el.tableData ?? [], el.tableMerges ?? [], r, c, rowspan, colspan);
    if (!next) return;
    patchSelected({ tableMerges: next });
  };

  const applyTableSplit = (r: number, c: number) => {
    const id = selectedElementId();
    const el = selectedElement();
    if (!id || el?.type !== "table") return;
    patchSelected({ tableMerges: splitTableCell(el.tableMerges ?? [], r, c) });
  };

  // --- Slide comment operations (offline review) ---
  const addSlideComment = () => {
    const text = commentDraft().trim();
    if (!text) return;
    const idx = activeSlideIndex();
    const list = [...slides()];
    const slide = list[idx];
    if (!slide) return;
    pushSlideHistory();
    const comment = {
      id: `comment-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
      author: props.authorName?.trim() || "You",
      text: text.slice(0, 2000),
      resolved: false,
      createdAt: new Date().toISOString(),
    };
    list[idx] = { ...slide, comments: [...(slide.comments ?? []), comment] };
    setSlides(list);
    emitChange(list);
    setCommentDraft("");
  };

  const setSlideCommentResolved = (id: string, resolved: boolean) => {
    const idx = activeSlideIndex();
    const list = [...slides()];
    const slide = list[idx];
    if (!slide) return;
    pushSlideHistory();
    list[idx] = {
      ...slide,
      comments: (slide.comments ?? []).map((comment) =>
        comment.id === id ? { ...comment, resolved } : comment,
      ),
    };
    setSlides(list);
    emitChange(list);
  };

  const deleteSlideComment = (id: string) => {
    const idx = activeSlideIndex();
    const list = [...slides()];
    const slide = list[idx];
    if (!slide || !(slide.comments ?? []).some((comment) => comment.id === id)) return;
    pushSlideHistory();
    list[idx] = { ...slide, comments: (slide.comments ?? []).filter((comment) => comment.id !== id) };
    setSlides(list);
    emitChange(list);
  };

  // --- Undo / Redo snapshot stack ---
  type SlideSnapshot = { slides: Slide[]; activeSlideIndex: number };
  const [slidePast, setSlidePast] = createSignal<SlideSnapshot[]>([]);
  const [slideFuture, setSlideFuture] = createSignal<SlideSnapshot[]>([]);

  const captureSlideSnapshot = (): SlideSnapshot => ({
    slides: JSON.parse(JSON.stringify(slides())),
    activeSlideIndex: activeSlideIndex(),
  });

  const MAX_SLIDE_HISTORY = 50;

  const pushSlideHistory = () => {
    setSlidePast((prev) => [...prev, captureSlideSnapshot()].slice(-MAX_SLIDE_HISTORY));
    setSlideFuture([]);
  };

  const undoSlide = () => {
    const past = slidePast();
    if (!past.length) return;
    const snapshot = past[past.length - 1];
    setSlidePast(past.slice(0, -1));
    setSlideFuture((prev) => [...prev, captureSlideSnapshot()].slice(-MAX_SLIDE_HISTORY));
    setSlides(snapshot.slides);
    setActiveSlideIndex(Math.min(snapshot.activeSlideIndex, snapshot.slides.length - 1));
    setSelectedElementId(null);
    emitChange(snapshot.slides);
  };

  const redoSlide = () => {
    const future = slideFuture();
    if (!future.length) return;
    const snapshot = future[future.length - 1];
    setSlideFuture(future.slice(0, -1));
    setSlidePast((prev) => [...prev, captureSlideSnapshot()].slice(-MAX_SLIDE_HISTORY));
    setSlides(snapshot.slides);
    setActiveSlideIndex(Math.min(snapshot.activeSlideIndex, snapshot.slides.length - 1));
    setSelectedElementId(null);
    emitChange(snapshot.slides);
  };

  const selectElement = (id: string, additive = false) => {
    if (additive) {
      const current = selectedElementIds();
      if (current.includes(id)) setSelectedElementIds(current.filter((x) => x !== id));
      else setSelectedElementIds([...current, id]);
    } else {
      setSelectedElementIds([id]);
    }
  };

  const getSelectedElements = (): SlideElement[] => {
    const slide = activeSlide();
    if (!slide) return [];
    const ids = selectedElementIds();
    if (ids.length) return slide.elements.filter((el) => ids.includes(el.id));
    return [];
  };

  const alignSelected = (mode: "left" | "center" | "right" | "top" | "middle" | "bottom") => {
    const els = getSelectedElements();
    if (!els.length) return;
    pushSlideHistory();
    const positions = alignElements(els, mode, canvasW(), canvasH());
    setSlides(prev => prev.map((s, i) => {
      if (i !== activeSlideIndex()) return s;
      return {
        ...s,
        elements: s.elements.map(e => {
          const pos = positions.find((p) => p.id === e.id);
          if (pos) {
            return { ...e, x: pos.x, y: pos.y };
          }
          return e;
        })
      };
    }));
    commitChange();
  };

  const distributeSelected = (axis: "h" | "v") => {
    const els = [...getSelectedElements()].sort((a, b) => axis === "h" ? a.x - b.x : a.y - b.y);
    if (els.length < 3) return;
    pushSlideHistory();
    const updates = new Map<string, number>();
    if (axis === "h") {
      const minX = els[0].x;
      const maxX = els[els.length - 1].x;
      const totalWidth = els.reduce((sum, e) => sum + e.width, 0);
      const gap = (maxX + els[els.length - 1].width - minX - totalWidth) / (els.length - 1);
      let x = minX;
      for (const el of els) {
        updates.set(el.id, x);
        x += el.width + gap;
      }
    } else {
      const minY = els[0].y;
      const maxY = els[els.length - 1].y;
      const totalHeight = els.reduce((sum, e) => sum + e.height, 0);
      const gap = (maxY + els[els.length - 1].height - minY - totalHeight) / (els.length - 1);
      let y = minY;
      for (const el of els) {
        updates.set(el.id, y);
        y += el.height + gap;
      }
    }
    setSlides(prev => prev.map((s, i) => {
      if (i !== activeSlideIndex()) return s;
      return {
        ...s,
        elements: s.elements.map(e => {
          if (updates.has(e.id)) {
            return axis === "h" ? { ...e, x: updates.get(e.id)! } : { ...e, y: updates.get(e.id)! };
          }
          return e;
        })
      };
    }));
    commitChange();
  };

  const groupSelected = () => {
    const ids = selectedElementIds();
    if (ids.length < 2) return;
    pushSlideHistory();
    const gid = `grp-${Date.now()}`;
    const list = [...slides()];
    const slide = list[activeSlideIndex()];
    slide.elements = assignGroupId(slide.elements, ids, gid);
    list[activeSlideIndex()] = slide;
    setSlides(list);
    commitChange(list);
  };

  const ungroupSelected = () => {
    const ids = selectedElementIds();
    if (!ids.length) return;
    pushSlideHistory();
    const list = [...slides()];
    const slide = list[activeSlideIndex()];
    slide.elements = clearGroupIds(slide.elements, ids);
    list[activeSlideIndex()] = slide;
    setSlides(list);
    commitChange(list);
  };

  const elementsToMoveWith = (el: SlideElement): SlideElement[] => {
    const slide = activeSlide();
    if (!slide) return [el];
    const ids = selectedElementIds();
    if (ids.length > 1 && ids.includes(el.id)) {
      return slide.elements.filter((e) => ids.includes(e.id));
    }
    if (el.groupId) return slide.elements.filter((e) => e.groupId === el.groupId);
    return [el];
  };

  const activeSlide = () => slides()[activeSlideIndex()];
  const selectedElement = () => activeSlide()?.elements.find((el) => el.id === selectedElementId()) || null;
  const [hyperlinkDraft, setHyperlinkDraft] = createSignal("");
  const [hyperlinkError, setHyperlinkError] = createSignal<string | null>(null);
  createEffect(() => {
    setHyperlinkDraft(selectedElement()?.hyperlink || "");
    setHyperlinkError(null);
  });
  const canvasSize = () => normalizeCanvasSize(
    props.initialContent?.canvasWidth,
    props.initialContent?.canvasHeight,
  );
  const canvasW = () => canvasSize().width;
  const canvasH = () => canvasSize().height;
  // Per-keystroke pipeline throttle: serialize the deck at most once per
  // animation frame instead of once per input event.
  let emitScheduled = false;
  let pendingSlides: Slide[] | null = null;
  const emitChange = (next: Slide[] = slides()) => {
    pendingSlides = next;
    if (emitScheduled) return;
    emitScheduled = true;
    window.requestAnimationFrame(() => {
      emitScheduled = false;
      const deck = toDeck(
        pendingSlides ?? slides(),
        props.initialContent,
        theme(),
        activeSlideIndex(),
        masters(),
      );
      props.onChange?.(deck);
      stashPresenterDeck(deck, activeSlideIndex());
    });
  };

  const canvasToLocal = (clientX: number, clientY: number) => {
    const rect = canvasEl?.getBoundingClientRect();
    if (!rect || rect.width <= 0 || rect.height <= 0) {
      return { x: clientX, y: clientY };
    }
    return {
      x: (clientX - rect.left) * (canvasW() / rect.width),
      y: (clientY - rect.top) * (canvasH() / rect.height),
    };
  };

  const slideBackground = () => activeSlide()?.bgOverride || theme().bgColor;

  const applyResize = (el: SlideElement, handle: ResizeHandle, localX: number, localY: number, start: NonNullable<ReturnType<typeof resizing>>, shiftKey: boolean): SlideElement => {
    const minW = 40;
    const minH = 30;
    const cw = canvasW();
    const ch = canvasH();
    let { origX, origY, width, height } = start;
    const ratio = width / height || 1;
    const right = origX + width;
    const bottom = origY + height;

    // Rotated elements: the handles live inside the rotated div, but the model
    // stores the unrotated AABB. Inverse-rotate the pointer around the
    // element center into the unrotated frame so the AABB math stays correct.
    const rotation = ((el.rotation ?? 0) % 360 + 360) % 360;
    if (rotation !== 0) {
      const rad = (-rotation * Math.PI) / 180;
      const cx = origX + width / 2;
      const cy = origY + height / 2;
      const dx = localX - cx;
      const dy = localY - cy;
      localX = cx + dx * Math.cos(rad) - dy * Math.sin(rad);
      localY = cy + dx * Math.sin(rad) + dy * Math.cos(rad);
    }

    if (handle.includes("e")) {
      width = Math.max(minW, Math.min(cw - origX, localX - origX));
    }
    if (handle.includes("w")) {
      const newX = Math.max(0, Math.min(right - minW, localX));
      width = right - newX;
      origX = newX;
    }
    if (handle.includes("s")) {
      height = Math.max(minH, Math.min(ch - origY, localY - origY));
    }
    if (handle.includes("n")) {
      const newY = Math.max(0, Math.min(bottom - minH, localY));
      height = bottom - newY;
      origY = newY;
    }

    if (shiftKey) {
      if (handle === "nw" || handle === "ne" || handle === "se" || handle === "sw") {
        height = width / ratio;
        if (handle.includes("n")) origY = bottom - height;
      } else if (handle === "e" || handle === "w") {
        height = width / ratio;
      } else if (handle === "n" || handle === "s") {
        width = height * ratio;
      }
    }

    const newX = Math.max(0, Math.min(cw - minW, origX));
    const newY = Math.max(0, Math.min(ch - minH, origY));
    return {
      ...el,
      x: newX,
      y: newY,
      width: Math.max(minW, Math.min(cw - newX, width)),
      height: Math.max(minH, Math.min(ch - newY, height)),
    };
  };

  const reorderSlides = (from: number, to: number) => {
    if (from === to || from < 0 || to < 0 || from >= slides().length || to >= slides().length) return;
    pushSlideHistory();
    const next = [...slides()];
    const [item] = next.splice(from, 1);
    next.splice(to, 0, item);
    setSlides(next);
    const active = activeSlideIndex();
    if (active === from) setActiveSlideIndex(to);
    else if (from < active && to >= active) setActiveSlideIndex(active - 1);
    else if (from > active && to <= active) setActiveSlideIndex(active + 1);
    emitChange(next);
  };

  const addTable = () => {
    pushSlideHistory();
    const rows = 3;
    const cols = 3;
    const data = Array.from({ length: rows }, () => Array.from({ length: cols }, () => ""));
    const element: SlideElement = {
      id: `el-${Date.now()}`,
      type: "table",
      x: 120,
      y: 120,
      width: 360,
      height: 180,
      content: "",
      tableRows: rows,
      tableCols: cols,
      tableData: data,
      rotation: 0,
    };
    const list = [...slides()];
    const slide = list[activeSlideIndex()];
    list[activeSlideIndex()] = { ...slide, elements: [...slide.elements, element] };
    setSlides(list);
    setSelectedElementId(element.id);
    emitChange(list);
  };

  const addChart = () => {
    pushSlideHistory();
    const element: SlideElement = {
      id: `el-${Date.now()}`,
      type: "chart",
      x: 140,
      y: 100,
      width: 400,
      height: 280,
      content: "",
      chartType: "bar",
      chartTitle: "Chart",
      chartData: [3, 5, 2, 8],
      chartLabels: ["A", "B", "C", "D"],
      rotation: 0,
    };
    const list = [...slides()];
    const slide = list[activeSlideIndex()];
    list[activeSlideIndex()] = { ...slide, elements: [...slide.elements, element] };
    setSlides(list);
    setSelectedElementId(element.id);
    emitChange(list);
  };

  const pasteImageFromClipboard = (file: File) => {
    const reader = new FileReader();
    reader.onload = () => {
      const src = String(reader.result || "");
      if (!src) return;
      pushSlideHistory();
      const element: SlideElement = {
        id: `el-${Date.now()}`,
        type: "image",
        x: 200,
        y: 150,
        width: 320,
        height: 240,
        content: src,
        rotation: 0,
      };
      const list = [...slides()];
      const slide = list[activeSlideIndex()];
      list[activeSlideIndex()] = { ...slide, elements: [...slide.elements, element] };
      setSlides(list);
      setSelectedElementId(element.id);
      emitChange(list);
    };
    reader.readAsDataURL(file);
  };

  const setSlideBgOverride = (color: string | null) => {
    pushSlideHistory();
    const list = [...slides()];
    const slide = list[activeSlideIndex()];
    if (!slide) return;
    list[activeSlideIndex()] = { ...slide, bgOverride: color };
    setSlides(list);
    emitChange(list);
  };

  const commitChange = (next?: Slide[]) => {
    emitChange(next);
  };

  const patchSelected = (patch: Partial<SlideElement>) => {
    pushSlideHistory();
    const id = selectedElementId();
    if (!id) return;
    const list = [...slides()];
    const slide = { ...list[activeSlideIndex()] };
    slide.elements = slide.elements.map((el) => (el.id === id ? { ...el, ...patch } : el));
    list[activeSlideIndex()] = slide;
    setSlides(list);
    emitChange(list);
  };

  /** Move an element's entrance order within the slide's animated sequence. */
  const reorderAnimation = (elementId: string, direction: -1 | 1) => {
    const slide = slides()[activeSlideIndex()];
    if (!slide) return;
    const animated = slide.elements
      .filter((el) => el.entrance !== "none" || el.exit !== "none")
      .slice()
      .sort((a, b) => (a.entranceOrder ?? 0) - (b.entranceOrder ?? 0));
    const position = animated.findIndex((el) => el.id === elementId);
    const target = position + direction;
    if (position < 0 || target < 0 || target >= animated.length) return;
    const [moved] = animated.splice(position, 1);
    animated.splice(target, 0, moved);
    const orderById = new Map(animated.map((el, index) => [el.id, index]));
    pushSlideHistory();
    const list = [...slides()];
    const updated = { ...list[activeSlideIndex()] };
    updated.elements = updated.elements.map((el) =>
      orderById.has(el.id) ? { ...el, entranceOrder: orderById.get(el.id) } : el,
    );
    list[activeSlideIndex()] = updated;
    setSlides(list);
    emitChange(list);
  };

  const applySelectedHyperlink = (value = hyperlinkDraft()) => {
    const normalized = value.trim() ? normalizeSlideHyperlink(value) : null;
    if (value.trim() && !normalized) {
      setHyperlinkError(`Enter a safe ${SLIDE_HYPERLINK_HINT}.`);
      return;
    }
    setHyperlinkError(null);
    setHyperlinkDraft(normalized || "");
    patchSelected({ hyperlink: normalized || undefined });
  };

  const updateSelectedColor = (color: string) => {
    setFillColor(color);
    setLineColor(color);
    patchSelected({ color });
  };

  const setThemePreset = (preset: string) => {
    pushSlideHistory();
    const presets: Record<string, typeof defaultTheme> = {
      light: defaultTheme,
      midnight: { id: "midnight", name: "Midnight", bgColor: "#0f172a", textColor: "#f8fafc", accentColor: "#38bdf8", fontFamily: "Inter, sans-serif" },
      coral: { id: "coral", name: "Coral", bgColor: "#fff7ed", textColor: "#431407", accentColor: "#f97316", fontFamily: "Inter, sans-serif" },
      forest: { id: "forest", name: "Forest", bgColor: "#f0fdf4", textColor: "#14532d", accentColor: "#16a34a", fontFamily: "Inter, sans-serif" },
      lavender: { id: "lavender", name: "Lavender", bgColor: "#faf5ff", textColor: "#581c87", accentColor: "#a855f7", fontFamily: "Inter, sans-serif" },
      sunset: { id: "sunset", name: "Sunset", bgColor: "#fff1f2", textColor: "#881337", accentColor: "#e11d48", fontFamily: "Inter, sans-serif" },
    };
    const nextTheme = presets[preset] || defaultTheme;
    const previous = theme();
    setTheme(nextTheme);
    // Restyle existing content: elements carry explicit colors from the old
    // theme (or defaults). Without this, a dark theme over light-styled text
    // leaves unreadable slides. Map old theme slots -> new theme slots and
    // the common insertion defaults.
    const colorMap = new Map<string, string>([
      [previous.bgColor.toLowerCase(), nextTheme.bgColor],
      [previous.textColor.toLowerCase(), nextTheme.textColor],
      [previous.accentColor.toLowerCase(), nextTheme.accentColor],
      ["#3b82f6", nextTheme.accentColor],
      ["#ffffff", nextTheme.textColor],
      ["#000000", nextTheme.textColor],
    ]);
    const remap = (color: string | undefined): string | undefined => {
      if (!color) return color;
      return colorMap.get(color.toLowerCase()) ?? color;
    };
    const restyled = slides().map((slide) => ({
      ...slide,
      elements: slide.elements.map((el) => {
        const next = { ...el };
        if (next.color) next.color = remap(next.color)!;
        if (next.lineColor) next.lineColor = remap(next.lineColor)!;
        return next;
      }),
    }));
    setSlides(restyled);
    emitChange(restyled);
  };

  const setSlideTransition = (transition: SlideTransition) => {
    pushSlideHistory();
    const list = [...slides()];
    const slide = list[activeSlideIndex()];
    if (!slide) return;
    list[activeSlideIndex()] = { ...slide, transition };
    setSlides(list);
    emitChange(list);
  };

  const reorderSelected = (toFront: boolean) => {
    pushSlideHistory();
    const id = selectedElementId();
    const slide = activeSlide();
    if (!id || !slide) return;
    const elements = [...slide.elements];
    const index = elements.findIndex((el) => el.id === id);
    if (index < 0) return;
    const [item] = elements.splice(index, 1);
    if (toFront) elements.push(item);
    else elements.unshift(item);
    const list = [...slides()];
    list[activeSlideIndex()] = { ...slide, elements };
    setSlides(list);
    emitChange(list);
  };

  /** Step the selected element one layer toward front/back (PowerPoint-style). */
  const stepSelectedZOrder = (forward: boolean) => {
    pushSlideHistory();
    const id = selectedElementId();
    const slide = activeSlide();
    if (!id || !slide) return;
    const elements = [...slide.elements];
    const index = elements.findIndex((el) => el.id === id);
    if (index < 0) return;
    const target = forward ? Math.min(elements.length - 1, index + 1) : Math.max(0, index - 1);
    if (target === index) return;
    const [item] = elements.splice(index, 1);
    elements.splice(target, 0, item);
    const list = [...slides()];
    list[activeSlideIndex()] = { ...slide, elements };
    setSlides(list);
    emitChange(list);
  };

  const rotateSelected = (delta: number) => {
    const el = selectedElement();
    if (!el) return;
    const next = ((el.rotation || 0) + delta) % 360;
    patchSelected({ rotation: next < 0 ? next + 360 : next });
  };

  const addSlide = () => {
    pushSlideHistory();
    const id = generateSlideId("slide");
    const newSlide: Slide = {
      id,
      title: `Slide ${slides().length + 1}`,
      elements: [
        {
          id: generateSlideId("el"),
          type: "text",
          x: 100,
          y: 100,
          width: 500,
          height: 60,
          content: PLACEHOLDER_TITLE,
          fontSize: 32,
          color: "#1e293b",
          rotation: 0,
          align: "center",
        },
      ],
      layout: "title",
      notes: "",
      transition: "none",
    };
    const next = [...slides(), newSlide];
    setSlides(next);
    emitChange(next);
    setActiveSlideIndex(next.length - 1);
    if (props.onSlideInfoChange) {
      props.onSlideInfoChange(`Slide ${next.length} of ${next.length}`);
    }
  };

  const applyLayout = (layout: string) => {
    const slide = activeSlide();
    if (!slide) return;
    // Only scaffold placeholder elements for an empty slide. Applying a
    // layout to a slide the user has filled keeps their content and changes
    // just the layout tag — the old behavior silently deleted every element.
    // The `placeholder` flag is set on scaffolded elements, so user content
    // that happens to match placeholder text is never misdetected.
    const hasUserContent = slide.elements.some((el) => !el.placeholder);
    if (hasUserContent) {
      pushSlideHistory();
      const next = [...slides()];
      next[activeSlideIndex()] = { ...slide, layout, title: layout };
      setSlides(next);
      emitChange(next);
      return;
    }
    pushSlideHistory();
    const heading = (content: string, x = 100, y = 70, width = 760): SlideElement => ({
      id: `el-${Date.now()}-${Math.random()}`,
      type: "text",
      x, y, width, height: 60,
      content,
      fontSize: 32,
      color: theme().textColor,
      placeholder: true,
    });
    const nextElements: Record<string, SlideElement[]> = {
      title: [heading(PLACEHOLDER_TITLE, 100, 170, 760), { ...heading(PLACEHOLDER_BODY, 150, 270, 660), fontSize: 22, color: theme().textColor }],
      "title-body": [heading(PLACEHOLDER_TITLE, 80, 45, 800), { ...heading(PLACEHOLDER_BODY, 100, 150, 760), fontSize: 24, color: theme().textColor }],
      section: [heading(PLACEHOLDER_TITLE, 100, 220, 760)],
      "two-column": [heading(PLACEHOLDER_TITLE, 80, 40, 800), { ...heading(PLACEHOLDER_BODY, 70, 160, 360), fontSize: 22, color: theme().textColor }, { ...heading(PLACEHOLDER_BODY, 530, 160, 360), fontSize: 22, color: theme().textColor }],
      "image-caption": [{ id: `el-${Date.now()}-image`, type: "rect", x: 180, y: 80, width: 600, height: 320, content: "", color: theme().accentColor, placeholder: true }, { ...heading(PLACEHOLDER_BODY, 150, 430, 660), fontSize: 22, color: theme().textColor }],
      blank: [],
    };
    const next = [...slides()];
    next[activeSlideIndex()] = { ...slide, layout, title: layout, elements: nextElements[layout] || [] };
    setSlides(next);
    emitChange(next);
  };

  const addShape = (type: ShapeType) => {
    pushSlideHistory();
    const el: SlideElement = {
      id: `el-${Date.now()}`,
      type,
      x: 380,
      y: 200,
      width: type === "line" || type === "arrow" ? 200 : 200,
      height: type === "line" || type === "arrow" ? 40 : 120,
      content: "",
      color: fillColor(),
    };
    const current = activeSlide();
    const updated = { ...current, elements: [...current.elements, el] };
    const list = [...slides()];
    list[activeSlideIndex()] = updated;
    setSlides(list);
    setSelectedElementId(el.id);
    emitChange(list);
  };

  const addTextBox = () => {
    pushSlideHistory();
    const el: SlideElement = {
      id: `el-${Date.now()}`,
      type: "text",
      x: 280,
      y: 200,
      width: 400,
      height: 60,
      content: PLACEHOLDER_BODY,
      fontSize: 24,
      color: theme().textColor,
    };
    const list = [...slides()];
    list[activeSlideIndex()] = { ...activeSlide(), elements: [...activeSlide().elements, el] };
    setSlides(list);
    setSelectedElementId(el.id);
    setDrawTool("text");
    emitChange(list);
  };

  const addImageFromUrl = (src: string | null) => {
    if (!src) return;
    pushSlideHistory();
    const el: SlideElement = { id: `el-${Date.now()}`, type: "image", x: 180, y: 100, width: 600, height: 320, content: src, color: "#ffffff" };
    const list = [...slides()];
    list[activeSlideIndex()] = { ...activeSlide(), elements: [...activeSlide().elements, el] };
    setSlides(list);
    setSelectedElementId(el.id);
    emitChange(list);
  };

  const addImage = () => {
    // Native file picker only. The old fallback `window.prompt("Image URL")`
    // was a blocking browser prompt — noisy in a desktop app and a dead end
    // in sandboxed WebViews; silently doing nothing is the honest behavior.
    try {
      const input = document.createElement("input");
      input.type = "file";
      input.accept = "image/png,image/jpeg,image/gif,image/webp";
      input.onchange = (e) => {
        const file = (e.target as HTMLInputElement).files?.[0];
        if (file) {
          const reader = new FileReader();
          reader.onload = (ev) => addImageFromUrl(ev.target?.result as string);
          reader.readAsDataURL(file);
        }
      };
      input.click();
    } catch (e) {
      // File pickers can fail in restricted environments; drop the image
      // insertion silently rather than prompting for a raw URL.
    }
  };

  const duplicateSlide = () => {
    pushSlideHistory();
    const source = activeSlide();
    if (!source) return;
    const copy: Slide = {
      ...source,
      id: generateSlideId("slide"),
      title: `${source.title} copy`,
      elements: JSON.parse(JSON.stringify(source.elements)).map((element: any) => ({ ...element, id: generateSlideId("el") })),
    };
    const next = [...slides()];
    next.splice(activeSlideIndex() + 1, 0, copy);
    setSlides(next);
    setActiveSlideIndex(activeSlideIndex() + 1);
    emitChange(next);
  };

  const moveSlide = (direction: -1 | 1) => {
    const from = activeSlideIndex();
    const to = from + direction;
    if (to < 0 || to >= slides().length) return;
    pushSlideHistory();
    const next = [...slides()];
    [next[from], next[to]] = [next[to], next[from]];
    setSlides(next);
    setActiveSlideIndex(to);
    emitChange(next);
  };

  const deleteSlide = () => {
    if (slides().length <= 1) return;
    pushSlideHistory();
    const next = slides().filter((_, index) => index !== activeSlideIndex());
    setSlides(next);
    setActiveSlideIndex(Math.min(activeSlideIndex(), next.length - 1));
    emitChange(next);
  };

  const deleteSelectedElement = () => {
    const ids = selectedElementIds();
    if (!ids.length) return;
    pushSlideHistory();
    const next = [...slides()];
    const index = activeSlideIndex();
    next[index] = { ...next[index], elements: next[index].elements.filter((el) => !ids.includes(el.id)) };
    setSlides(next);
    setSelectedElementIds([]);
    emitChange(next);
  };

  /** Write elements to clipboard — system clipboard with internal fallback. */
  const writeToClipboard = async (els: SlideElement[]) => {
    const payload = JSON.stringify(els);
    clipboardBuffer = payload;
    // Also write to system clipboard for cross-application copy/paste
    try {
      if (navigator.clipboard && typeof navigator.clipboard.writeText === "function") {
        await navigator.clipboard.writeText(payload);
      }
    } catch {
      // Clipboard API unavailable (e.g., insecure context) — internal buffer still works
    }
  };

  /** Read elements from clipboard — system clipboard with internal fallback. */
  const readFromClipboard = async (): Promise<SlideElement[] | null> => {
    // Try system clipboard first for cross-application paste
    try {
      if (navigator.clipboard && typeof navigator.clipboard.readText === "function") {
        const text = await navigator.clipboard.readText();
        if (text) {
          const els = JSON.parse(text) as SlideElement[];
          if (Array.isArray(els) && els.length > 0) return els;
        }
      }
    } catch {
      // Fall through to internal buffer
    }
    // Fall back to internal buffer
    if (clipboardBuffer) {
      const els = JSON.parse(clipboardBuffer) as SlideElement[];
      if (Array.isArray(els) && els.length > 0) return els;
    }
    return null;
  };

  /** Internal element clipboard (shared by shortcuts and toolbar buttons). */
  const copySelectedElements = () => {
    const els = getSelectedElements();
    if (els.length > 0) {
      void writeToClipboard(els);
    }
  };

  const pasteClippedElements = (inPlace: boolean = false) => {
    void readFromClipboard().then((els) => {
      if (!els || els.length === 0) return;
      pushSlideHistory();
      const newEls = els.map(el => ({
        ...el,
        id: `el-${Date.now()}-${Math.random()}`,
        x: el.x + (inPlace ? 0 : 20),
        y: el.y + (inPlace ? 0 : 20)
      }));
      const list = [...slides()];
      const slide = list[activeSlideIndex()];
      list[activeSlideIndex()] = { ...slide, elements: [...slide.elements, ...newEls] };
      setSlides(list);
      setSelectedElementIds(newEls.map(e => e.id));
      emitChange(list);
    });
  };

  const cutSelectedElements = () => {
    copySelectedElements();
    deleteSelectedElement();
  };


  // --- Coalesced undo-history pushes -------------------------------
  // Continuous interactions (keystrokes, nudges) push one snapshot per
  // interaction: the first call arms a timer, the timer commits the
  // boundary, and a later call after idle arms a new one.
  let historyCoalesceTimer: number | undefined;
  const pushSlideHistoryCoalesced = () => {
    if (historyCoalesceTimer !== undefined) return;
    historyCoalesceTimer = window.setTimeout(() => {
      historyCoalesceTimer = undefined;
    }, 500);
    pushSlideHistory();
  };

  const nudgeSelected = (dx: number, dy: number) => {
    pushSlideHistoryCoalesced();
    const id = selectedElementId();
    if (!id) return;
    const cw = canvasW();
    const ch = canvasH();
    setSlides(prev => prev.map((slide, i) => {
      if (i !== activeSlideIndex()) return slide;
      return {
        ...slide,
        elements: slide.elements.map(el => {
          if (el.id === id) {
            return {
              ...el,
              x: Math.max(0, Math.min(cw - el.width, el.x + dx)),
              y: Math.max(0, Math.min(ch - el.height, el.y + dy))
            };
          }
          return el;
        })
      };
    }));
    emitChange();
  };

  const openPresenter = () => {
    const deck = toDeck(slides(), props.initialContent, theme(), activeSlideIndex(), masters());
    stashPresenterDeckNow(deck, activeSlideIndex());
    void commands.openPresenterWindow().catch(() => {
      showToast("Could not open presenter window", "error");
    });
  };

  /** F5: fullscreen audience slideshow in this window, starting from the
   * current slide (Shift+F5 from the first). Esc exits. */
  const startSlideshow = (fromCurrent: boolean) => {
    if (!fromCurrent) selectSlide(0);
    setSlideshowDeck(toDeck(slides(), props.initialContent, theme(), activeSlideIndex(), masters()));
    setSlideshowOpen(true);
  };
  const exitSlideshow = () => setSlideshowOpen(false);

  const printSlides = () => {
    const perPage = printPerPage();
    const includeNotes = printIncludeNotes();
    const deck = slides();
    const bg = theme().bgColor;
    const escape = (value: string) => value.replace(/[&<>\"]/g, (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", "\"": "&quot;" }[character] || character));
    const renderElements = (slide: Slide) => slide.elements.map((element) => {
      const rot = element.rotation || 0;
      const base = `position:absolute;left:${element.x}px;top:${element.y}px;width:${element.width}px;height:${element.height}px;transform:rotate(${rot}deg);`;
      if (element.type === "text") {
        const align = element.align || "center";
        const text = element.bullets
          ? element.content.split("\n").map((line) => `• ${escape(line)}`).join("<br/>")
          : escape(element.content);
        const fw = element.bold ? "bold" : "normal";
        const fs = element.italic ? "italic" : "normal";
        const td = element.underline ? "underline" : "none";
        const ff = element.fontFamily || "Inter, sans-serif";
        return `<div style="${base}font-family:${ff};font-weight:${fw};font-style:${fs};text-decoration:${td};font-size:${element.fontSize || 20}px;color:${element.color || "#111827"};display:flex;align-items:center;justify-content:${align === "left" ? "flex-start" : align === "right" ? "flex-end" : "center"};text-align:${align}">${text}</div>`;
      }
      if (element.type === "image") return `<img src="${escape(element.content)}" alt="" style="${base}object-fit:contain"/>`;
      if (element.type === "line" || element.type === "arrow") {
        const color = escape(element.color || "#3b82f6");
        const defs = element.type === "arrow" ? `<defs><marker id="arrowhead-${element.id}" markerWidth="8" markerHeight="8" refX="7" refY="4" orient="auto"><path d="M0,0 L8,4 L0,8 z" fill="${color}"/></marker></defs>` : "";
        const markerEnd = element.type === "arrow" ? ` marker-end="url(#arrowhead-${element.id})"` : "";
        return `<svg viewBox="0 0 100 100" preserveAspectRatio="none" style="${base}">${defs}<line x1="0" y1="0" x2="100" y2="100" stroke="${color}" stroke-width="4" vector-effect="non-scaling-stroke"${markerEnd}/></svg>`;
      }
      const clip = shapeClipPath(element.type);
      const radius = shapeBorderRadius(element.type);
      const styleExt = (clip ? `clip-path:${clip};` : "") + (radius ? `border-radius:${radius};` : "");
      const textOverlay = element.content ? `<div style="position:absolute;inset:0;display:flex;align-items:center;justify-content:center;text-align:center;font-family:${element.fontFamily || 'Inter, sans-serif'};font-weight:${element.bold?'bold':'normal'};font-style:${element.italic?'italic':'normal'};text-decoration:${element.underline?'underline':'none'};font-size:${element.fontSize || 20}px;color:${element.color && element.color.toLowerCase() === '#ffffff' ? '#000' : '#fff'};white-space:pre-wrap;padding:8px">${escape(element.content)}</div>` : "";
      return `<div style="${base}background:${element.color || "#3b82f6"};${styleExt}">${textOverlay}</div>`;
    }).join("");

    const cols = perPage === 1 ? 1 : 2;
    const rows = perPage === 1 ? 1 : perPage === 2 ? 2 : perPage === 4 ? 2 : 3;
    const scale = perPage === 1 ? 0.72 : perPage === 2 ? 0.48 : perPage === 4 ? 0.36 : 0.28;
    const cw = canvasW();
    const ch = canvasH();
    const cellW = Math.round(cw * scale);
    const cellH = Math.round(ch * scale);
    const pages: string[] = [];
    for (let i = 0; i < deck.length; i += perPage) {
      const chunk = deck.slice(i, i + perPage);
      const cells = chunk.map((slide, offset) => {
        const num = i + offset + 1;
        const notes = includeNotes && slide.notes
          ? `<div class="notes">${escape(slide.notes)}</div>`
          : "";
        return `<div class="cell"><div class="label">${num}</div><div class="frame" style="width:${cellW}px;height:${cellH}px"><div class="slide" style="background:${escape(bg)};transform:scale(${scale})">${renderElements(slide)}</div></div>${notes}</div>`;
      }).join("");
      pages.push(`<section class="page" style="grid-template-columns:repeat(${cols},1fr);grid-template-rows:repeat(${rows},auto)">${cells}</section>`);
    }

    const html = `<!doctype html><html><head><title>Print Slides</title>
<style>
  @page{margin:12mm}
  body{margin:0;font:12px Inter,system-ui,sans-serif;color:#111827;background:#fff}
  .page{display:grid;gap:14px;page-break-after:always;align-content:start;padding:8px;box-sizing:border-box;min-height:100vh}
  .page:last-child{page-break-after:auto}
  .cell{display:flex;flex-direction:column;gap:6px;break-inside:avoid}
  .label{font-size:11px;color:#64748b}
  .frame{overflow:hidden;border:1px solid #cbd5e1;background:#fff}
  .slide{position:relative;width:${cw}px;height:${ch}px;transform-origin:top left}
  .notes{font-size:11px;line-height:1.35;color:#334155;white-space:pre-wrap}
  @media print{body{margin:0}.page{min-height:auto}}
</style></head><body>${pages.join("")}
<script>window.onload=()=>{window.print();setTimeout(()=>window.close(),300)}</script>
</body></html>`;
    const win = window.open("", "_blank", "noopener,noreferrer,width=1000,height=800");
    if (!win) return;
    win.document.open();
    win.document.write(html);
    win.document.close();
    setPrintDialogOpen(false);
  };

  const selectSlide = (idx: number) => {
    const incoming = slides()[idx];
    const transition = incoming?.transition || "none";
    setActiveSlideIndex(idx);
    setSelectedElementId(null);
    setSlideAnimClass("");
    if (transition !== "none") {
      queueMicrotask(() => setSlideAnimClass(transition));
      window.setTimeout(() => setSlideAnimClass(""), 450);
    }
    if (props.onSlideInfoChange) {
      props.onSlideInfoChange(`Slide ${idx + 1} of ${slides().length}`);
    }
  };

  onMount(() => {
    if (filmstripEl) {
      setFilmstripScroll({ top: filmstripEl.scrollTop, height: filmstripEl.clientHeight });
    }
    const onCommand = (event: Event) => {
      const detail = (event as CustomEvent<EditorCommandDetail>).detail;
      if (!detail?.id) return;
      if (detail.id === "duplicate-slide") duplicateSlide();
      else if (detail.id === "new-slide") addSlide();
      else if (detail.id === "delete-slide") deleteSlide();
      else if (detail.id === "present") openPresenter();
      else if (detail.id === "insert-image") addImage();
      else if (detail.id === "insert-textbox") addTextBox();
      else if (detail.id === "print") setPrintDialogOpen(true);
      else if (detail.id === "undo") undoSlide();
      else if (detail.id === "redo") redoSlide();
      else if (detail.id === "slide-layout") setSidebarPanel("master");
    };
    window.addEventListener(EDITOR_COMMAND, onCommand);

      const onKeyDown = (e: KeyboardEvent) => {
      const activeEl = document.activeElement;
      if (activeEl && (activeEl.tagName === "INPUT" || activeEl.tagName === "TEXTAREA" || activeEl.hasAttribute("contenteditable"))) {
        return;
      }

      if (e.ctrlKey || e.metaKey) {
        if (e.key === 'c' || e.key === 'C') {
          copySelectedElements();
          return;
        } else if (e.key === 'v' || e.key === 'V') {
          pasteClippedElements(false);
          e.preventDefault();
          return;
        } else if (e.key === 'x' || e.key === 'X') {
          cutSelectedElements();
          e.preventDefault();
          return;
        } else if (e.key === 'd' || e.key === 'D') {
          copySelectedElements();
          pasteClippedElements(true);
          e.preventDefault();
          return;
        }
      }

      if (e.key === "Delete" || e.key === "Backspace") {
        if (selectedElementIds().length) {
          deleteSelectedElement();
          e.preventDefault();
        }
      }

      if (e.key === "PageDown") {
        const to = activeSlideIndex() + 1;
        if (to < slides().length) selectSlide(to);
        e.preventDefault();
      } else if (e.key === "PageUp") {
        const to = activeSlideIndex() - 1;
        if (to >= 0) selectSlide(to);
        e.preventDefault();
      } else if (e.key === "ArrowLeft" || e.key === "ArrowRight" || e.key === "ArrowUp" || e.key === "ArrowDown") {
        if (selectedElementIds().length) {
          const amount = e.shiftKey ? 10 : 1;
          if (e.key === "ArrowLeft") nudgeSelected(-amount, 0);
          else if (e.key === "ArrowRight") nudgeSelected(amount, 0);
          else if (e.key === "ArrowUp") nudgeSelected(0, -amount);
          else if (e.key === "ArrowDown") nudgeSelected(0, amount);
          e.preventDefault();
        }
      }
    };
    window.addEventListener("keydown", onKeyDown);

    // Slideshow keys run on a capture-phase listener with preventDefault so
    // editor shortcuts never fire while the fullscreen overlay is open.
    const onSlideshowKey = (e: KeyboardEvent) => {
      if (!slideshowOpen()) return;
      if (e.key === "F5" || e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        exitSlideshow();
      }
    };
    window.addEventListener("keydown", onSlideshowKey, true);

    const slideShortcutIds = ["slide-present-f5", "slide-undo", "slide-redo"];
    shortcutRegistry.register({
      id: "slide-present-f5",
      title: "Start Presentation",
      shortcut: "F5",
      mode: "slide",
      action: () => startSlideshow(true),
    });
    shortcutRegistry.register({
      id: "slide-undo",
      title: "Undo",
      shortcut: "Ctrl+Z",
      mode: "slide",
      action: () => emitEditorCommand("undo"),
    });
    shortcutRegistry.register({
      id: "slide-redo",
      title: "Redo",
      shortcut: "Ctrl+Y",
      mode: "slide",
      action: () => emitEditorCommand("redo"),
    });

    let unlistenSlide: (() => void) | undefined;
    import("@tauri-apps/api/event").then((mod) => {
      mod.listen("slide_changed", (event: { payload?: { slideIndex?: number } }) => {
        const idx = event.payload?.slideIndex;
        if (typeof idx !== "number" || idx < 0 || idx >= slides().length) return;
        if (idx === activeSlideIndex()) return;
        selectSlide(idx);
      }).then((fn: () => void) => { unlistenSlide = fn; });
    }).catch(() => undefined);

    onCleanup(() => {
      window.removeEventListener(EDITOR_COMMAND, onCommand);
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("keydown", onSlideshowKey, true);
      slideShortcutIds.forEach((id) => shortcutRegistry.unregister(id));
      unlistenSlide?.();
    });
  });

  const fieldLabel = (label: string, children: JSX.Element) => (
    <label style={{ display: "flex", "flex-direction": "column", gap: "2px", "font-size": "11px" }}>
      <span>{label}</span>
      {children}
    </label>
  );

  const numInput = (value: number, onCommit: (n: number) => void) => (
    <input
      type="number"
      value={Math.round(value)}
      style={{ width: "100%", background: "var(--bg-input, #2a2a2a)", border: "1px solid var(--border-color)", color: "var(--text-primary)", padding: "2px 4px", "font-size": "12px" }}
      onChange={(e) => onCommit(Number(e.currentTarget.value) || 0)}
    />
  );

  const sidebarPanels = (): SidebarPanel[] => [
    {
      id: "properties",
      title: "Properties",
      icon: <IconProperties />,
      content: (
        <Show
          when={selectedElement()}
          fallback={<div>Select an element on the slide.</div>}
        >
          {(el) => (
            <div style={{ display: "flex", "flex-direction": "column", gap: "8px" }}>
              <div><strong>Type</strong>: {el().type}</div>
              {fieldLabel("X", numInput(el().x, (n) => patchSelected({ x: Math.max(0, Math.min(canvasW() - el().width, n)) })))}
              {fieldLabel("Y", numInput(el().y, (n) => patchSelected({ y: Math.max(0, Math.min(canvasH() - el().height, n)) })))}
              {fieldLabel("Width", numInput(el().width, (n) => patchSelected({ width: Math.max(40, Math.min(canvasW() - el().x, n)) })))}
              {fieldLabel("Height", numInput(el().height, (n) => patchSelected({ height: Math.max(30, Math.min(canvasH() - el().y, n)) })))}
              {fieldLabel(
                "Rotation",
                <div style={{ display: "flex", gap: "4px", "align-items": "center" }}>
                  <button type="button" class="g-toolbar-btn" title="Rotate -15°" onClick={() => rotateSelected(-15)}>−</button>
                  {numInput(el().rotation || 0, (n) => patchSelected({ rotation: ((n % 360) + 360) % 360 }))}
                  <button type="button" class="g-toolbar-btn" title="Rotate +15°" onClick={() => rotateSelected(15)}>+</button>
                </div>,
              )}
              {fieldLabel(
                "Hyperlink",
                <div style={{ display: "flex", "flex-direction": "column", gap: "4px" }}>
                  <input
                    aria-label="Element hyperlink"
                    placeholder="https://example.com"
                    value={hyperlinkDraft()}
                    onInput={(e) => {
                      setHyperlinkDraft(e.currentTarget.value);
                      setHyperlinkError(null);
                    }}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") {
                        e.preventDefault();
                        applySelectedHyperlink();
                      }
                    }}
                    style={{ width: "100%", background: "var(--bg-input, #2a2a2a)", border: "1px solid var(--border-color)", color: "var(--text-primary)", padding: "3px 4px", "font-size": "11px" }}
                  />
                  <div style={{ display: "flex", gap: "4px" }}>
                    <button type="button" class="g-toolbar-btn" onClick={() => applySelectedHyperlink()}>Apply</button>
                    <button type="button" class="g-toolbar-btn" onClick={() => applySelectedHyperlink("")}>Clear</button>
                  </div>
                  <Show when={hyperlinkError()}>
                    <span role="alert" style={{ color: "var(--text-danger, #ef4444)", "font-size": "10px" }}>{hyperlinkError()}</span>
                  </Show>
                  <span style={{ color: "var(--text-muted)", "font-size": "10px" }}>{SLIDE_HYPERLINK_HINT}. Ctrl/Cmd-click opens it.</span>
                </div>,
              )}
              <Show when={el().type === "text"}>
                {fieldLabel(
                  "Font Family",
                  <select
                    aria-label="Font family"
                    value={el().fontFamily || "Inter, sans-serif"}
                    onChange={(e) => patchSelected({ fontFamily: e.currentTarget.value })}
                    style={{ width: "100%", background: "var(--bg-input, #2a2a2a)", border: "1px solid var(--border-color)", color: "var(--text-primary)", padding: "2px 4px" }}
                  >
                    <option value="Inter, sans-serif">Inter</option>
                    <option value="Arial, sans-serif">Arial</option>
                    <option value="'Courier New', monospace">Courier New</option>
                    <option value="'Times New Roman', serif">Times New Roman</option>
                    <option value="Georgia, serif">Georgia</option>
                    <option value="Verdana, sans-serif">Verdana</option>
                  </select>
                )}
                {fieldLabel("Font size", numInput(el().fontSize || 20, (n) => patchSelected({ fontSize: Math.max(8, Math.min(96, n)) })))}
                <div style={{ display: "flex", gap: "4px" }}>
                  <button type="button" class={`g-toolbar-btn ${el().bold ? 'active' : ''}`} title="Bold" onClick={() => patchSelected({ bold: !el().bold })} style={el().bold ? { "background-color": "var(--bg-active, #3a3a3a)" } : {}}><IconBold /></button>
                  <button type="button" class={`g-toolbar-btn ${el().italic ? 'active' : ''}`} title="Italic" onClick={() => patchSelected({ italic: !el().italic })} style={el().italic ? { "background-color": "var(--bg-active, #3a3a3a)" } : {}}><IconItalic /></button>
                  <button type="button" class={`g-toolbar-btn ${el().underline ? 'active' : ''}`} title="Underline" onClick={() => patchSelected({ underline: !el().underline })} style={el().underline ? { "background-color": "var(--bg-active, #3a3a3a)" } : {}}><IconUnderline /></button>
                </div>
                {fieldLabel(
                  "Align",
                  <select
                    aria-label="Text align"
                    value={el().align || "center"}
                    onChange={(e) => patchSelected({ align: e.currentTarget.value as "left" | "center" | "right" })}
                    style={{ width: "100%", background: "var(--bg-input, #2a2a2a)", border: "1px solid var(--border-color)", color: "var(--text-primary)", padding: "2px 4px" }}
                  >
                    <option value="left">Left</option>
                    <option value="center">Center</option>
                    <option value="right">Right</option>
                  </select>,
                )}
                <label style={{ display: "flex", "align-items": "center", gap: "8px", cursor: "pointer", "font-size": "11px" }}>
                  <input
                    type="checkbox"
                    checked={Boolean(el().bullets)}
                    onChange={(e) => patchSelected({ bullets: e.currentTarget.checked })}
                  />
                  Bullets
                </label>
              </Show>
              <Show when={el().type === "table"}>
                <div style={{ display: "flex", "flex-direction": "column", gap: "4px", "margin-top": "8px", "margin-bottom": "8px" }}>
                  <div style={{ "font-size": "11px", color: "var(--text-secondary)" }}>Table Controls</div>
                  <div style={{ display: "flex", gap: "4px" }}>
                    <button type="button" class="g-toolbar-btn" onClick={() => {
                      const td = el().tableData;
                      if (!td) return;
                      const cols = td[0]?.length || 1;
                      const newRow = Array.from({ length: cols }, () => "");
                      const newData = [...td, newRow];
                      patchSelected({ tableData: newData, tableRows: newData.length });
                    }}>Add Row</button>
                    <button type="button" class="g-toolbar-btn" onClick={() => {
                      const td = el().tableData;
                      if (!td || td.length <= 1) return;
                      const newData = td.slice(0, -1);
                      patchSelected({ tableData: newData, tableRows: newData.length });
                    }}>Remove Row</button>
                  </div>
                  <div style={{ display: "flex", gap: "4px" }}>
                    <button type="button" class="g-toolbar-btn" onClick={() => {
                      const td = el().tableData;
                      if (!td) return;
                      const newData = td.map(r => [...r, ""]);
                      patchSelected({ tableData: newData, tableCols: newData[0]?.length || 1 });
                    }}>Add Col</button>
                    <button type="button" class="g-toolbar-btn" onClick={() => {
                      const td = el().tableData;
                      if (!td || (td[0]?.length || 0) <= 1) return;
                      const newData = td.map(r => r.slice(0, -1));
                      patchSelected({ tableData: newData, tableCols: newData[0]?.length || 1 });
                    }}>Remove Col</button>
                  </div>
                </div>
              </Show>
              <Show when={el().type === "chart"}>
                {fieldLabel(
                  "Chart type",
                  <select
                    aria-label="Chart type"
                    value={el().chartType || "bar"}
                    onChange={(e) => patchSelected({ chartType: e.currentTarget.value as ChartType })}
                    style={{ width: "100%", background: "var(--bg-input, #2a2a2a)", border: "1px solid var(--border-color)", color: "var(--text-primary)", padding: "2px 4px" }}
                  >
                    <For each={CHART_TYPES}>
                      {(type) => <option value={type}>{type}</option>}
                    </For>
                  </select>,
                )}
              </Show>
              {fieldLabel(
                "Fill color",
                <input
                  type="color"
                  value={el().color || "#3b82f6"}
                  onInput={(e) => updateSelectedColor(e.currentTarget.value)}
                  style={{ width: "100%", height: "28px", border: "none", background: "transparent", cursor: "pointer" }}
                />,
              )}
            </div>
          )}
        </Show>
      ),
    },
    {
      id: "transition",
      title: "Transition",
      icon: <IconTransition />,
      content: (
        <div style={{ display: "flex", "flex-direction": "column", gap: "8px" }}>
          <label style={{ "font-size": "11px" }}>
            Slide transition
            <select
              aria-label="Slide transition"
              value={activeSlide()?.transition || "none"}
              onChange={(e) => setSlideTransition(normalizeTransition(e.currentTarget.value))}
              style={{ display: "block", width: "100%", "margin-top": "4px", background: "var(--bg-input, #2a2a2a)", border: "1px solid var(--border-color)", color: "var(--text-primary)", padding: "4px" }}
            >
              <option value="none">None</option>
              <option value="fade">Fade</option>
              <option value="slide-left">Slide left</option>
              <option value="slide-right">Slide right</option>
              <option value="wipe-left">Wipe left</option>
              <option value="wipe-right">Wipe right</option>
              <option value="zoom">Zoom</option>
              <option value="dissolve">Dissolve</option>
              <option value="morph">Morph</option>
            </select>
          </label>
          <label style={{ "font-size": "11px" }}>
            Background override
            <div style={{ display: "flex", gap: "6px", "margin-top": "4px", "align-items": "center" }}>
              <input
                type="color"
                aria-label="Slide background override"
                value={activeSlide()?.bgOverride || theme().bgColor}
                onInput={(e) => setSlideBgOverride(e.currentTarget.value)}
                style={{ width: "36px", height: "28px", border: "none", background: "transparent", cursor: "pointer" }}
              />
              <button type="button" class="g-toolbar-btn" onClick={() => setSlideBgOverride(null)}>Use theme</button>
            </div>
          </label>
          <div style={{ "font-size": "11px", color: "var(--text-muted)" }}>
            Applied when changing to this slide in the editor and presenter.
          </div>
        </div>
      ),
    },
    {
      id: "animation",
      title: "Animation",
      icon: <IconAnimation />,
      content: (
        <div style={{ display: "flex", "flex-direction": "column", gap: "10px" }}>
          <div style={{ "font-size": "11px", color: "var(--text-muted)" }}>
            Animated elements on this slide reveal in the listed order in presenter view.
          </div>
          <div style={{ display: "flex", "flex-direction": "column", gap: "4px" }}>
            <For each={activeSlide()?.elements
              .filter((e) => e.entrance !== "none" || e.exit !== "none")
              .slice()
              .sort((a, b) => (a.entranceOrder ?? 0) - (b.entranceOrder ?? 0)) ?? []}
            >
              {(el, index) => (
                <div
                  style={{
                    display: "flex",
                    "align-items": "center",
                    gap: "4px",
                    padding: "3px 4px",
                    "border-radius": "4px",
                    background: selectedElementIds().includes(el.id) ? "var(--bg-selected, #505050)" : "var(--bg-tertiary)",
                  }}
                >
                  <span
                    style={{ "font-size": "11px", flex: 1, overflow: "hidden", "text-overflow": "ellipsis", "white-space": "nowrap", cursor: "pointer" }}
                    title={`${el.entrance !== "none" ? `Entrance: ${el.entrance}` : ""}${el.entrance !== "none" && el.exit !== "none" ? " · " : ""}${el.exit !== "none" ? `Exit: ${el.exit}` : ""}`}
                    onClick={() => setSelectedElementId(el.id)}
                  >
                    {index() + 1}. {el.type}
                  </span>
                  <button
                    type="button"
                    class="g-toolbar-btn"
                    aria-label={`Move ${el.type} animation earlier`}
                    title="Play earlier"
                    disabled={index() === 0}
                    style={{ width: "22px", height: "20px", padding: 0, "font-size": "10px" }}
                    onClick={() => reorderAnimation(el.id, -1)}
                  >
                    ↑
                  </button>
                  <button
                    type="button"
                    class="g-toolbar-btn"
                    aria-label={`Move ${el.type} animation later`}
                    title="Play later"
                    disabled={index() === (activeSlide()?.elements.filter((e) => e.entrance !== "none" || e.exit !== "none").length ?? 0) - 1}
                    style={{ width: "22px", height: "20px", padding: 0, "font-size": "10px" }}
                    onClick={() => reorderAnimation(el.id, 1)}
                  >
                    ↓
                  </button>
                </div>
              )}
            </For>
            <Show when={(activeSlide()?.elements.filter((e) => e.entrance !== "none" || e.exit !== "none").length ?? 0) === 0}>
              <div style={{ "font-size": "11px", color: "var(--text-muted)" }}>
                No animated elements yet. Select an element to add an entrance or exit effect.
              </div>
            </Show>
          </div>
          <Show
            when={selectedElement()}
            fallback={<div style={{ "font-size": "11px", color: "var(--text-muted)" }}>Select an element to set its animation.</div>}
          >
            {(el) => (
              <div style={{ display: "flex", "flex-direction": "column", gap: "8px", "border-top": "1px solid var(--border-color)", "padding-top": "8px" }}>
                <div style={{ "font-size": "12px" }}>
                  <strong>{el().type}</strong>
                  <div style={{ "font-size": "11px", color: "var(--text-muted)", "margin-top": "2px" }}>{el().id}</div>
                </div>
                <label style={{ "font-size": "11px" }}>
                  Entrance effect
                  <select
                    aria-label="Entrance animation effect"
                    value={el().entrance || "none"}
                    onChange={(e) => patchSelected({ entrance: e.currentTarget.value as ElementEntrance })}
                    style={{ display: "block", width: "100%", "margin-top": "3px" }}
                  >
                    <option value="none">None</option>
                    <option value="fade">Fade</option>
                    <option value="zoom">Zoom</option>
                  </select>
                </label>
                <label style={{ "font-size": "11px" }}>
                  Exit effect
                  <select
                    aria-label="Exit animation effect"
                    value={el().exit || "none"}
                    onChange={(e) => patchSelected({ exit: e.currentTarget.value as ElementExit })}
                    style={{ display: "block", width: "100%", "margin-top": "3px" }}
                  >
                    <option value="none">None</option>
                    <option value="fade">Fade out on slide exit</option>
                  </select>
                </label>
                <label style={{ "font-size": "11px" }}>
                  Entrance delay (ms)
                  <input
                    type="number"
                    min="0"
                    max="60000"
                    step="50"
                    aria-label="Animation delay milliseconds"
                    value={el().entranceDelayMs ?? 0}
                    onInput={(e) => patchSelected({ entranceDelayMs: Math.max(0, Math.min(60000, Number(e.currentTarget.value) || 0)) })}
                    style={{ display: "block", width: "100%", "margin-top": "3px" }}
                  />
                </label>
                <label style={{ "font-size": "11px" }}>
                  Entrance duration (ms)
                  <input
                    type="number"
                    min="50"
                    max="60000"
                    step="50"
                    aria-label="Animation duration milliseconds"
                    value={el().entranceDurationMs ?? 350}
                    onInput={(e) => patchSelected({ entranceDurationMs: Math.max(50, Math.min(60000, Number(e.currentTarget.value) || 350)) })}
                    style={{ display: "block", width: "100%", "margin-top": "3px" }}
                  />
                </label>
                <label style={{ "font-size": "11px" }}>
                  Exit duration (ms)
                  <input
                    type="number"
                    min="50"
                    max="60000"
                    step="50"
                    aria-label="Exit duration milliseconds"
                    value={el().exitDurationMs ?? 350}
                    onInput={(e) => patchSelected({ exitDurationMs: Math.max(50, Math.min(60000, Number(e.currentTarget.value) || 350)) })}
                    style={{ display: "block", width: "100%", "margin-top": "3px" }}
                  />
                </label>
                <div style={{ "font-size": "11px", color: "var(--text-muted)" }}>
                  In presenter view, entrance-animated elements start hidden and reveal in the listed order with Space or click. Exit effects play when leaving the slide.
                </div>
              </div>
            )}
          </Show>
        </div>
      ),
    },
    {
      id: "master",
      title: "Master",
      icon: <IconMaster />,
      content: (
        <div style={{ display: "flex", "flex-direction": "column", gap: "4px" }}>
          <div style={{ "font-size": "11px", color: "var(--text-muted)", "margin-bottom": "4px" }}>
            Apply a layout master to the current slide.
          </div>
          {(
            [
              ["title", "Title slide"],
              ["title-body", "Title and body"],
              ["section", "Section header"],
              ["two-column", "Two columns"],
              ["image-caption", "Image + caption"],
              ["blank", "Blank"],
            ] as const
          ).map(([id, label]) => (
            <button
              type="button"
              class="g-toolbar-btn"
              style={{
                "justify-content": "flex-start",
                width: "100%",
                height: "auto",
                padding: "4px 6px",
                background: activeSlide()?.layout === id ? "var(--bg-selected, #505050)" : "transparent",
              }}
              onClick={() => applyLayout(id)}
            >
              {label}
            </button>
          ))}
        </div>
      ),
    },
    {
      id: "styles",
      title: "Styles",
      icon: <IconStyles />,
      content: (
        <div style={{ display: "flex", "flex-direction": "column", gap: "4px" }}>
          {(
            [
              ["light", "Modern Light"],
              ["midnight", "Midnight"],
              ["coral", "Coral"],
              ["forest", "Forest"],
              ["lavender", "Lavender"],
              ["sunset", "Sunset"],
            ] as const
          ).map(([id, label]) => (
            <button
              type="button"
              class="g-toolbar-btn"
              style={{
                "justify-content": "flex-start",
                width: "100%",
                height: "auto",
                padding: "4px 6px",
                background: theme().id === id || (id === "light" && theme().id === "default-light") ? "var(--bg-selected, #505050)" : "transparent",
              }}
              onClick={() => setThemePreset(id)}
            >
              {label}
            </button>
          ))}
        </div>
      ),
    },
    {
      id: "gallery",
      title: "Gallery",
      icon: <IconGallery />,
      content: <div>Insert images via the Standard toolbar. Gallery media packs are not bundled offline.</div>,
    },
    {
      id: "navigator",
      title: "Navigator",
      icon: <IconNavigator />,
      content: (
        <div style={{ display: "flex", "flex-direction": "column", gap: "2px" }}>
          <For each={slides()}>
            {(slide, idx) => (
              <button
                type="button"
                class="g-toolbar-btn"
                style={{
                  "justify-content": "flex-start",
                  width: "100%",
                  height: "auto",
                  padding: "4px 6px",
                  background: idx() === activeSlideIndex() ? "var(--bg-selected, #505050)" : "transparent",
                }}
                onClick={() => selectSlide(idx())}
              >
                {idx() + 1}. {slide.elements.find((e) => e.type === "text")?.content || slide.title}
              </button>
            )}
          </For>
        </div>
      ),
    },
  ];

  return (
    <div style={{ display: "flex", "flex-direction": "column", height: "100%", background: "var(--bg-canvas)", overflow: "hidden" }}>
      {/* Standard toolbar */}
      <ToolbarRow>
        <ToolbarButton title="New" onClick={() => props.onRequestNew?.()}><IconNew /></ToolbarButton>
        <ToolbarButton title="Open" onClick={() => props.onRequestOpen?.()}><IconFolderOpen /></ToolbarButton>
        <ToolbarButton title="Save" onClick={() => props.onRequestSave?.()}><IconSave /></ToolbarButton>
        <ToolbarButton title="Export PDF" onClick={() => props.onRequestExportPdf?.()}><IconPdf /></ToolbarButton>
        <ToolbarButton title="Export as PPTX (.pptx)" onClick={() => props.onRequestExportPptx?.()}>PPTX</ToolbarButton>
        <ToolbarButton
          title="Export current slide as PNG"
          onClick={() => {
            const slide = activeSlide();
            if (!slide) return;
            void renderSlideToPng({
              elements: slide.elements as any,
              bg: slide.bgOverride || theme().bgColor || "#ffffff",
            }, { canvasWidth: canvasW(), canvasHeight: canvasH() })
              .then((blob) => downloadBlob(blob, `slide-${activeSlideIndex() + 1}.png`))
              .catch(() => showToast("Could not export slide as PNG", "error"));
          }}
        >
          <IconImage />
        </ToolbarButton>
        <ToolbarButton title="Print…" onClick={() => setPrintDialogOpen(true)}><IconPrint /></ToolbarButton>
        <ToolbarSep />
        <ToolbarButton title="Cut" onClick={() => cutSelectedElements()}><IconCut /></ToolbarButton>
        <ToolbarButton title="Copy" onClick={() => copySelectedElements()}><IconCopy /></ToolbarButton>
        <ToolbarButton title="Paste" onClick={() => pasteClippedElements(false)}><IconPaste /></ToolbarButton>
        <ToolbarSep />
        <ToolbarButton title="Undo" onClick={undoSlide}><IconUndo /></ToolbarButton>
        <ToolbarButton title="Redo" onClick={redoSlide}><IconRedo /></ToolbarButton>
        <ToolbarSep />
        <ToolbarButton title="New slide" onClick={addSlide}><IconPlus /></ToolbarButton>
        <ToolbarButton title="Duplicate slide" onClick={duplicateSlide}><IconDuplicate /></ToolbarButton>
        <ToolbarButton title="Delete slide" onClick={deleteSlide}><IconTrash /></ToolbarButton>
        <ToolbarButton title="Move slide up" onClick={() => moveSlide(-1)}><IconChevronUp /></ToolbarButton>
        <ToolbarButton title="Move slide down" onClick={() => moveSlide(1)}><IconChevronDown /></ToolbarButton>
        <ToolbarSep />
        <ToolbarButton title="Insert Table" onClick={addTable}><IconTable /></ToolbarButton>
        <ToolbarButton title="Insert Chart" onClick={addChart}><IconChart /></ToolbarButton>
        <ToolbarButton title="Insert Image" onClick={addImage}><IconImage /></ToolbarButton>
        <ToolbarButton title="Insert Text Box" onClick={addTextBox}><IconTextBox /></ToolbarButton>
        <ToolbarSep />
        <ToolbarButton title="Zoom out" onClick={() => setZoom((z) => Math.max(50, z - 10))}>−</ToolbarButton>
        <span style={{ "font-size": "11px", color: "var(--text-secondary)", "min-width": "36px", "text-align": "center" }}>{zoom()}%</span>
        <ToolbarButton title="Zoom in" onClick={() => setZoom((z) => Math.min(200, z + 10))}>+</ToolbarButton>
        <ToolbarSep />
        <ToolbarButton title="Present" onClick={openPresenter}><IconPresent /></ToolbarButton>
        <ToolbarButton title="Present from Beginning (Shift+F5)" onClick={() => startSlideshow(false)}>▶|</ToolbarButton>
      </ToolbarRow>

      {/* Drawing toolbar */}
      <ToolbarRow>
        <ToolbarButton title="Select" active={drawTool() === "select"} onClick={() => setDrawTool("select")}><IconSelect /></ToolbarButton>
        <ToolbarButton
          title="Line"
          active={drawTool() === "line"}
          onClick={() => { setDrawTool("line"); addShape("line"); }}
        >
          <IconLine />
        </ToolbarButton>
        <ToolbarButton
          title="Rectangle"
          active={drawTool() === "rect"}
          onClick={() => { setDrawTool("rect"); addShape("rect"); }}
        >
          <IconRect />
        </ToolbarButton>
        <ToolbarButton
          title="Ellipse"
          active={drawTool() === "ellipse"}
          onClick={() => { setDrawTool("ellipse"); addShape("ellipse"); }}
        >
          <IconEllipse />
        </ToolbarButton>
        <ToolbarButton
          title="Rounded rectangle"
          active={drawTool() === "roundedRect"}
          onClick={() => { setDrawTool("roundedRect"); addShape("roundedRect"); }}
        >
          ◢
        </ToolbarButton>
        <ToolbarButton title="Triangle" active={drawTool() === "triangle"} onClick={() => { setDrawTool("triangle"); addShape("triangle"); }}>△</ToolbarButton>
        <ToolbarButton title="Diamond" active={drawTool() === "diamond"} onClick={() => { setDrawTool("diamond"); addShape("diamond"); }}>◇</ToolbarButton>
        <ToolbarButton title="Star" active={drawTool() === "star"} onClick={() => { setDrawTool("star"); addShape("star"); }}>★</ToolbarButton>
        <ToolbarButton
          title="Arrow"
          active={drawTool() === "arrow"}
          onClick={() => { setDrawTool("arrow"); addShape("arrow"); }}
        >
          <IconArrow />
        </ToolbarButton>
        <ToolbarButton
          title="Text Box"
          active={drawTool() === "text"}
          onClick={addTextBox}
        >
          <IconTextBox />
        </ToolbarButton>
        <ToolbarSep />
        <ToolbarButton disabled={selectedElement()?.type !== "text"} title="Bold" active={Boolean(selectedElement()?.bold)} onClick={() => patchSelected({ bold: !selectedElement()?.bold })}><IconBold /></ToolbarButton>
        <ToolbarButton disabled={selectedElement()?.type !== "text"} title="Italic" active={Boolean(selectedElement()?.italic)} onClick={() => patchSelected({ italic: !selectedElement()?.italic })}><IconItalic /></ToolbarButton>
        <ToolbarButton disabled={selectedElement()?.type !== "text"} title="Underline" active={Boolean(selectedElement()?.underline)} onClick={() => patchSelected({ underline: !selectedElement()?.underline })}><IconUnderline /></ToolbarButton>
        <ToolbarSep />
        <ToolbarColor
          title="Fill Color"
          value={selectedElement()?.color || fillColor()}
          onChange={(c) => updateSelectedColor(c)}
        >
          <IconRect />
          <span style={{ width: "14px", height: "3px", background: selectedElement()?.color || fillColor(), display: "block", "margin-top": "-2px" }} />
        </ToolbarColor>
        <ToolbarColor
          title="Line Color"
          value={selectedElement()?.color || lineColor()}
          onChange={(c) => updateSelectedColor(c)}
        >
          <IconTextColor />
          <span style={{ width: "14px", height: "3px", background: selectedElement()?.color || lineColor(), display: "block", "margin-top": "-2px" }} />
        </ToolbarColor>
        <ToolbarSep />
        <ToolbarButton title="Align left" onClick={() => alignSelected("left")}>⫷</ToolbarButton>
        <ToolbarButton title="Align center" onClick={() => alignSelected("center")}>⫿</ToolbarButton>
        <ToolbarButton title="Align right" onClick={() => alignSelected("right")}>⫸</ToolbarButton>
        <ToolbarButton title="Align top" onClick={() => alignSelected("top")}>⫶</ToolbarButton>
        <ToolbarButton title="Align middle" onClick={() => alignSelected("middle")}>⫯</ToolbarButton>
        <ToolbarButton title="Align bottom" onClick={() => alignSelected("bottom")}>⫷</ToolbarButton>
        <ToolbarButton title="Distribute horizontally" onClick={() => distributeSelected("h")}>⇹</ToolbarButton>
        <ToolbarButton title="Distribute vertically" onClick={() => distributeSelected("v")}>⇳</ToolbarButton>
        <ToolbarButton title="Group" onClick={groupSelected} disabled={selectedElementIds().length < 2}>Group</ToolbarButton>
        <ToolbarButton title="Ungroup" onClick={ungroupSelected} disabled={!selectedElementIds().length}>Ungroup</ToolbarButton>
        <ToolbarSep />
        <ToolbarButton title="Bring to Front" onClick={() => reorderSelected(true)} ariaLabel="Bring to Front">
          <span style={{ "font-size": "10px", "font-weight": "700" }}>⤒</span>
        </ToolbarButton>
        <ToolbarButton title="Send to Back" onClick={() => reorderSelected(false)} ariaLabel="Send to Back">
          <span style={{ "font-size": "10px", "font-weight": "700" }}>⤓</span>
        </ToolbarButton>
        <ToolbarSep />
        <ToolbarSelect
          ariaLabel="Slide layout"
          width="130px"
          value={activeSlide()?.layout || "blank"}
          onChange={applyLayout}
          options={[
            { value: "title", label: "Title slide" },
            { value: "title-body", label: "Title and body" },
            { value: "section", label: "Section header" },
            { value: "two-column", label: "Two columns" },
            { value: "image-caption", label: "Caption" },
            { value: "blank", label: "Blank" },
          ]}
        />
      </ToolbarRow>

      {/* Main workspace */}
      <div style={{ flex: 1, display: "flex", "min-height": "0", overflow: "hidden" }}>
        {/* Left filmstrip (virtualized: only a window around the scroll
            viewport plus the active slide renders live thumbnails). */}
        <aside class="g-filmstrip" aria-label="Slides">
          <div style={{
            padding: "6px 8px",
            "font-size": "11px",
            "font-weight": "600",
            color: "var(--text-secondary)",
            "border-bottom": "1px solid var(--border-color)",
            "flex-shrink": "0",
            "letter-spacing": "0.02em",
          }}>
            Slides
          </div>
          <div
            ref={(el) => { filmstripEl = el; }}
            style={{ flex: 1, "overflow-y": "auto", padding: "8px 6px", display: "flex", "flex-direction": "column", gap: "8px" }}
            onScroll={(e) => {
              const el = e.currentTarget;
              setFilmstripScroll({ top: el.scrollTop, height: el.clientHeight });
            }}
          >
            <Show when={(visibleRange().first) > 0}>
              <div aria-hidden="true" style={{ height: `${visibleRange().first * FILMSTRIP_ROW_HEIGHT()}px`, "flex-shrink": "0" }} />
            </Show>
            <For each={windowedSlides()}>
              {(slide, idx) => {
                const index = () => visibleRange().first + idx();
                const active = () => index() === activeSlideIndex();
                return (
                  <div
                    onClick={() => selectSlide(index())}
                    draggable
                    onDragStart={(e) => {
                      setFilmstripDragIndex(index());
                      e.dataTransfer?.setData("text/plain", index().toString());
                    }}
                    onDragOver={(e) => {
                      e.preventDefault();
                    }}
                    onDrop={(e) => {
                      const from = filmstripDragIndex();
                      if (from !== null && from !== index()) {
                        reorderSlides(from, index());
                      }
                      setFilmstripDragIndex(null);
                    }}
                    onDragEnd={() => setFilmstripDragIndex(null)}
                    style={{
                      display: "flex",
                      "align-items": "flex-start",
                      gap: "6px",
                      cursor: "grab",
                      padding: "2px",
                      "flex-shrink": "0",
                      opacity: filmstripDragIndex() === index() ? 0.65 : 1,
                    }}
                  >
                    <span style={{ "font-size": "11px", color: "var(--text-muted)", width: "14px", "padding-top": "2px", "text-align": "right" }}>
                      {index() + 1}
                    </span>
                    <SlideThumb slide={slide} active={active()} theme={theme()} canvasWidth={canvasW()} canvasHeight={canvasH()} />
                  </div>
                );
              }}
            </For>
            <Show when={(slides().length - 1 - visibleRange().last) > 0}>
              <div aria-hidden="true" style={{ height: `${(slides().length - 1 - visibleRange().last) * FILMSTRIP_ROW_HEIGHT()}px`, "flex-shrink": "0" }} />
            </Show>
          </div>
        </aside>

        {/* Center canvas + notes */}
        <main
          style={{
            flex: 1,
            display: "flex",
            "flex-direction": "column",
            "align-items": "center",
            "justify-content": "flex-start",
            padding: "20px 16px 12px",
            overflow: "auto",
            background: "var(--bg-canvas)",
            "min-width": "0",
          }}
        >
          <div
            style={{
              transform: `scale(${zoom() / 100})`,
              "transform-origin": "top center",
              "flex-shrink": 0,
            }}
          >
          <div
            ref={(el) => { canvasEl = el; }}
            class={`g-slide-canvas g-print-slide ${slideAnimClass() === "fade" ? "g-slide-anim-fade" : ""} ${slideAnimClass() === "slide-left" ? "g-slide-anim-left" : ""} ${slideAnimClass() === "slide-right" ? "g-slide-anim-right" : ""} ${slideAnimClass() === "wipe-left" ? "g-slide-anim-wipe-left" : ""} ${slideAnimClass() === "wipe-right" ? "g-slide-anim-wipe-right" : ""} ${slideAnimClass() === "zoom" ? "g-slide-anim-zoom" : ""} ${slideAnimClass() === "dissolve" ? "g-slide-anim-dissolve" : ""} ${slideAnimClass() === "morph" ? "g-slide-anim-morph" : ""}`}
            style={{
              width: `${canvasW()}px`,
              height: `${canvasH()}px`,
              background: slideBackground(),
              "box-shadow": "0 2px 12px rgba(0,0,0,.45)",
              "border-radius": "0",
              position: "relative",
              overflow: "hidden",
              "flex-shrink": "0",
            }}
            tabindex="0"
            role="region"
            data-pane="canvas"
            aria-label="Slide canvas. Tab moves between elements; arrow keys nudge the selection; all toolbar actions have keyboard equivalents."
            onContextMenu={(event) => {
              event.preventDefault();
              const sel = selectedElement();
              const tableCell = sel?.type === "table" && tableContextCell() ? tableContextCell()! : null;
              const items: ContextMenuItem[] = [
                { id: "new-slide", label: "New Slide", action: addSlide },
                { id: "duplicate-slide", label: "Duplicate Slide", action: duplicateSlide },
                { id: "sep1", label: "", separator: true },
                { id: "insert-text", label: "Insert Text Box", action: addTextBox },
                { id: "insert-image", label: "Insert Image", action: addImage },
                { id: "sep2", label: "", separator: true },
                { id: "bring-front", label: "Bring to Front", disabled: !selectedElementId(), action: () => reorderSelected(true) },
                { id: "bring-forward", label: "Bring Forward", disabled: !selectedElementId(), action: () => stepSelectedZOrder(true) },
                { id: "send-backward", label: "Send Backward", disabled: !selectedElementId(), action: () => stepSelectedZOrder(false) },
                { id: "send-back", label: "Send to Back", disabled: !selectedElementId(), action: () => reorderSelected(false) },
                { id: "rotate", label: "Rotate +15°", disabled: !selectedElementId(), action: () => rotateSelected(15) },
              ];
              if (sel?.type === "table" && tableCell) {
                const merges = sel.tableMerges ?? [];
                const covered = mergeAt(merges, tableCell.r, tableCell.c);
                const inMerge = isCoveredByMerge(merges, tableCell.r, tableCell.c) || (covered && (covered.rowspan > 1 || covered.colspan > 1));
                const data = sel.tableData ?? [];
                items.push(
                  { id: "sep-table", label: "", separator: true },
                  {
                    id: "table-merge-right",
                    label: "Merge with Right",
                    disabled: !mergeTableCells(data, merges, tableCell.r, tableCell.c, 1, 2),
                    action: () => applyTableMerge(tableCell.r, tableCell.c, 1, 2),
                  },
                  {
                    id: "table-merge-down",
                    label: "Merge with Below",
                    disabled: !mergeTableCells(data, merges, tableCell.r, tableCell.c, 2, 1),
                    action: () => applyTableMerge(tableCell.r, tableCell.c, 2, 1),
                  },
                  {
                    id: "table-split",
                    label: "Split Cell",
                    disabled: !inMerge,
                    action: () => applyTableSplit(tableCell.r, tableCell.c),
                  },
                );
              }
              setContextMenu({ x: event.clientX, y: event.clientY, items });
            }}
            onKeyDown={(event) => {
              const target = event.target as HTMLElement | null;
              if (target?.isContentEditable || target?.tagName === "INPUT" || target?.tagName === "TEXTAREA") return;
              const amount = event.shiftKey ? 10 : 1;
              if (event.key === "ArrowLeft") nudgeSelected(-amount, 0);
              else if (event.key === "ArrowRight") nudgeSelected(amount, 0);
              else if (event.key === "ArrowUp") nudgeSelected(0, -amount);
              else if (event.key === "ArrowDown") nudgeSelected(0, amount);
              else return;
              // Keep the window-level handler from nudging again (double-move).
              event.stopPropagation();
              event.preventDefault();
            }}
            onPaste={(event) => {
              const items = event.clipboardData?.items;
              if (!items) return;
              for (const item of Array.from(items)) {
                if (item.type.startsWith("image/")) {
                  event.preventDefault();
                  const file = item.getAsFile();
                  if (file) pasteImageFromClipboard(file);
                  break;
                }
              }
            }}
          >
            <For each={alignmentGuides()}>
              {(guide) => (
                <div
                  aria-hidden="true"
                  style={guide.axis === "x"
                    ? {
                        position: "absolute",
                        left: `${guide.position}px`,
                        top: "0",
                        width: "1px",
                        height: `${canvasH()}px`,
                        background: "#f43f5e",
                        "pointer-events": "none",
                        "z-index": 10,
                      }
                    : {
                        position: "absolute",
                        left: "0",
                        top: `${guide.position}px`,
                        width: `${canvasW()}px`,
                        height: "1px",
                        background: "#f43f5e",
                        "pointer-events": "none",
                        "z-index": 10,
                      }}
                />
              )}
            </For>
            <For each={slideChromeOverlays(activeSlide() ?? emptySlide, masters(), activeSlideIndex(), canvasW(), canvasH())}>
              {(overlay) => (
                <div
                  aria-hidden="true"
                  style={{
                    position: "absolute",
                    left: `${overlay.x}px`,
                    top: `${overlay.y}px`,
                    width: `${overlay.width}px`,
                    "font-size": `${overlay.fontSize}px`,
                    color: "var(--text-muted, #64748b)",
                    "text-align": overlay.align,
                    "pointer-events": "none",
                    "user-select": "none",
                  }}
                >
                  {overlay.text}
                </div>
              )}
            </For>
            <For each={activeSlide()?.elements || []}>
              {(el) => {
                const isSelected = () => selectedElementIds().includes(el.id);
                const placeholder = () => el.type === "text" && isPlaceholder(el.content);
                const elementLabel = () => {
                  if (el.type === "text") return `Text: ${(el.content || "empty").slice(0, 50)}`;
                  if (el.type === "image") return "Image";
                  if (el.type === "chart") return `Chart: ${el.chartTitle || "Untitled"}`;
                  if (el.type === "table") return `Table${el.tableRows && el.tableCols ? ` ${el.tableRows}x${el.tableCols}` : ""}`;
                  if (el.type === "line") return "Line";
                  if (el.type === "arrow") return "Arrow";
                  return `${el.type.charAt(0).toUpperCase() + el.type.slice(1)} shape`;
                };
                return (
                  <div
                    role="img"
                    aria-label={elementLabel()}
                    aria-selected={isSelected()}
                    onClick={(event) => {
                      if ((event.ctrlKey || event.metaKey) && el.hyperlink) {
                        const internalId = parseInternalSlideId(el.hyperlink);
                        if (internalId) {
                          event.preventDefault();
                          event.stopPropagation();
                          const targetIndex = slides().findIndex((slide) => slide.id === internalId);
                          if (targetIndex >= 0) selectSlide(targetIndex);
                          else showToast("Linked slide was not found in this deck.", "error");
                          return;
                        }
                        const target = normalizeSlideHyperlink(el.hyperlink);
                        if (target) {
                          event.preventDefault();
                          event.stopPropagation();
                          window.open(target, "_blank", "noopener,noreferrer");
                          return;
                        }
                        showToast("This hyperlink is invalid or unsafe.", "error");
                      }
                      selectElement(el.id, event.shiftKey);
                    }}
                    onPointerDown={(event) => {
                      if ((el.type === "text" || isFilledShape(el.type)) && document.activeElement === event.currentTarget.querySelector("[contenteditable]")) {
                        return;
                      }
                      event.stopPropagation();
                      dragPointerId = event.pointerId;
                      dragStartClient = { x: event.clientX, y: event.clientY };
                      dragMoved = false;
                      dragAltDuplicate = event.altKey;
                      if (!event.shiftKey && !selectedElementIds().includes(el.id)) {
                        selectElement(el.id, false);
                      } else if (event.shiftKey) {
                        selectElement(el.id, true);
                      }
                      setAlignmentGuides([]);
                      const local = canvasToLocal(event.clientX, event.clientY);
                      setDragOffset({ x: local.x - el.x, y: local.y - el.y });
                    }}
                    onPointerMove={(event) => {
                      const currentResize = resizing();
                      if (currentResize && currentResize.id === el.id) {
                        const local = canvasToLocal(event.clientX, event.clientY);
                        const updatedEl = applyResize(el, currentResize.handle, local.x, local.y, currentResize, event.shiftKey);
                        setAlignmentGuides([]);
                        setSlides(prev => prev.map((s, i) => i === activeSlideIndex() ? { ...s, elements: s.elements.map(e => e.id === el.id ? updatedEl : e) } : s));
                        return;
                      }
                      if (dragPointerId !== event.pointerId) return;
                      const dx = event.clientX - dragStartClient.x;
                      const dy = event.clientY - dragStartClient.y;
                      if (!dragging() && Math.hypot(dx, dy) < DRAG_THRESHOLD) return;
                      if (!dragging()) {
                        pushSlideHistory();
                        setDragging(true);
                        event.currentTarget.setPointerCapture(event.pointerId);
                        // Alt+drag: spawn duplicates of the selection behind
                        // the dragged element (PowerPoint-style); the drag
                        // itself keeps moving the grabbed element.
                        if (dragAltDuplicate) {
                          dragAltDuplicate = false;
                          const sourceIds = selectedElementIds().includes(el.id) && selectedElementIds().length > 0
                            ? selectedElementIds()
                            : elementsToMoveWith(el).map((peer) => peer.id);
                          setSlides(prev => prev.map((s, i) => {
                            if (i !== activeSlideIndex()) return s;
                            return {
                              ...s,
                              elements: [
                                ...s.elements,
                                ...s.elements
                                  .filter((e) => sourceIds.includes(e.id))
                                  .map((e) => ({ ...e, id: generateSlideId("el") })),
                              ],
                            };
                          }));
                          return;
                        }
                      }
                      const local = canvasToLocal(event.clientX, event.clientY);
                      const dragEl = () => activeSlide()?.elements.find((e) => e.id === el.id) || el;
                      const prevX = dragEl().x;
                      const prevY = dragEl().y;
                      const snapped = snapElementPosition(
                        dragEl(),
                        local.x - dragOffset().x,
                        local.y - dragOffset().y,
                        activeSlide()?.elements || [],
                        canvasW(),
                        canvasH(),
                      );
                      const moveDx = snapped.x - prevX;
                      const moveDy = snapped.y - prevY;
                      const peersToMove = elementsToMoveWith(el).map(p => p.id);
                      dragMoved = true;
                      setAlignmentGuides(snapped.guides);
                      setSlides(prev => prev.map((s, i) => {
                        if (i !== activeSlideIndex()) return s;
                        return {
                          ...s,
                          elements: s.elements.map(e => {
                            if (e.id === el.id) {
                              return { ...e, x: snapped.x, y: snapped.y };
                            } else if (peersToMove.includes(e.id)) {
                              return { ...e, x: e.x + moveDx, y: e.y + moveDy };
                            }
                            return e;
                          })
                        };
                      }));
                    }}
                    onPointerUp={(event) => {
                      if (resizing()?.id === el.id) {
                        setResizing(null);
                        setAlignmentGuides([]);
                        commitChange();
                        return;
                      }
                      if (dragPointerId !== event.pointerId) return;
                      dragPointerId = null;
                      if (dragging()) {
                        setDragging(false);
                        setAlignmentGuides([]);
                        event.currentTarget.releasePointerCapture(event.pointerId);
                        if (dragMoved) commitChange();
                      }
                    }}
                    style={{
                      position: "absolute",
                      left: `${el.x}px`,
                      top: `${el.y}px`,
                      width: `${el.width}px`,
                      height: `${el.height}px`,
                      border: isSelected() ? "2px dashed var(--slide-accent)" : "none",
                      cursor: "move",
                      display: "flex",
                      "align-items": "center",
                      "justify-content": el.align === "left" ? "flex-start" : el.align === "right" ? "flex-end" : "center",
                      transform: `rotate(${el.rotation || 0}deg)`,
                      "transform-origin": "center center",
                    }}
                  >
                    {el.type === "text" && (
                      <div
                        contentEditable
                        onFocus={(e) => {
                          pushSlideHistory();
                          // Use the placeholder flag for reliable detection — content
                          // that matches placeholder text but was typed by the user
                          // (without the flag) is preserved.
                          if (el.placeholder) {
                            e.currentTarget.innerText = "";
                            setSlides(prev => prev.map((s, i) => i === activeSlideIndex() ? { ...s, elements: s.elements.map(eItem => eItem.id === el.id ? { ...eItem, content: "", placeholder: false } : eItem) } : s));
                          }
                        }}
                        onBlur={(e) => {
                          const text = e.currentTarget.innerText.trim();
                          const isNowPlaceholder = !text;
                          const newContent = text || (el.fontSize && el.fontSize >= 28 ? PLACEHOLDER_TITLE : PLACEHOLDER_BODY);
                          e.currentTarget.innerText = newContent;
                          setSlides(prev => prev.map((s, i) => i === activeSlideIndex() ? { ...s, elements: s.elements.map(eItem => eItem.id === el.id ? { ...eItem, content: newContent, placeholder: isNowPlaceholder } : eItem) } : s));
                          commitChange();
                        }}
                        style={{
                          width: "100%",
                          height: "100%",
                          "font-family": el.fontFamily || "Inter, sans-serif",
                          "font-weight": el.bold ? "bold" : "normal",
                          "font-size": `${el.fontSize || 20}px`,
                          color: placeholder() ? "#999" : (el.color || "#0f172a"),
                          "font-style": el.italic || placeholder() ? "italic" : "normal",
                          "text-decoration": el.underline ? "underline" : "none",
                          "text-align": el.align || "center",
                          "white-space": "pre-wrap",
                          "padding-left": el.bullets ? "18px" : "0",
                          outline: "none",
                          "background-image": el.bullets && !placeholder()
                            ? "radial-gradient(circle, currentColor 1.5px, transparent 1.6px)"
                            : "none",
                          "background-size": "6px 1.4em",
                          "background-position": "4px 0.55em",
                          "background-repeat": "repeat-y",
                        }}
                      >
                        {el.content}
                      </div>
                    )}
                    <Show when={isSelected()}>
                      <div
                        role="button"
                        aria-label="Rotate element"
                        title="Drag vertically to rotate"
                        onPointerDown={(event) => {
                          event.stopPropagation();
                          pushSlideHistory();
                          event.currentTarget.setPointerCapture(event.pointerId);
                          const startY = event.clientY;
                          const startRot = el.rotation || 0;
                          const onMove = (moveEvent: PointerEvent) => {
                            const delta = Math.round((moveEvent.clientY - startY) / 2);
                            const newRotation = ((startRot + delta) % 360 + 360) % 360;
                            setSlides(prev => prev.map((s, i) => i === activeSlideIndex() ? { ...s, elements: s.elements.map(eItem => eItem.id === el.id ? { ...eItem, rotation: newRotation } : eItem) } : s));
                          };
                          const onUp = (upEvent: PointerEvent) => {
                            window.removeEventListener("pointermove", onMove);
                            window.removeEventListener("pointerup", onUp);
                            (upEvent.target as HTMLElement | null)?.releasePointerCapture?.(upEvent.pointerId);
                            commitChange();
                          };
                          window.addEventListener("pointermove", onMove);
                          window.addEventListener("pointerup", onUp);
                        }}
                        style={{ position: "absolute", left: "50%", top: "-18px", width: "12px", height: "12px", background: "var(--slide-accent)", border: "2px solid white", "border-radius": "50%", cursor: "grab", transform: "translateX(-50%)", "z-index": 3 }}
                      />
                    </Show>
                    {isFilledShape(el.type) && (
                      <>
                        <ShapeBody type={el.type} color={el.color || "#3b82f6"} id={el.id} />
                        <div
                          contentEditable
                          onFocus={(e) => {
                            pushSlideHistory();
                            if (!el.content) {
                              e.currentTarget.innerText = "";
                              setSlides(prev => prev.map((s, i) => i === activeSlideIndex() ? { ...s, elements: s.elements.map(eItem => eItem.id === el.id ? { ...eItem, content: "" } : eItem) } : s));
                            }
                          }}
                          onBlur={(e) => {
                            const newContent = e.currentTarget.innerText.trim();
                            e.currentTarget.innerText = newContent;
                            setSlides(prev => prev.map((s, i) => i === activeSlideIndex() ? { ...s, elements: s.elements.map(eItem => eItem.id === el.id ? { ...eItem, content: newContent } : eItem) } : s));
                            commitChange();
                          }}
                          style={{
                            position: "absolute",
                            inset: "8px",
                            "font-family": el.fontFamily || "Inter, sans-serif",
                            "font-size": `${el.fontSize || 16}px`,
                            color: el.color === "#3b82f6" ? "#ffffff" : (el.color || "#ffffff"),
                            "text-align": el.align || "center",
                            outline: "none",
                            display: "flex",
                            "align-items": "center",
                            "justify-content": "center",
                            "pointer-events": "auto",
                          }}
                        >
                          {el.content}
                        </div>
                      </>
                    )}
                    {(el.type === "line" || el.type === "arrow") && (
                      <ShapeBody type={el.type} color={el.color || "#3b82f6"} id={el.id} />
                    )}
                    {el.type === "image" && (
                      <img src={el.content} alt="Slide asset" style={{ width: "100%", height: "100%", "object-fit": "contain", "pointer-events": "none" }} />
                    )}
                    {el.type === "table" && el.tableData && (
                      <table style={{ width: "100%", height: "100%", "border-collapse": "collapse", "font-size": "12px", background: "#fff" }}>
                        <tbody>
                          <For each={el.tableData}>
                            {(row, ri) => (
                              <tr>
                                <For each={row}>
                                  {(cell, ci) => {
                                    const merge = mergeAt(el.tableMerges ?? [], ri(), ci());
                                    if (merge && (merge.r !== ri() || merge.c !== ci())) {
                                      // Covered by a merge anchored elsewhere: skip.
                                      return <td style={{ display: "none" }} />;
                                    }
                                    const span = merge ?? { rowspan: 1, colspan: 1 };
                                    return (
                                      <td
                                        contentEditable
                                        rowSpan={span.rowspan}
                                        colSpan={span.colspan}
                                        onContextMenu={(e) => {
                                          setTableContextCell({ r: ri(), c: ci() });
                                        }}
                                        style={{ border: "1px solid #cbd5e1", padding: "4px", "min-width": "24px", ...(el.tableHeaderRow && ri() === 0 ? { background: "#e2e8f0", "font-weight": "600" } : {}) }}
                                        onInput={(e) => {
                                          const newText = e.currentTarget.textContent || "";
                                          setSlides(prev => prev.map((s, i) => {
                                            if (i !== activeSlideIndex()) return s;
                                            return {
                                              ...s,
                                              elements: s.elements.map(elItem => {
                                                if (elItem.id === el.id && elItem.tableData) {
                                                  const newData = elItem.tableData.map(r => [...r]);
                                                  newData[ri()][ci()] = newText;
                                                  return { ...elItem, tableData: newData };
                                                }
                                                return elItem;
                                              })
                                            };
                                          }));
                                        }}
                                        onBlur={() => commitChange()}
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
                    )}
                    {el.type === "chart" && (
                      <div style={{ width: "100%", height: "100%", background: "#f8fafc", display: "flex", "flex-direction": "column", padding: "8px", "box-sizing": "border-box", position: "relative" }}>
                        <div style={{ display: "flex", gap: "6px", "align-items": "center", "margin-bottom": "4px" }}>
                          <input
                            type="text"
                            value={el.chartTitle || "Chart"}
                            onInput={(e) => {
                              const newTitle = e.currentTarget.value;
                              setSlides(prev => prev.map((s, i) => i === activeSlideIndex() ? { ...s, elements: s.elements.map(elItem => elItem.id === el.id ? { ...elItem, chartTitle: newTitle } : elItem) } : s));
                            }}
                            onBlur={() => commitChange()}
                            style={{ flex: 1, "font-size": "14px", "font-weight": "600", border: "none", background: "transparent" }}
                          />
                          <select
                            aria-label="Chart type"
                            value={el.chartType || "bar"}
                            onClick={(e) => e.stopPropagation()}
                            onPointerDown={(e) => e.stopPropagation()}
                            onChange={(e) => {
                              const chartType = e.currentTarget.value as ChartType;
                              pushSlideHistory();
                              setSlides(prev => prev.map((s, i) => i === activeSlideIndex() ? { ...s, elements: s.elements.map(elItem => elItem.id === el.id ? { ...elItem, chartType } : elItem) } : s));
                              commitChange();
                            }}
                            style={{ "font-size": "11px", border: "1px solid var(--border-color, #d9d9d9)", "border-radius": "4px", background: "var(--bg-input, #fff)", color: "inherit", padding: "1px 2px" }}
                          >
                            <For each={CHART_TYPES}>
                              {(type) => <option value={type}>{type}</option>}
                            </For>
                          </select>
                        </div>
                        {el.chartType === "pie" ? (
                          <svg viewBox="0 0 100 100" style={{ flex: 1 }}>
                            <For each={(() => {
                              const data = el.chartData || [3, 5, 2, 8];
                              const total = data.reduce((a, b) => a + b, 0) || 1;
                              let angle = 0;
                              return data.map((v, i) => {
                                const slice = (v / total) * 360;
                                const start = angle;
                                angle += slice;
                                return { start, slice, color: `hsl(${(i * 47) % 360}, 65%, 55%)` };
                              });
                            })()}>
                              {(slice) => {
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
                        ) : el.chartType === "line" ? (
                          <>
                            <svg viewBox={`0 0 ${(el.chartData || [3, 5, 2, 8]).length * 40} 100`} preserveAspectRatio="none" style={{ flex: 1, width: "100%" }}>
                              <For each={(el.chartData || [3, 5, 2, 8]).slice(0, -1)}>
                                {(_, i) => {
                                  const data = el.chartData || [3, 5, 2, 8];
                                  const max = Math.max(...data, 1);
                                  const x1 = i() * 40 + 20;
                                  const x2 = (i() + 1) * 40 + 20;
                                  const y1 = 100 - (data[i()] / max) * 100;
                                  const y2 = 100 - (data[i() + 1] / max) * 100;
                                  return <line x1={x1} y1={y1} x2={x2} y2={y2} stroke="#3b82f6" stroke-width="3" vector-effect="non-scaling-stroke" />;
                                }}
                              </For>
                              <For each={el.chartData || [3, 5, 2, 8]}>
                                {(v, i) => {
                                  const data = el.chartData || [3, 5, 2, 8];
                                  const max = Math.max(...data, 1);
                                  const cx = i() * 40 + 20;
                                  const cy = 100 - (v / max) * 100;
                                  return <circle cx={cx} cy={cy} r="3" fill={`hsl(${(i() * 47) % 360}, 65%, 55%)`} />;
                                }}
                              </For>
                            </svg>
                            <div style={{ display: "flex", "font-size": "9px", color: "#64748b", "justify-content": "space-around" }}>
                              <For each={el.chartLabels || []}>{(label) => <span>{label}</span>}</For>
                            </div>
                          </>
                        ) : (
                          <div style={{ flex: 1, display: "flex", "align-items": "flex-end", gap: "4px" }}>
                            <For each={el.chartData || [3, 5, 2, 8]}>
                              {(v, i) => {
                                const max = Math.max(...(el.chartData || [1]), 1);
                                return (
                                  <div style={{ flex: 1, display: "flex", "flex-direction": "column", "align-items": "center", gap: "2px" }}>
                                    <div style={{ width: "100%", height: `${(v / max) * 100}%`, "min-height": "2px", background: `hsl(${(i() * 47) % 360}, 65%, 55%)` }} />
                                    <span style={{ "font-size": "9px", color: "#64748b" }}>{el.chartLabels?.[i()] || i() + 1}</span>
                                  </div>
                                );
                              }}
                            </For>
                          </div>
                        )}
                        <Show when={isSelected()}>
                          {/* Inline chart-data editor: values and labels become
                              editable after insert instead of being frozen. */}
                          <div
                            class="g-chart-data-editor"
                            style={{
                              position: "absolute",
                              top: "100%",
                              left: "0",
                              "margin-top": "6px",
                              width: "240px",
                              background: "var(--bg-surface, #ffffff)",
                              border: "1px solid var(--border-color, #d9d9d9)",
                              "border-radius": "8px",
                              "box-shadow": "0 8px 24px rgba(0,0,0,.18)",
                              padding: "8px",
                              display: "flex",
                              "flex-direction": "column",
                              gap: "6px",
                              "z-index": "40",
                            }}
                          >
                            <div style={{ "font-size": "11px", "font-weight": "600", color: "var(--text-secondary, #6b7280)" }}>Chart data</div>
                            <For each={el.chartData || [3, 5, 2, 8]}>
                              {(v, i) => (
                                <div style={{ display: "flex", gap: "4px", "align-items": "center" }}>
                                  <input
                                    aria-label={`Label for data point ${i() + 1}`}
                                    type="text"
                                    value={el.chartLabels?.[i()] ?? ""}
                                    placeholder={`Point ${i() + 1}`}
                                    onInput={(e) => {
                                      const label = e.currentTarget.value;
                                      const labels = [...(el.chartLabels || [])];
                                      labels[i()] = label;
                                      setSlides(prev => prev.map((s, si) => si === activeSlideIndex() ? { ...s, elements: s.elements.map(item => item.id === el.id ? { ...item, chartLabels: labels } : item) } : s));
                                    }}
                                    onBlur={() => commitChange()}
                                    style={{ width: "72px", "font-size": "11px", padding: "2px 6px", border: "1px solid var(--border-color, #d9d9d9)", "border-radius": "4px", background: "var(--bg-input, #fff)", color: "inherit" }}
                                  />
                                  <input
                                    aria-label={`Value for data point ${i() + 1}`}
                                    type="number"
                                    value={v}
                                    onInput={(e) => {
                                      const parsed = Number(e.currentTarget.value);
                                      if (!Number.isFinite(parsed)) return;
                                      const data = [...(el.chartData || [])];
                                      data[i()] = parsed;
                                      setSlides(prev => prev.map((s, si) => si === activeSlideIndex() ? { ...s, elements: s.elements.map(item => item.id === el.id ? { ...item, chartData: data } : item) } : s));
                                    }}
                                    onBlur={() => commitChange()}
                                    style={{ width: "60px", "font-size": "11px", padding: "2px 6px", border: "1px solid var(--border-color, #d9d9d9)", "border-radius": "4px", background: "var(--bg-input, #fff)", color: "inherit" }}
                                  />
                                  <button
                                    type="button"
                                    aria-label={`Remove data point ${i() + 1}`}
                                    style={{ border: "none", background: "transparent", color: "var(--danger, #b91c1c)", cursor: "pointer", "font-size": "12px", padding: "0 4px" }}
                                    onClick={(e) => {
                                      e.stopPropagation();
                                      pushSlideHistory();
                                      const data = (el.chartData || []).filter((_, di) => di !== i());
                                      const labels = (el.chartLabels || []).filter((_, di) => di !== i());
                                      setSlides(prev => prev.map((s, si) => si === activeSlideIndex() ? { ...s, elements: s.elements.map(item => item.id === el.id ? { ...item, chartData: data, chartLabels: labels } : item) } : s));
                                      commitChange();
                                    }}
                                  >
                                    ×
                                  </button>
                                </div>
                              )}
                            </For>
                            <button
                              type="button"
                              style={{ "font-size": "11px", padding: "3px 8px", border: "1px solid var(--border-color, #d9d9d9)", "border-radius": "4px", background: "var(--bg-input, #fff)", color: "inherit", cursor: "pointer" }}
                              onClick={(e) => {
                                e.stopPropagation();
                                pushSlideHistory();
                                const data = [...(el.chartData || []), 1];
                                const labels = [...(el.chartLabels || []), `P${data.length}`];
                                setSlides(prev => prev.map((s, si) => si === activeSlideIndex() ? { ...s, elements: s.elements.map(item => item.id === el.id ? { ...item, chartData: data, chartLabels: labels } : item) } : s));
                                commitChange();
                              }}
                            >
                              + Add data point
                            </button>
                          </div>
                        </Show>
                      </div>
                    )}
                    <Show when={isSelected()}>
                      <For each={["nw", "n", "ne", "e", "se", "s", "sw", "w"] as ResizeHandle[]}>
                        {(handle) => (
                          <div
                            role="button"
                            aria-label={`Resize ${handle}`}
                            class={`g-resize-handle g-resize-handle-${handle}`}
                            onPointerDown={(event) => {
                              event.stopPropagation();
                              pushSlideHistory();
                              event.currentTarget.setPointerCapture(event.pointerId);
                              setAlignmentGuides([]);
                              setResizing({
                                id: el.id,
                                handle,
                                startX: event.clientX,
                                startY: event.clientY,
                                origX: el.x,
                                origY: el.y,
                                width: el.width,
                                height: el.height,
                              });
                            }}
                          />
                        )}
                      </For>
                    </Show>
                  </div>
                );
              }}
            </For>
          </div>
          </div>

          {/* Notes pane */}
          <div
            style={{
              width: `${canvasW()}px`,
              "margin-top": "12px",
              background: "var(--bg-toolbar)",
              border: "1px solid var(--border-color)",
              "border-radius": "0",
              padding: "8px 10px",
              "flex-shrink": "0",
            }}
          >
            <div
              role="separator"
              aria-orientation="horizontal"
              aria-label="Resize notes pane"
              title="Drag to resize notes"
              onPointerDown={(event) => {
                event.currentTarget.setPointerCapture(event.pointerId);
                const startY = event.clientY;
                const startH = notesHeight();
                const onMove = (ev: PointerEvent) => {
                  const next = Math.min(240, Math.max(56, startH + (startY - ev.clientY)));
                  setNotesHeight(next);
                };
                const onUp = (ev: PointerEvent) => {
                  window.removeEventListener("pointermove", onMove);
                  window.removeEventListener("pointerup", onUp);
                  try {
                    (ev.target as HTMLElement)?.releasePointerCapture?.(ev.pointerId);
                  } catch {
                    /* ignore */
                  }
                };
                window.addEventListener("pointermove", onMove);
                window.addEventListener("pointerup", onUp);
              }}
              style={{
                height: "6px",
                "margin-bottom": "6px",
                cursor: "ns-resize",
                background: "var(--border-color)",
                "border-radius": "2px",
              }}
            />
            <div style={{ "font-size": "11px", color: "var(--text-muted)", "margin-bottom": "4px" }}>Notes</div>
            <textarea
              placeholder="Type speaker notes here..."
              value={activeSlide()?.notes || ""}
              onInput={(e) => {
                const idx = activeSlideIndex();
                const list = [...slides()];
                const slide = list[idx];
                if (!slide) return;
                if (!notesHistoryPushed()) {
                  pushSlideHistory();
                  setNotesHistoryPushed(true);
                }
                list[idx] = { ...slide, notes: e.currentTarget.value };
                setSlides(list);
                emitChange(list);
              }}
              onBlur={() => {
                setNotesHistoryPushed(false);
              }}
              style={{
                width: "100%",
                height: `${notesHeight()}px`,
                border: "none",
                background: "transparent",
                color: "var(--text-primary)",
                resize: "vertical",
                "min-height": "56px",
                "max-height": "240px",
                "font-size": "13px",
              }}
            />
            {/* Review comments on this slide (offline collaboration) */}
            <div style={{ "margin-top": "8px" }}>
              <div style={{ "font-size": "11px", color: "var(--text-muted)", "margin-bottom": "4px" }}>
                Comments{" "}
                {(activeSlide()?.comments ?? []).filter((comment) => !comment.resolved).length > 0
                  ? `· ${(activeSlide()?.comments ?? []).filter((comment) => !comment.resolved).length} open`
                  : ""}
              </div>
              <div style={{ display: "flex", gap: "6px" }}>
                <input
                  class="g-toolbar-input"
                  aria-label="New slide comment"
                  placeholder="Comment on this slide…"
                  value={commentDraft()}
                  onInput={(e) => setCommentDraft(e.currentTarget.value)}
                  style={{ flex: 1 }}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      addSlideComment();
                      e.preventDefault();
                    }
                  }}
                />
                <button type="button" class="g-toolbar-btn" onClick={addSlideComment}>Add</button>
              </div>
              <For each={activeSlide()?.comments ?? []}>
                {(comment) => (
                  <div style={{ border: "1px solid var(--border-color)", "border-radius": "6px", padding: "5px", "margin-top": "5px" }}>
                    <div style={{ display: "flex", "align-items": "center", gap: "5px" }}>
                      <span style={{ flex: 1, "font-size": "11px", "font-weight": "600" }}>{comment.author}</span>
                      <button
                        type="button"
                        class="g-toolbar-btn"
                        aria-label={comment.resolved ? `Reopen comment ${comment.id}` : `Resolve comment ${comment.id}`}
                        onClick={() => setSlideCommentResolved(comment.id, !comment.resolved)}
                      >
                        {comment.resolved ? "Reopen" : "Resolve"}
                      </button>
                      <button
                        type="button"
                        class="g-toolbar-btn"
                        aria-label={`Delete comment ${comment.id}`}
                        onClick={() => deleteSlideComment(comment.id)}
                      >
                        ✕
                      </button>
                    </div>
                    <div
                      style={{
                        "font-size": "11px",
                        "margin-top": "3px",
                        color: comment.resolved ? "var(--text-muted)" : "var(--text-primary)",
                        "text-decoration": comment.resolved ? "line-through" : "none",
                      }}
                    >
                      {comment.text}
                    </div>
                  </div>
                )}
              </For>
            </div>
          </div>
        </main>

        <IconSidebar panels={sidebarPanels()} activePanel={sidebarPanel()} onActivePanelChange={setSidebarPanel} />
      </div>
      <Show when={slideshowOpen()}>
        <AudienceSlideshow
          deck={slideshowDeck()}
          startIndex={activeSlideIndex()}
          onExit={() => setSlideshowOpen(false)}
        />
      </Show>
      <Show when={contextMenu()}>
        {(menu) => (
          <ContextMenu
            x={menu().x}
            y={menu().y}
            items={menu().items}
            onClose={() => setContextMenu(null)}
          />
        )}
      </Show>
      <Dialog open={printDialogOpen()} title="Print Slides" onClose={() => setPrintDialogOpen(false)}>
        <div style={{ display: "flex", "flex-direction": "column", gap: "10px", "min-width": "280px" }}>
          <div style={{ "font-size": "12px", "font-weight": "600" }}>Handouts</div>
          {([1, 2, 4, 6] as PrintPerPage[]).map((n) => (
            <label style={{ display: "flex", gap: "8px", "align-items": "center", "font-size": "13px" }}>
              <input
                type="radio"
                name="slide-print-per-page"
                checked={printPerPage() === n}
                onChange={() => setPrintPerPage(n)}
              />
              {n} per page
            </label>
          ))}
          <label style={{ display: "flex", gap: "8px", "align-items": "center", "font-size": "13px" }}>
            <input
              type="checkbox"
              checked={printIncludeNotes()}
              onChange={(e) => setPrintIncludeNotes(e.currentTarget.checked)}
            />
            Include speaker notes
          </label>
          <div style={{ display: "flex", gap: "8px", "justify-content": "flex-end" }}>
            <button type="button" class="g-toolbar-btn" onClick={() => setPrintDialogOpen(false)}>Cancel</button>
            <button type="button" class="g-toolbar-btn" onClick={printSlides}>Print</button>
          </div>
        </div>
      </Dialog>
    </div>
  );
}
