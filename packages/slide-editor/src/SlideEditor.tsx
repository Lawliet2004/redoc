import { createSignal, For, Show, onMount, onCleanup, type JSX } from "solid-js";
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
import { snapElementPosition, type AlignmentGuide } from "./geometry";

const PLACEHOLDER_TITLE = "Click to add Title";
const PLACEHOLDER_BODY = "Click to add Text";

type SlideTransition = "none" | "fade" | "slide-left" | "slide-right";
type ElementEntrance = "none" | "fade";
type PrintPerPage = 1 | 2 | 4 | 6;

function isPlaceholder(content: string) {
  return content === PLACEHOLDER_TITLE || content === PLACEHOLDER_BODY;
}

function normalizeEntrance(value: unknown): ElementEntrance {
  return value === "fade" ? "fade" : "none";
}

interface SlideElement {
  id: string;
  type: "text" | "rect" | "ellipse" | "line" | "arrow" | "image";
  x: number;
  y: number;
  width: number;
  height: number;
  content: string;
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
}

interface Slide {
  id: string;
  title: string;
  layout: string;
  elements: SlideElement[];
  notes: string;
  transition: SlideTransition;
}

interface SlideEditorProps {
  initialContent?: any;
  onChange?: (jsonContent: any) => void;
  onSlideInfoChange?: (info: string) => void;
  onRequestNew?: () => void;
  onRequestOpen?: () => void;
  onRequestSave?: () => void;
  onRequestExportPdf?: () => void;
}

const defaultTheme = {
  id: "default-light",
  name: "Modern Light",
  bgColor: "#ffffff",
  textColor: "#1e293b",
  accentColor: "#3b82f6",
  fontFamily: "Inter, sans-serif",
};

type DrawTool = "select" | "line" | "rect" | "ellipse" | "arrow" | "text";

function normalizeTransition(value: unknown, fallbackFade = false): SlideTransition {
  if (value === "fade" || value === "slide-left" || value === "slide-right" || value === "none") return value;
  return fallbackFade ? "fade" : "none";
}

function normalizeTextAlign(value: unknown): "left" | "center" | "right" {
  return value === "left" || value === "right" ? value : "center";
}

function normalizeFlatElement(element: any, rotation: number, entrance: ElementEntrance): SlideElement | null {
  const type = element.type as string;
  const base = {
    id: element.id || `el-${Math.random()}`,
    x: typeof element.x === "number" ? element.x : 100,
    y: typeof element.y === "number" ? element.y : 100,
    width: typeof element.width === "number" ? element.width : 400,
    height: typeof element.height === "number" ? element.height : 60,
    rotation,
    entrance,
  };
  if (type === "text") {
    return {
      ...base,
      type: "text",
      content: element.content ?? "",
      fontSize: element.fontSize,
      color: element.color ?? "#1e293b",
      align: normalizeTextAlign(element.align),
      bullets: Boolean(element.bullets),
      fontFamily: element.fontFamily,
      bold: Boolean(element.bold),
      italic: Boolean(element.italic),
      underline: Boolean(element.underline),
    };
  }
  if (type === "image") {
    return {
      ...base,
      type: "image",
      content: element.content ?? "",
      color: element.color ?? "#ffffff",
    };
  }
  if (type === "rect" || type === "ellipse" || type === "line" || type === "arrow") {
    return {
      ...base,
      type,
      content: "",
      color: element.color ?? "#3b82f6",
    };
  }
  return null;
}

function normalizeDeck(deck: any): Slide[] {
  if (!deck?.slides?.length) return [{
    id: "slide-1", title: "Title Slide", layout: "title", notes: "", transition: "none", elements: [
      { id: "el-1", type: "text", x: 100, y: 180, width: 760, height: 80, content: PLACEHOLDER_TITLE, fontSize: 40, color: "#1e293b", rotation: 0, align: "center" },
      { id: "el-2", type: "text", x: 150, y: 280, width: 660, height: 50, content: PLACEHOLDER_BODY, fontSize: 22, color: "#64748b", rotation: 0, align: "center" },
    ],
  }];
  const deckFade = Boolean(deck.fadeBetweenSlides);
  return deck.slides.map((slide: any) => ({
    id: slide.id,
    title: slide.title || slide.layout || "Slide",
    layout: slide.layout || "blank",
    notes: slide.notes || "",
    transition: normalizeTransition(slide.transition, deckFade),
    elements: (slide.elements || []).map((element: any) => {
      const kind = element.kind || {};
      const rotation = typeof element.rotation === "number" ? element.rotation : 0;
      const entrance = normalizeEntrance(element.entrance);
      if (!element.kind && element.type) {
        const flat = normalizeFlatElement(element, rotation, entrance);
        if (flat) return flat;
      }
      if (kind.Text) {
        return {
          id: element.id,
          type: "text" as const,
          x: element.x,
          y: element.y,
          width: element.width,
          height: element.height,
          content: kind.Text.text,
          fontSize: kind.Text.fontSize,
          color: kind.Text.color,
          rotation,
          align: normalizeTextAlign(kind.Text.align),
          bullets: Boolean(kind.Text.bullets),
          entrance,
          fontFamily: kind.Text.fontFamily,
          bold: Boolean(kind.Text.bold),
          italic: Boolean(kind.Text.italic),
          underline: Boolean(kind.Text.underline),
        };
      }
      if (kind.Shape) {
        const shapeType = kind.Shape.shapeType;
        const type = shapeType === "ellipse" || shapeType === "line" || shapeType === "arrow" ? shapeType : "rect";
        const strokeColor = kind.Shape.strokeColor;
        const color = type === "line" || type === "arrow"
          ? (strokeColor && strokeColor !== "transparent" ? strokeColor : kind.Shape.fillColor || "#1e293b")
          : kind.Shape.fillColor;
        return { id: element.id, type, x: element.x, y: element.y, width: element.width, height: element.height, content: "", color: color || "#3b82f6", rotation, entrance };
      }
      if (kind.Image) return { id: element.id, type: "image", x: element.x, y: element.y, width: element.width, height: element.height, content: kind.Image.assetHash, color: "#ffffff", rotation, entrance };
      return { id: element.id, type: "rect", x: element.x ?? 100, y: element.y ?? 100, width: element.width ?? 200, height: element.height ?? 120, content: "", color: "#e2e8f0", rotation, entrance };
    }),
  }));
}

function shapeKindForElement(element: SlideElement, theme: typeof defaultTheme) {
  const isLine = element.type === "line" || element.type === "arrow";
  const strokeColor = element.color || theme.textColor || "#000000";
  return {
    Shape: {
      shapeType: element.type,
      fillColor: isLine ? "transparent" : (element.color || theme.accentColor || "#3b82f6"),
      strokeColor: isLine ? strokeColor : "transparent",
      strokeWidth: isLine ? 3 : 0,
    },
  };
}

function toDeck(slides: Slide[], source: any, theme: typeof defaultTheme, activeIndex: number) {
  const anyFade = slides.some((slide) => slide.transition === "fade");
  return {
    slides: slides.map((slide) => ({
      id: slide.id,
      layout: slide.layout,
      notes: slide.notes,
      bgOverride: null,
      transition: slide.transition || "none",
      elements: slide.elements.map((element, index) => ({
        id: element.id,
        x: element.x,
        y: element.y,
        width: element.width,
        height: element.height,
        rotation: element.rotation || 0,
        zIndex: index,
        entrance: element.entrance || "none",
        kind: element.type === "text"
          ? {
              Text: {
                text: element.content,
                fontSize: element.fontSize || 20,
                fontFamily: element.fontFamily || "Inter, sans-serif",
                color: element.color || "#1e293b",
                align: element.align || "center",
                bullets: Boolean(element.bullets),
                bold: Boolean(element.bold),
                italic: Boolean(element.italic),
                underline: Boolean(element.underline),
              },
            }
          : element.type === "image"
            ? { Image: { assetHash: element.content, mime: "image/url" } }
            : shapeKindForElement(element, theme),
      })),
    })),
    theme,
    canvasWidth: source?.canvasWidth || 960,
    canvasHeight: source?.canvasHeight || 540,
    activeSlideIndex: activeIndex,
    fadeBetweenSlides: anyFade,
  };
}

export function SlideEditor(props: SlideEditorProps) {
  const [slides, setSlides] = createSignal<Slide[]>(normalizeDeck(props.initialContent));
  const [theme, setTheme] = createSignal(props.initialContent?.theme || defaultTheme);

  const [activeSlideIndex, setActiveSlideIndex] = createSignal(
    typeof props.initialContent?.activeSlideIndex === "number" ? props.initialContent.activeSlideIndex : 0,
  );
  const [sidebarPanel, setSidebarPanel] = createSignal<string | null>("properties");
  let canvasEl: HTMLDivElement | undefined;
  let notesHistoryPushed = false;
  let dragPointerId: number | null = null;
  let dragStartClient = { x: 0, y: 0 };
  let dragMoved = false;
  const DRAG_THRESHOLD = 5;
  const [selectedElementId, setSelectedElementId] = createSignal<string | null>(null);
  const [dragging, setDragging] = createSignal(false);
  const [dragOffset, setDragOffset] = createSignal({ x: 0, y: 0 });
  const [resizing, setResizing] = createSignal<{ id: string; startX: number; startY: number; width: number; height: number } | null>(null);
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

  // --- Undo / Redo snapshot stack ---
  type SlideSnapshot = { slides: Slide[]; activeSlideIndex: number };
  const [slidePast, setSlidePast] = createSignal<SlideSnapshot[]>([]);
  const [slideFuture, setSlideFuture] = createSignal<SlideSnapshot[]>([]);

  const captureSlideSnapshot = (): SlideSnapshot => ({
    slides: JSON.parse(JSON.stringify(slides())),
    activeSlideIndex: activeSlideIndex(),
  });

  const pushSlideHistory = () => {
    setSlidePast((prev) => [...prev, captureSlideSnapshot()].slice(-100));
    setSlideFuture([]);
  };

  const undoSlide = () => {
    const past = slidePast();
    if (!past.length) return;
    const snapshot = past[past.length - 1];
    setSlidePast(past.slice(0, -1));
    setSlideFuture((prev) => [...prev, captureSlideSnapshot()]);
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
    setSlidePast((prev) => [...prev, captureSlideSnapshot()]);
    setSlides(snapshot.slides);
    setActiveSlideIndex(Math.min(snapshot.activeSlideIndex, snapshot.slides.length - 1));
    setSelectedElementId(null);
    emitChange(snapshot.slides);
  };

  const activeSlide = () => slides()[activeSlideIndex()];
  const selectedElement = () => activeSlide()?.elements.find((el) => el.id === selectedElementId()) || null;
  const emitChange = (next: Slide[] = slides()) =>
    props.onChange?.(toDeck(next, props.initialContent, theme(), activeSlideIndex()));

  const canvasToLocal = (clientX: number, clientY: number) => {
    const rect = canvasEl?.getBoundingClientRect();
    if (!rect || rect.width <= 0 || rect.height <= 0) {
      return { x: clientX, y: clientY };
    }
    return {
      x: (clientX - rect.left) * (960 / rect.width),
      y: (clientY - rect.top) * (540 / rect.height),
    };
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
    setTheme(nextTheme);
    props.onChange?.(toDeck(slides(), props.initialContent, nextTheme, activeSlideIndex()));
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

  const rotateSelected = (delta: number) => {
    const el = selectedElement();
    if (!el) return;
    const next = ((el.rotation || 0) + delta) % 360;
    patchSelected({ rotation: next < 0 ? next + 360 : next });
  };

  const addSlide = () => {
    pushSlideHistory();
    const id = `slide-${slides().length + 1}`;
    const newSlide: Slide = {
      id,
      title: `Slide ${slides().length + 1}`,
      elements: [
        {
          id: `el-${Date.now()}`,
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
    pushSlideHistory();
    const slide = activeSlide();
    if (!slide) return;
    const heading = (content: string, x = 100, y = 70, width = 760): SlideElement => ({ id: `el-${Date.now()}-${Math.random()}`, type: "text", x, y, width, height: 60, content, fontSize: 32, color: theme().textColor });
    const nextElements: Record<string, SlideElement[]> = {
      title: [heading(PLACEHOLDER_TITLE, 100, 170, 760), { ...heading(PLACEHOLDER_BODY, 150, 270, 660), fontSize: 22, color: theme().textColor }],
      "title-body": [heading(PLACEHOLDER_TITLE, 80, 45, 800), { ...heading(PLACEHOLDER_BODY, 100, 150, 760), fontSize: 24, color: theme().textColor }],
      section: [heading(PLACEHOLDER_TITLE, 100, 220, 760)],
      "two-column": [heading(PLACEHOLDER_TITLE, 80, 40, 800), { ...heading(PLACEHOLDER_BODY, 70, 160, 360), fontSize: 22, color: theme().textColor }, { ...heading(PLACEHOLDER_BODY, 530, 160, 360), fontSize: 22, color: theme().textColor }],
      "image-caption": [{ id: `el-${Date.now()}-image`, type: "rect", x: 180, y: 80, width: 600, height: 320, content: "", color: theme().accentColor }, { ...heading(PLACEHOLDER_BODY, 150, 430, 660), fontSize: 22, color: theme().textColor }],
      blank: [],
    };
    const next = [...slides()];
    next[activeSlideIndex()] = { ...slide, layout, title: layout, elements: nextElements[layout] || [] };
    setSlides(next);
    emitChange(next);
  };

  const addShape = (type: "rect" | "ellipse" | "line" | "arrow") => {
    pushSlideHistory();
    const el: SlideElement = {
      id: `el-${Date.now()}`,
      type,
      x: 380,
      y: 200,
      width: 200,
      height: 120,
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
    try {
      const input = document.createElement("input");
      input.type = "file";
      input.accept = "image/*";
      input.addEventListener('cancel', () => {
        addImageFromUrl(window.prompt("Image URL"));
      });
      input.onchange = (e) => {
        const file = (e.target as HTMLInputElement).files?.[0];
        if (file) {
          const reader = new FileReader();
          reader.onload = (ev) => addImageFromUrl(ev.target?.result as string);
          reader.readAsDataURL(file);
        } else {
          addImageFromUrl(window.prompt("Image URL"));
        }
      };
      input.click();
    } catch (e) {
      addImageFromUrl(window.prompt("Image URL"));
    }
  };

  const duplicateSlide = () => {
    pushSlideHistory();
    const source = activeSlide();
    if (!source) return;
    const copy: Slide = {
      ...source,
      id: `slide-${Date.now()}`,
      title: `${source.title} copy`,
      elements: source.elements.map((element) => ({ ...element, id: `el-${Date.now()}-${Math.random()}` })),
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
    const id = selectedElementId();
    if (!id) return;
    pushSlideHistory();
    const next = [...slides()];
    const index = activeSlideIndex();
    next[index] = { ...next[index], elements: next[index].elements.filter((el) => el.id !== id) };
    setSlides(next);
    setSelectedElementId(null);
    emitChange(next);
  };


  const nudgeSelected = (dx: number, dy: number) => {
    pushSlideHistory();
    const id = selectedElementId();
    if (!id) return;
    const slide = activeSlide();
    const element = slide?.elements.find((item) => item.id === id);
    if (!element) return;
    element.x = Math.max(0, Math.min(960 - element.width, element.x + dx));
    element.y = Math.max(0, Math.min(540 - element.height, element.y + dy));
    setSlides([...slides()]);
    emitChange();
  };

  const openPresenter = () => {
    const win = window.open("", "redoc-presenter", "popup,width=1280,height=800");
    if (!win) return;
    const deck = slides();
    const start = activeSlideIndex();
    const escape = (value: string) => value.replace(/[&<>\"]/g, (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", "\"": "&quot;" }[character] || character));
    const renderElements = (slide: Slide | undefined) => (slide?.elements || []).map((element) => {
      const rot = element.rotation || 0;
      const entrance = element.entrance === "fade" ? "fade" : "none";
      const fadeStyle = entrance === "fade" ? "opacity:0;transition:opacity .35s ease;" : "";
      const base = `position:absolute;left:${element.x}px;top:${element.y}px;width:${element.width}px;height:${element.height}px;transform:rotate(${rot}deg);${fadeStyle}`;
      const attrs = `data-entrance="${entrance}"`;
      if (element.type === "text") {
        const align = element.align || "center";
        const text = element.bullets
          ? element.content.split("\n").map((line) => `• ${line}`).join("<br/>")
          : escape(element.content);
        const fw = element.bold ? "bold" : "normal";
        const fs = element.italic ? "italic" : "normal";
        const td = element.underline ? "underline" : "none";
        const ff = element.fontFamily || "Inter, sans-serif";
        return `<div ${attrs} style="${base}font-family:${ff};font-weight:${fw};font-style:${fs};text-decoration:${td};font-size:${element.fontSize || 20}px;color:${element.color || "#111827"};display:flex;align-items:center;justify-content:${align === "left" ? "flex-start" : align === "right" ? "flex-end" : "center"};text-align:${align}">${text}</div>`;
      }
      if (element.type === "image") return `<img ${attrs} src="${escape(element.content)}" alt="" style="${base}object-fit:contain"/>`;
      if (element.type === "line" || element.type === "arrow") {
        const color = escape(element.color || "#3b82f6");
        const arrow = element.type === "arrow" ? `<polygon points="96,0 100,0 100,4" fill="${color}"/>` : "";
        return `<svg ${attrs} viewBox="0 0 100 100" preserveAspectRatio="none" style="${base}"><line x1="0" y1="100" x2="100" y2="0" stroke="${color}" stroke-width="3"/>${arrow}</svg>`;
      }
      return `<div ${attrs} style="${base}background:${element.color || "#3b82f6"};border-radius:${element.type === "ellipse" ? "50%" : "4px"}"></div>`;
    }).join("");
    const payload = JSON.stringify(deck.map((s) => ({
      title: s.title,
      notes: s.notes,
      transition: s.transition,
      bg: theme().bgColor,
      html: renderElements(s),
    })));
    win.document.open();
    win.document.write(`<!doctype html><html><head><title>Presenter view</title><style>
body{margin:0;background:#111827;color:#f8fafc;font-family:Inter,system-ui,sans-serif;display:flex;flex-direction:column;align-items:center;gap:14px;padding:24px}
#slide{width:960px;height:540px;position:relative;box-shadow:0 8px 30px #0008;overflow:hidden;cursor:pointer}
#slide.fade{animation:fadeIn .4s ease}
#slide.slide-left{animation:slideLeft .4s ease}
#slide.slide-right{animation:slideRight .4s ease}
@keyframes fadeIn{from{opacity:0}to{opacity:1}}
@keyframes slideLeft{from{transform:translateX(40px);opacity:.2}to{transform:none;opacity:1}}
@keyframes slideRight{from{transform:translateX(-40px);opacity:.2}to{transform:none;opacity:1}}
#notes{width:960px;background:#1f2937;padding:14px;box-sizing:border-box;border-radius:6px}small{color:#cbd5e1}
</style></head><body>
<div id="slide"></div>
<div id="notes"><strong>Speaker notes</strong><p id="notes-text"></p><small>Next: <span id="next"></span> · Elapsed: <span id="timer">00:00</span> · Space/click reveals · ←/→</small></div>
<script>
const deck=${payload}; let idx=${start}; let seconds=0; let revealIdx=0;
const fadeEls=()=>[...document.querySelectorAll('#slide [data-entrance="fade"]')];
const show=()=>{const s=deck[idx];const el=document.getElementById('slide');el.className='';el.style.background=s.bg;el.innerHTML=s.html;void el.offsetWidth;if(s.transition&&s.transition!=='none')el.className=s.transition;revealIdx=0;document.getElementById('notes-text').textContent=s.notes||'No notes';document.getElementById('next').textContent=(deck[idx+1]&&deck[idx+1].title)||'End of deck'};
const advance=()=>{const pending=fadeEls();if(revealIdx<pending.length){pending[revealIdx].style.opacity='1';revealIdx++;return}if(idx<deck.length-1){idx++;show()}};
const back=()=>{if(revealIdx>0){const pending=fadeEls();revealIdx--;pending[revealIdx].style.opacity='0';return}if(idx>0){idx--;show()}};
show();setInterval(()=>{seconds++;document.getElementById('timer').textContent=String(Math.floor(seconds/60)).padStart(2,'0')+':'+String(seconds%60).padStart(2,'0')},1000);
document.getElementById('slide').addEventListener('click',advance);
window.addEventListener('keydown',(e)=>{if(e.key==='ArrowRight'||e.key===' '){e.preventDefault();advance()}if(e.key==='ArrowLeft'){e.preventDefault();back()}});
</script></body></html>`);
    win.document.close();
    win.focus();
  };

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
        const arrow = element.type === "arrow" ? `<polygon points="96,0 100,0 100,4" fill="${color}"/>` : "";
        return `<svg viewBox="0 0 100 100" preserveAspectRatio="none" style="${base}"><line x1="0" y1="100" x2="100" y2="0" stroke="${color}" stroke-width="3"/>${arrow}</svg>`;
      }
      return `<div style="${base}background:${element.color || "#3b82f6"};border-radius:${element.type === "ellipse" ? "50%" : "4px"}"></div>`;
    }).join("");

    const cols = perPage === 1 ? 1 : 2;
    const rows = perPage === 1 ? 1 : perPage === 2 ? 2 : perPage === 4 ? 2 : 3;
    const scale = perPage === 1 ? 0.72 : perPage === 2 ? 0.48 : perPage === 4 ? 0.36 : 0.28;
    const cellW = Math.round(960 * scale);
    const cellH = Math.round(540 * scale);
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
  .slide{position:relative;width:960px;height:540px;transform-origin:top left}
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
      if (e.key === "Delete" || e.key === "Backspace") {
        if (selectedElementId()) {
          deleteSelectedElement();
          e.preventDefault();
        }
      }
    };
    window.addEventListener("keydown", onKeyDown);

    const slideShortcutIds = ["slide-present-f5", "slide-undo", "slide-redo"];
    shortcutRegistry.register({
      id: "slide-present-f5",
      title: "Start Presentation",
      shortcut: "F5",
      mode: "slide",
      action: () => emitEditorCommand("present"),
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

    onCleanup(() => {
      window.removeEventListener(EDITOR_COMMAND, onCommand);
      window.removeEventListener("keydown", onKeyDown);
      slideShortcutIds.forEach((id) => shortcutRegistry.unregister(id));
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
              {fieldLabel("X", numInput(el().x, (n) => patchSelected({ x: Math.max(0, Math.min(960 - el().width, n)) })))}
              {fieldLabel("Y", numInput(el().y, (n) => patchSelected({ y: Math.max(0, Math.min(540 - el().height, n)) })))}
              {fieldLabel("Width", numInput(el().width, (n) => patchSelected({ width: Math.max(40, Math.min(960 - el().x, n)) })))}
              {fieldLabel("Height", numInput(el().height, (n) => patchSelected({ height: Math.max(30, Math.min(540 - el().y, n)) })))}
              {fieldLabel(
                "Rotation",
                <div style={{ display: "flex", gap: "4px", "align-items": "center" }}>
                  <button type="button" class="g-toolbar-btn" title="Rotate -15°" onClick={() => rotateSelected(-15)}>−</button>
                  {numInput(el().rotation || 0, (n) => patchSelected({ rotation: ((n % 360) + 360) % 360 }))}
                  <button type="button" class="g-toolbar-btn" title="Rotate +15°" onClick={() => rotateSelected(15)}>+</button>
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
            </select>
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
        <Show
          when={selectedElement()}
          fallback={<div style={{ "font-size": "11px", color: "var(--text-muted)" }}>Select an element to set its entrance animation.</div>}
        >
          {(el) => (
            <div style={{ display: "flex", "flex-direction": "column", gap: "8px" }}>
              <div style={{ "font-size": "12px" }}>
                <strong>{el().type}</strong>
                <div style={{ "font-size": "11px", color: "var(--text-muted)", "margin-top": "2px" }}>{el().id}</div>
              </div>
              <label style={{ display: "flex", "align-items": "center", gap: "8px", cursor: "pointer", "font-size": "12px" }}>
                <input
                  type="checkbox"
                  checked={(el().entrance || "none") === "fade"}
                  onChange={(e) => patchSelected({ entrance: e.currentTarget.checked ? "fade" : "none" })}
                />
                Entrance fade
              </label>
              <div style={{ "font-size": "11px", color: "var(--text-muted)" }}>
                In presenter view, faded elements start hidden and reveal in z-order with Space or click.
              </div>
            </div>
          )}
        </Show>
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
        <ToolbarButton title="Print…" onClick={() => setPrintDialogOpen(true)}><IconPrint /></ToolbarButton>
        <ToolbarSep />
        <ToolbarButton title="Cut" onClick={() => document.execCommand("cut")}><IconCut /></ToolbarButton>
        <ToolbarButton title="Copy" onClick={() => document.execCommand("copy")}><IconCopy /></ToolbarButton>
        <ToolbarButton title="Paste" onClick={() => document.execCommand("paste")}><IconPaste /></ToolbarButton>
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
        <ToolbarButton title="Insert Table" onClick={() => showToast("Table insertion in presentations is coming in a future release.", "info")}><IconTable /></ToolbarButton>
        <ToolbarButton title="Insert Chart" onClick={() => showToast("Chart insertion in presentations is coming in a future release.", "info")}><IconChart /></ToolbarButton>
        <ToolbarButton title="Insert Image" onClick={addImage}><IconImage /></ToolbarButton>
        <ToolbarButton title="Insert Text Box" onClick={addTextBox}><IconTextBox /></ToolbarButton>
        <ToolbarSep />
        <ToolbarButton title="Present" onClick={openPresenter}><IconPresent /></ToolbarButton>
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
        {/* Left filmstrip */}
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
          <div style={{ flex: 1, "overflow-y": "auto", padding: "8px 6px", display: "flex", "flex-direction": "column", gap: "8px" }}>
            <For each={slides()}>
              {(slide, idx) => {
                const active = () => idx() === activeSlideIndex();
                const previewText = () => slide.elements.find((e) => e.type === "text")?.content || slide.title;
                const shapeCount = () => slide.elements.filter((e) => e.type !== "text").length;
                return (
                  <div
                    onClick={() => selectSlide(idx())}
                    style={{
                      display: "flex",
                      "align-items": "flex-start",
                      gap: "6px",
                      cursor: "pointer",
                      padding: "2px",
                    }}
                  >
                    <span style={{ "font-size": "11px", color: "var(--text-muted)", width: "14px", "padding-top": "2px", "text-align": "right" }}>
                      {idx() + 1}
                    </span>
                    <div
                      style={{
                        width: "112px",
                        height: "63px",
                        background: theme().bgColor || "white",
                        border: active() ? "2px solid var(--slide-accent)" : "1px solid var(--border-color)",
                        "border-radius": "0",
                        padding: "5px 6px",
                        overflow: "hidden",
                        "box-shadow": active() ? "0 0 0 1px var(--slide-accent)" : "0 1px 3px rgba(0,0,0,.35)",
                        position: "relative",
                      }}
                    >
                      <div style={{
                        "font-size": "7px",
                        "font-weight": "600",
                        color: isPlaceholder(previewText()) ? "#999" : (theme().textColor || "#202124"),
                        "font-style": isPlaceholder(previewText()) ? "italic" : "normal",
                        "line-height": "1.3",
                        overflow: "hidden",
                        "text-overflow": "ellipsis",
                        "white-space": "nowrap",
                      }}>
                        {previewText()}
                      </div>
                      <Show when={shapeCount() > 0}>
                        <div style={{
                          position: "absolute",
                          right: "4px",
                          bottom: "3px",
                          width: "10px",
                          height: "7px",
                          background: theme().accentColor || "#3b82f6",
                          opacity: "0.7",
                        }} />
                      </Show>
                    </div>
                  </div>
                );
              }}
            </For>
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
            ref={(el) => { canvasEl = el; }}
            class={`g-slide-canvas g-print-slide ${slideAnimClass() === "fade" ? "g-slide-anim-fade" : ""} ${slideAnimClass() === "slide-left" ? "g-slide-anim-left" : ""} ${slideAnimClass() === "slide-right" ? "g-slide-anim-right" : ""}`}
            style={{
              width: "960px",
              height: "540px",
              background: theme().bgColor,
              "box-shadow": "0 2px 12px rgba(0,0,0,.45)",
              "border-radius": "0",
              position: "relative",
              overflow: "hidden",
              "flex-shrink": "0",
            }}
            tabindex="0"
            role="region"
            aria-label="Slide canvas"
            onContextMenu={(event) => {
              event.preventDefault();
              const items: ContextMenuItem[] = [
                { id: "new-slide", label: "New Slide", action: addSlide },
                { id: "duplicate-slide", label: "Duplicate Slide", action: duplicateSlide },
                { id: "sep1", label: "", separator: true },
                { id: "insert-text", label: "Insert Text Box", action: addTextBox },
                { id: "insert-image", label: "Insert Image", action: addImage },
                { id: "sep2", label: "", separator: true },
                { id: "bring-front", label: "Bring to Front", disabled: !selectedElementId(), action: () => reorderSelected(true) },
                { id: "send-back", label: "Send to Back", disabled: !selectedElementId(), action: () => reorderSelected(false) },
                { id: "rotate", label: "Rotate +15°", disabled: !selectedElementId(), action: () => rotateSelected(15) },
              ];
              setContextMenu({ x: event.clientX, y: event.clientY, items });
            }}
            onKeyDown={(event) => {
              const amount = event.shiftKey ? 10 : 1;
              if (event.key === "ArrowLeft") nudgeSelected(-amount, 0);
              else if (event.key === "ArrowRight") nudgeSelected(amount, 0);
              else if (event.key === "ArrowUp") nudgeSelected(0, -amount);
              else if (event.key === "ArrowDown") nudgeSelected(0, amount);
              else return;
              event.preventDefault();
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
                        height: "540px",
                        background: "#f43f5e",
                        "pointer-events": "none",
                        "z-index": 10,
                      }
                    : {
                        position: "absolute",
                        left: "0",
                        top: `${guide.position}px`,
                        width: "960px",
                        height: "1px",
                        background: "#f43f5e",
                        "pointer-events": "none",
                        "z-index": 10,
                      }}
                />
              )}
            </For>
            <For each={activeSlide()?.elements || []}>
              {(el) => {
                const isSelected = () => el.id === selectedElementId();
                const placeholder = () => el.type === "text" && isPlaceholder(el.content);
                return (
                  <div
                    onClick={() => setSelectedElementId(el.id)}
                    onPointerDown={(event) => {
                      if (el.type === "text" && document.activeElement === event.currentTarget.querySelector("[contenteditable]")) {
                        return;
                      }
                      event.stopPropagation();
                      dragPointerId = event.pointerId;
                      dragStartClient = { x: event.clientX, y: event.clientY };
                      dragMoved = false;
                      setSelectedElementId(el.id);
                      setAlignmentGuides([]);
                      const local = canvasToLocal(event.clientX, event.clientY);
                      setDragOffset({ x: local.x - el.x, y: local.y - el.y });
                    }}
                    onPointerMove={(event) => {
                      const resize = resizing();
                      if (resize?.id === el.id) {
                        const local = canvasToLocal(event.clientX, event.clientY);
                        const startLocal = canvasToLocal(resize.startX, resize.startY);
                        el.width = Math.max(40, Math.min(960 - el.x, resize.width + local.x - startLocal.x));
                        el.height = Math.max(30, Math.min(540 - el.y, resize.height + local.y - startLocal.y));
                        setAlignmentGuides([]);
                        setSlides([...slides()]);
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
                      }
                      const local = canvasToLocal(event.clientX, event.clientY);
                      const snapped = snapElementPosition(
                        el,
                        local.x - dragOffset().x,
                        local.y - dragOffset().y,
                        activeSlide()?.elements || [],
                      );
                      el.x = snapped.x;
                      el.y = snapped.y;
                      dragMoved = true;
                      setAlignmentGuides(snapped.guides);
                      setSlides([...slides()]);
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
                          if (isPlaceholder(e.currentTarget.innerText)) {
                            e.currentTarget.innerText = "";
                            el.content = "";
                          }
                        }}
                        onBlur={(e) => {
                          const text = e.currentTarget.innerText.trim();
                          el.content = text || (el.fontSize && el.fontSize >= 28 ? PLACEHOLDER_TITLE : PLACEHOLDER_BODY);
                          e.currentTarget.innerText = el.content;
                          setSlides([...slides()]);
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
                            el.rotation = ((startRot + delta) % 360 + 360) % 360;
                            setSlides([...slides()]);
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
                    {el.type === "rect" && (
                      <div
                        style={{
                          width: "100%",
                          height: "100%",
                          background: el.color || "#3b82f6",
                          "border-radius": "4px",
                        }}
                      />
                    )}
                    {el.type === "ellipse" && (
                      <div
                        style={{
                          width: "100%",
                          height: "100%",
                          background: el.color || "#3b82f6",
                          "border-radius": "50%",
                        }}
                      />
                    )}
                    {(el.type === "line" || el.type === "arrow") && (
                      <svg width="100%" height="100%" viewBox="0 0 100 100" preserveAspectRatio="none" aria-label={el.type === "arrow" ? "Arrow" : "Line"}>
                        <defs>
                          <marker id={`arrowhead-${el.id}`} markerWidth="8" markerHeight="8" refX="7" refY="4" orient="auto">
                            <path d="M0,0 L8,4 L0,8 z" fill={el.color || "#3b82f6"} />
                          </marker>
                        </defs>
                        <line x1="0" y1="100" x2="100" y2="0" stroke={el.color || "#3b82f6"} stroke-width="3" marker-end={el.type === "arrow" ? `url(#arrowhead-${el.id})` : undefined} />
                      </svg>
                    )}
                    {el.type === "image" && (
                      <img src={el.content} alt="Slide asset" style={{ width: "100%", height: "100%", "object-fit": "contain", "pointer-events": "none" }} />
                    )}
                    <Show when={isSelected()}>
                      <div
                        role="button"
                        aria-label="Resize element"
                        onPointerDown={(event) => {
                          event.stopPropagation();
                          pushSlideHistory();
                          event.currentTarget.setPointerCapture(event.pointerId);
                          setAlignmentGuides([]);
                          setResizing({ id: el.id, startX: event.clientX, startY: event.clientY, width: el.width, height: el.height });
                        }}
                        onPointerMove={(event) => {
                          const resize = resizing();
                          if (resize?.id !== el.id) return;
                          el.width = Math.max(40, Math.min(960 - el.x, resize.width + event.clientX - resize.startX));
                          el.height = Math.max(30, Math.min(540 - el.y, resize.height + event.clientY - resize.startY));
                          setAlignmentGuides([]);
                          setSlides([...slides()]);
                        }}
                        onPointerUp={(event) => {
                          setResizing(null);
                          setAlignmentGuides([]);
                          event.currentTarget.releasePointerCapture(event.pointerId);
                          commitChange();
                        }}
                        style={{ position: "absolute", right: "-6px", bottom: "-6px", width: "12px", height: "12px", background: "var(--slide-accent)", border: "2px solid white", "border-radius": "2px", cursor: "nwse-resize", "z-index": 2 }}
                      />
                    </Show>
                  </div>
                );
              }}
            </For>
          </div>

          {/* Notes pane */}
          <div
            style={{
              width: "960px",
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
                if (!notesHistoryPushed) {
                  pushSlideHistory();
                  notesHistoryPushed = true;
                }
                list[idx] = { ...slide, notes: e.currentTarget.value };
                setSlides(list);
                emitChange(list);
              }}
              onBlur={() => {
                notesHistoryPushed = false;
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
          </div>
        </main>

        <IconSidebar panels={sidebarPanels()} activePanel={sidebarPanel()} onActivePanelChange={setSidebarPanel} />
      </div>
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
