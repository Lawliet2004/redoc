import { createSignal, onMount, onCleanup, Show, For, lazy, Suspense, createEffect } from "solid-js";
import { Button, ToastContainer, showToast, Dialog, t } from "@redoc/ui";
import { IconDoc, IconSheet, IconSlide, IconSettings, IconSave } from "@redoc/icons";
import { CommandPalette, StatusBar, MenuBar, shortcutRegistry, registerShellShortcuts, buildMenus, emitEditorCommand } from "@redoc/editor-common";
import { commands, AppSettings, RecentEntry, RecoveredDoc } from "@redoc/api-client";
import { HomeScreen } from "./HomeScreen";
import { SettingsDialog } from "./SettingsDialog";
import { AboutDialog } from "./AboutDialog";
import "@redoc/ui/theme.css";
import { open, save } from "@tauri-apps/plugin-dialog";
import { CSVImportDialog } from "./CSVImportDialog";
const DocEditor = lazy(() => import("@redoc/doc-editor").then((m) => ({ default: m.DocEditor })));
const SheetEditor = lazy(() => import("@redoc/sheet-editor").then((m) => ({ default: m.SheetEditor })));
const SlideEditor = lazy(() => import("@redoc/slide-editor").then((m) => ({ default: m.SlideEditor })));

type OpenFileEvent = { payload: string[] };

async function listenOpenFiles(handler: (paths: string[]) => void): Promise<() => void> {
  try {
    const mod = await import("@tauri-apps/api/event");
    return await mod.listen<string[]>("redoc-open-file", (event: OpenFileEvent) => {
      handler(event.payload || []);
    });
  } catch {
    return () => undefined;
  }
}

function ModeIcon(props: { mode: "doc" | "sheet" | "slide"; size?: number }) {
  const s = props.size || 28;
  if (props.mode === "sheet") return <IconSheet width={s} height={s} color="var(--sheet-accent)" />;
  if (props.mode === "slide") return <IconSlide width={s} height={s} color="var(--slide-accent)" />;
  return <IconDoc width={s} height={s} color="var(--doc-accent)" />;
}

function PresenterWindow() {
  const [slideIndex, setSlideIndex] = createSignal(0);
  const [elapsedSeconds, setElapsedSeconds] = createSignal(0);

  onMount(() => {
    const timer = setInterval(() => setElapsedSeconds((s) => s + 1), 1000);
    let unlisten: (() => void) | undefined;
    import("@tauri-apps/api/event").then((mod) => {
      mod.listen<{ slideIndex: number }>("slide_changed", (event) => {
        if (typeof event.payload?.slideIndex === "number") {
          setSlideIndex(event.payload.slideIndex);
        }
      }).then((fn) => { unlisten = fn; });
    }).catch(() => undefined);

    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "ArrowRight" || e.key === " ") {
        e.preventDefault();
        const next = slideIndex() + 1;
        setSlideIndex(next);
        void commands.presenterNav(next).catch(() => undefined);
      } else if (e.key === "ArrowLeft") {
        e.preventDefault();
        const prev = Math.max(0, slideIndex() - 1);
        setSlideIndex(prev);
        void commands.presenterNav(prev).catch(() => undefined);
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
    <div style={{ display: "flex", "flex-direction": "column", height: "100vh", background: "#0f172a", color: "#f8fafc", "font-family": "Inter, sans-serif", padding: "20px", "box-sizing": "border-box", gap: "16px" }}>
      <div style={{ display: "flex", "justify-content": "space-between", "align-items": "center", "border-bottom": "1px solid #334155", "padding-bottom": "12px" }}>
        <h2 style={{ margin: 0, "font-size": "18px", color: "#38bdf8" }}>Redoc Presenter View</h2>
        <div style={{ display: "flex", gap: "16px", "font-size": "14px", color: "#94a3b8" }}>
          <span>Slide: <strong style={{ color: "#f8fafc" }}>{slideIndex() + 1}</strong></span>
          <span>Elapsed: <strong style={{ color: "#f8fafc" }}>{formatTimer(elapsedSeconds())}</strong></span>
        </div>
      </div>
      <div style={{ flex: 1, display: "flex", gap: "20px", "min-height": 0 }}>
        <div style={{ flex: 2, background: "#1e293b", border: "1px solid #334155", "border-radius": "8px", display: "flex", "flex-direction": "column", "align-items": "center", "justify-content": "center", padding: "16px" }}>
          <div style={{ width: "100%", height: "100%", display: "flex", "align-items": "center", "justify-content": "center", background: "#020617", "border-radius": "4px", border: "1px solid #475569" }}>
            <div style={{ "font-size": "24px", color: "#f8fafc" }}>Slide {slideIndex() + 1}</div>
          </div>
        </div>
        <div style={{ flex: 1, display: "flex", "flex-direction": "column", gap: "12px", background: "#1e293b", border: "1px solid #334155", "border-radius": "8px", padding: "16px" }}>
          <h3 style={{ margin: 0, "font-size": "14px", color: "#94a3b8" }}>Speaker Notes</h3>
          <div style={{ flex: 1, "font-size": "13px", color: "#cbd5e1", "line-height": "1.5", overflow: "auto" }}>
            No additional notes for slide {slideIndex() + 1}.
          </div>
          <div style={{ display: "flex", gap: "8px", "justify-content": "space-between", "margin-top": "auto" }}>
            <button
              type="button"
              style={{ padding: "6px 12px", background: "#334155", color: "#fff", border: "none", "border-radius": "4px", cursor: "pointer" }}
              onClick={() => {
                const prev = Math.max(0, slideIndex() - 1);
                setSlideIndex(prev);
                void commands.presenterNav(prev).catch(() => undefined);
              }}
            >
              Previous
            </button>
            <button
              type="button"
              style={{ padding: "6px 12px", background: "#0284c7", color: "#fff", border: "none", "border-radius": "4px", cursor: "pointer" }}
              onClick={() => {
                const next = slideIndex() + 1;
                setSlideIndex(next);
                void commands.presenterNav(next).catch(() => undefined);
              }}
            >
              Next
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

function getTemplateBody(templateId: string): any {
  if (templateId === "resume") {
    return {
      type: "doc",
      content: [
        {
          type: "heading",
          attrs: { level: 1, align: "center" },
          content: [{ type: "text", text: "Jane Doe" }],
        },
        {
          type: "paragraph",
          attrs: { align: "center" },
          content: [{ type: "text", text: "Software Engineer • jane.doe@example.com • (555) 019-2834 • City, State" }],
        },
        {
          type: "heading",
          attrs: { level: 2 },
          content: [{ type: "text", text: "Professional Summary" }],
        },
        {
          type: "paragraph",
          content: [{ type: "text", text: "Experienced software developer with expertise in high-performance web applications and desktop tools. Proven track record of delivering clean, testable code." }],
        },
        {
          type: "heading",
          attrs: { level: 2 },
          content: [{ type: "text", text: "Experience" }],
        },
        {
          type: "paragraph",
          content: [
            { type: "text", text: "Senior Engineer — Acme Corp", marks: [{ type: "bold" }] },
            { type: "text", text: " (2022 - Present)" },
          ],
        },
        {
          type: "bullet_list",
          content: [
            {
              type: "list_item",
              content: [{ type: "paragraph", content: [{ type: "text", text: "Led development of core features and improved software build performance by 40%." }] }],
            },
            {
              type: "list_item",
              content: [{ type: "paragraph", content: [{ type: "text", text: "Mentored junior engineers and instituted code review standards." }] }],
            },
          ],
        },
        {
          type: "heading",
          attrs: { level: 2 },
          content: [{ type: "text", text: "Education" }],
        },
        {
          type: "paragraph",
          content: [{ type: "text", text: "B.S. in Computer Science — Tech University, 2021" }],
        },
      ],
    };
  }
  if (templateId === "report") {
    return {
      type: "doc",
      content: [
        {
          type: "heading",
          attrs: { level: 1 },
          content: [{ type: "text", text: "Project Status & Progress Report" }],
        },
        {
          type: "paragraph",
          content: [{ type: "text", text: "Date: August 2026 | Prepared by: Development Team" }],
        },
        {
          type: "heading",
          attrs: { level: 2 },
          content: [{ type: "text", text: "1. Executive Summary" }],
        },
        {
          type: "paragraph",
          content: [{ type: "text", text: "This report details the recent achievements, current status, and key roadmap milestones for the Redoc suite initiative." }],
        },
        {
          type: "heading",
          attrs: { level: 2 },
          content: [{ type: "text", text: "2. Accomplishments & Key Milestones" }],
        },
        {
          type: "bullet_list",
          content: [
            {
              type: "list_item",
              content: [{ type: "paragraph", content: [{ type: "text", text: "Completed core editor architecture and shell integration." }] }],
            },
            {
              type: "list_item",
              content: [{ type: "paragraph", content: [{ type: "text", text: "Implemented comprehensive test coverage across document, sheet, and slide modules." }] }],
            },
          ],
        },
        {
          type: "heading",
          attrs: { level: 2 },
          content: [{ type: "text", text: "3. Next Steps" }],
        },
        {
          type: "paragraph",
          content: [{ type: "text", text: "Finalize remaining file format import/export filters and conduct final performance verification." }],
        },
      ],
    };
  }
  if (templateId === "budget") {
    return {
      sheets: [
        {
          id: "sheet-1",
          name: "Budget Summary",
          cells: {
            "1:1": { raw: "Category", display: "Category", style: { bold: true } },
            "1:2": { raw: "Budgeted", display: "Budgeted", style: { bold: true } },
            "1:3": { raw: "Actual", display: "Actual", style: { bold: true } },
            "1:4": { raw: "Difference", display: "Difference", style: { bold: true } },
            "2:1": { raw: "Housing / Rent" },
            "2:2": { raw: "1500", display: "$1,500" },
            "2:3": { raw: "1500", display: "$1,500" },
            "2:4": { raw: "=C2-B2", display: "$0" },
            "3:1": { raw: "Utilities & Internet" },
            "3:2": { raw: "250", display: "$250" },
            "3:3": { raw: "230", display: "$230" },
            "3:4": { raw: "=C3-B3", display: "-$20" },
            "4:1": { raw: "Groceries & Food" },
            "4:2": { raw: "600", display: "$600" },
            "4:3": { raw: "650", display: "$650" },
            "4:4": { raw: "=C4-B4", display: "$50" },
            "5:1": { raw: "Transportation" },
            "5:2": { raw: "200", display: "$200" },
            "5:3": { raw: "180", display: "$180" },
            "5:4": { raw: "=C5-B5", display: "-$20" },
            "6:1": { raw: "Total", style: { bold: true } },
            "6:2": { raw: "=SUM(B2:B5)", display: "$2,550", style: { bold: true } },
            "6:3": { raw: "=SUM(C2:C5)", display: "$2,560", style: { bold: true } },
            "6:4": { raw: "=SUM(D2:D5)", display: "$10", style: { bold: true } },
          },
          colWidths: { "1": 160, "2": 100, "3": 100, "4": 100 },
          rowHeights: {},
          freezeRows: 1,
          freezeCols: 0,
        },
      ],
      activeSheetIndex: 0,
    };
  }
  if (templateId === "project_plan") {
    return {
      sheets: [
        {
          id: "sheet-1",
          name: "Project Roadmap",
          cells: {
            "1:1": { raw: "Task Name", display: "Task Name", style: { bold: true } },
            "1:2": { raw: "Owner", display: "Owner", style: { bold: true } },
            "1:3": { raw: "Start Date", display: "Start Date", style: { bold: true } },
            "1:4": { raw: "End Date", display: "End Date", style: { bold: true } },
            "1:5": { raw: "Status", display: "Status", style: { bold: true } },
            "2:1": { raw: "Requirement Gathering" },
            "2:2": { raw: "Alice" },
            "2:3": { raw: "2026-08-01" },
            "2:4": { raw: "2026-08-05" },
            "2:5": { raw: "Completed" },
            "3:1": { raw: "Architecture & Specs" },
            "3:2": { raw: "Bob" },
            "3:3": { raw: "2026-08-06" },
            "3:4": { raw: "2026-08-12" },
            "3:5": { raw: "In Progress" },
            "4:1": { raw: "Core Implementation" },
            "4:2": { raw: "Charlie" },
            "4:3": { raw: "2026-08-13" },
            "4:4": { raw: "2026-08-25" },
            "4:5": { raw: "Not Started" },
            "5:1": { raw: "Testing & QA" },
            "5:2": { raw: "Dana" },
            "5:3": { raw: "2026-08-26" },
            "5:4": { raw: "2026-08-31" },
            "5:5": { raw: "Not Started" },
          },
          colWidths: { "1": 180, "2": 100, "3": 110, "4": 110, "5": 120 },
          rowHeights: {},
          freezeRows: 1,
          freezeCols: 0,
        },
      ],
      activeSheetIndex: 0,
    };
  }
  if (templateId === "pitch_deck") {
    return {
      slides: [
        {
          id: "slide-1",
          title: "Title Slide",
          layout: "title",
          transition: "none",
          notes: "Introduce company vision and team.",
          elements: [
            {
              id: "el-1",
              type: "text",
              x: 80,
              y: 140,
              width: 800,
              height: 90,
              content: "Redoc Office Suite",
              fontSize: 42,
              align: "center",
              bold: true,
            },
            {
              id: "el-2",
              type: "text",
              x: 80,
              y: 250,
              width: 800,
              height: 50,
              content: "Next-Generation Productivity Platform",
              fontSize: 22,
              align: "center",
            },
          ],
        },
        {
          id: "slide-2",
          title: "Problem Statement",
          layout: "title-body",
          transition: "fade",
          notes: "Highlight key market pain points.",
          elements: [
            {
              id: "el-3",
              type: "text",
              x: 80,
              y: 50,
              width: 800,
              height: 60,
              content: "The Problem",
              fontSize: 32,
              align: "left",
              bold: true,
            },
            {
              id: "el-4",
              type: "text",
              x: 80,
              y: 140,
              width: 800,
              height: 240,
              content: "• Legacy office software is slow and monolithic.\n• Cloud-only tools lack robust offline capabilities.\n• Fragmented formats cause compatibility issues.",
              fontSize: 20,
              align: "left",
            },
          ],
        },
        {
          id: "slide-3",
          title: "The Solution",
          layout: "title-body",
          transition: "fade",
          notes: "Present Redoc's unified local-first architecture.",
          elements: [
            {
              id: "el-5",
              type: "text",
              x: 80,
              y: 50,
              width: 800,
              height: 60,
              content: "Our Solution",
              fontSize: 32,
              align: "left",
              bold: true,
            },
            {
              id: "el-6",
              type: "text",
              x: 80,
              y: 140,
              width: 800,
              height: 240,
              content: "• Unified desktop engine for Docs, Sheets, and Slides.\n• Blazing-fast native performance with zero telemetry lock-in.\n• Full interoperability with standard document formats.",
              fontSize: 20,
              align: "left",
            },
          ],
        },
      ],
      theme: {
        id: "default-light",
        name: "Modern Light",
        bgColor: "#ffffff",
        textColor: "#1e293b",
        accentColor: "#3b82f6",
        fontFamily: "Inter, sans-serif",
      },
    };
  }
  return null;
}

type EditorMode = "doc" | "sheet" | "slide";
type ModeBuffer = {
  content: any | null;
  title: string;
  filePath: string | null;
  docId: string | null;
  saveState: "Saved" | "Saving" | "Dirty" | "Error";
};

const DEFAULT_MODE_TITLES: Record<EditorMode, string> = {
  doc: "Untitled document",
  sheet: "Untitled spreadsheet",
  slide: "Untitled presentation",
};

const emptyModeBuffer = (mode: EditorMode): ModeBuffer => ({
  content: null,
  title: DEFAULT_MODE_TITLES[mode],
  filePath: null,
  docId: null,
  saveState: "Saved",
});

export function App() {
  if (typeof window !== "undefined" && new URLSearchParams(window.location.search).get("presenter") === "true") {
    return <PresenterWindow />;
  }
  const [activeMode, setActiveMode] = createSignal<"home" | "doc" | "sheet" | "slide">("home");
  const [systemTheme, setSystemTheme] = createSignal(window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light");
  const [docTitle, setDocTitle] = createSignal("Untitled document");
  const [currentFilePath, setCurrentFilePath] = createSignal<string | null>(null);
  const [currentDocId, setCurrentDocId] = createSignal<string | null>(null);
  const [saveState, setSaveState] = createSignal<"Saved" | "Saving" | "Dirty" | "Error">("Saved");
  const [zoomLevel, setZoomLevel] = createSignal(100);
  const [statusInfo, setStatusInfo] = createSignal("");
  const [paletteOpen, setPaletteOpen] = createSignal(false);
  const [helpOpen, setHelpOpen] = createSignal(false);
  const [aboutOpen, setAboutOpen] = createSignal(false);
  const [settingsOpen, setSettingsOpen] = createSignal(false);
  const [exportOpen, setExportOpen] = createSignal(false);
  const [recoveryOpen, setRecoveryOpen] = createSignal(false);
  const [recoveredDocs, setRecoveredDocs] = createSignal<RecoveredDoc[]>([]);
  const [importWarnings, setImportWarnings] = createSignal<string[]>([]);
  const [csvImportOpen, setCsvImportOpen] = createSignal(false);
  const [csvImportPath, setCsvImportPath] = createSignal<string | null>(null);
  let csvImportResolver: ((workbook: any | undefined) => void) | undefined;

  const [settings, setSettings] = createSignal<AppSettings>({
    theme: "dark",
    autosaveIntervalMs: 2000,
    spellcheckEnabled: true,
    fontSizeDefault: 12,
    telemetryEnabled: false,
    zoomLevel: 100,
    checkForUpdates: true,
  });

  const [recents, setRecents] = createSignal<RecentEntry[]>([]);
  const [docContent, setDocContent] = createSignal<any>(null);
  const [modeBuffers, setModeBuffers] = createSignal<Record<EditorMode, ModeBuffer>>({
    doc: emptyModeBuffer("doc"),
    sheet: emptyModeBuffer("sheet"),
    slide: emptyModeBuffer("slide"),
  });
  let autosaveTimer: number | undefined;

  const cancelAutosave = () => {
    if (autosaveTimer !== undefined) {
      window.clearTimeout(autosaveTimer);
      autosaveTimer = undefined;
    }
  };

  const updateWindowTitle = async () => {
    if (activeMode() === "home") return;
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      const path = currentFilePath();
      const base = path ? path.split(/[\\/]/).pop()! : docTitle();
      const prefix = saveState() === "Dirty" ? "• " : "";
      await getCurrentWindow().setTitle(`${prefix}${base} - Redoc`);
    } catch {
      // Not running inside Tauri.
    }
  };

  createEffect(() => {
    docTitle();
    currentFilePath();
    saveState();
    activeMode();
    void updateWindowTitle();
  });
  const telemetryBuildEnabled = import.meta.env.VITE_TELEMETRY === "1";
  const recordTelemetry = (event: string) => {
    if (telemetryBuildEnabled && settings().telemetryEnabled) {
      void commands.recordTelemetryEvent(event).catch(() => undefined);
    }
  };

  const maybeCheckForUpdates = async (enabled: boolean) => {
    if (!enabled) return;
    try {
      // Optional dependency; unsigned builds may omit the updater feature.
      const updater = await import(/* @vite-ignore */ "@tauri-apps/plugin-updater").catch(() => null);
      if (!updater?.check) return;
      const update = await updater.check();
      if (update) {
        showToast(`Update available: ${update.version}`, "info");
      }
    } catch {
      // Updater plugin / certs may be absent in unsigned or non-feature builds.
    }
  };

  const confirmDiscardIfDirty = (): boolean => {
    if (saveState() === "Dirty") {
      return window.confirm("You have unsaved changes. Do you want to discard them?");
    }
    return true;
  };

  const saveCurrentModeToBuffer = () => {
    const mode = activeMode();
    if (mode === "home") return;
    const snapshot: ModeBuffer = {
      content: docContent(),
      title: docTitle(),
      filePath: currentFilePath(),
      docId: currentDocId(),
      saveState: saveState(),
    };
    setModeBuffers((prev) => ({ ...prev, [mode]: snapshot }));
  };

  const updateModeBuffer = (mode: EditorMode, buffer: Partial<ModeBuffer>) => {
    setModeBuffers((prev) => ({
      ...prev,
      [mode]: { ...prev[mode], ...buffer },
    }));
  };

  const loadModeFromBuffer = (mode: EditorMode) => {
    const buf = modeBuffers()[mode];
    setDocContent(buf.content);
    setDocTitle(buf.title);
    setCurrentFilePath(buf.filePath);
    setCurrentDocId(buf.docId);
    setSaveState(buf.saveState);
  };

  const seedModeBuffer = async (mode: EditorMode) => {
    try {
      const created = await commands.createNewDocument(mode, DEFAULT_MODE_TITLES[mode]);
      const buffer: ModeBuffer = {
        content: created.body,
        title: DEFAULT_MODE_TITLES[mode],
        filePath: null,
        docId: created.meta.id,
        saveState: "Saved",
      };
      updateModeBuffer(mode, buffer);
      setDocTitle(buffer.title);
      setCurrentFilePath(buffer.filePath);
      setCurrentDocId(buffer.docId);
      setDocContent(buffer.content);
      setSaveState(buffer.saveState);
    } catch {
      const buffer = emptyModeBuffer(mode);
      updateModeBuffer(mode, buffer);
      setDocTitle(buffer.title);
      setCurrentFilePath(buffer.filePath);
      setCurrentDocId(buffer.docId);
      setDocContent(buffer.content);
      setSaveState(buffer.saveState);
    }
  };

  const handleSwitchMode = async (mode: EditorMode) => {
    const current = activeMode();
    if (current === mode) return;
    if (!confirmDiscardIfDirty()) return;
    if (current !== "home") {
      saveCurrentModeToBuffer();
    }
    const buf = modeBuffers()[mode];
    setActiveMode(mode);
    if (buf.content === null && buf.docId === null) {
      await seedModeBuffer(mode);
    } else {
      loadModeFromBuffer(mode);
    }
  };

  const goHome = () => {
    if (!confirmDiscardIfDirty()) return;
    saveCurrentModeToBuffer();
    setActiveMode("home");
  };

  const openRedocAtPath = async (path: string) => {
    cancelAutosave();
    try {
      const opened = await commands.openDocument(path);
      const mode = opened.meta.mode;
      if (mode === "doc" || mode === "sheet" || mode === "slide") {
        setActiveMode(mode);
      }
      setDocTitle(opened.meta.title);
      setCurrentFilePath(path);
      setCurrentDocId(opened.meta.id);
      setDocContent(opened.body);
      setSaveState("Saved");
      if (mode === "doc" || mode === "sheet" || mode === "slide") {
        updateModeBuffer(mode, {
          content: opened.body,
          title: opened.meta.title,
          filePath: path,
          docId: opened.meta.id,
          saveState: "Saved",
        });
      }
      setRecents(await commands.getRecents());
      showToast(`Opened ${opened.meta.title}`, "success");
    } catch (error) {
      showToast(`Could not open file: ${error}`, "error");
    }
  };

  const modeAccent = () => {
    const m = activeMode();
    if (m === "sheet") return "var(--sheet-accent)";
    if (m === "slide") return "var(--slide-accent)";
    return "var(--doc-accent)";
  };

  const scheduleAutosave = (content: any) => {
    cancelAutosave();
    const id = currentDocId();
    if (!id) return;
    const mode = activeMode();
    const title = docTitle();
    if (mode === "home") return;
    autosaveTimer = window.setTimeout(() => {
      void commands.autosaveDocument(id, mode, title, content);
    }, Math.max(500, settings().autosaveIntervalMs));
  };

  const openRecoveredDocument = async (snapshotPath: string) => {
    try {
      const opened = await commands.openRecoveredDocument(snapshotPath);
      const mode = opened.meta.mode;
      if (mode === "doc" || mode === "sheet" || mode === "slide") {
        setActiveMode(mode);
      }
      setDocTitle(opened.meta.title);
      setCurrentFilePath(null);
      setCurrentDocId(opened.meta.id);
      setDocContent(opened.body);
      setSaveState("Dirty");
      if (mode === "doc" || mode === "sheet" || mode === "slide") {
        updateModeBuffer(mode, {
          content: opened.body,
          title: opened.meta.title,
          filePath: null,
          docId: opened.meta.id,
          saveState: "Dirty",
        });
      }
      setRecoveryOpen(false);
      showToast(`Restored ${opened.meta.title}`, "success");
    } catch (error) {
      showToast(`Could not restore snapshot: ${error}`, "error");
    }
  };

  const discardAllRecovery = async () => {
    try {
      await commands.discardRecoverySnapshots();
      setRecoveredDocs([]);
      setRecoveryOpen(false);
      showToast("Discarded recovered snapshots", "info");
    } catch (error) {
      showToast(`Could not discard snapshots: ${error}`, "error");
    }
  };

  onMount(async () => {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const mqHandler = (e: MediaQueryListEvent) => setSystemTheme(e.matches ? "dark" : "light");
    mq.addEventListener("change", mqHandler);

    let unlistenClose: (() => void) | undefined;
    const onWindowError = (event: ErrorEvent) => {
      void commands.logFrontendError("error", event.message || "Unknown error", event.error?.stack ?? null).catch(() => undefined);
    };
    const onUnhandledRejection = (event: PromiseRejectionEvent) => {
      const reason = event.reason;
      const message = reason instanceof Error ? reason.message : String(reason);
      const stack = reason instanceof Error ? reason.stack ?? null : null;
      void commands.logFrontendError("error", message, stack).catch(() => undefined);
    };
    window.addEventListener("error", onWindowError);
    window.addEventListener("unhandledrejection", onUnhandledRejection);
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      const win = getCurrentWindow();
      unlistenClose = await win.onCloseRequested(async (event) => {
        if (saveState() === "Dirty") {
          const discard = window.confirm("You have unsaved changes. Quit without saving?");
          if (!discard) {
            event.preventDefault();
            return;
          }
        }
        try {
          await commands.markCleanShutdown();
        } catch {
          // Non-Tauri / older bindings.
        }
      });
    } catch {
      // Window API unavailable outside Tauri shell.
    }

    try {
      const s = await commands.getSettings();
      setSettings(s);
      if (typeof s.zoomLevel === "number" && s.zoomLevel > 0) {
        setZoomLevel(s.zoomLevel);
      }
      const r = await commands.getRecents();
      setRecents(r);
      const rec = await commands.checkRecovery();
      if (rec.length > 0) {
        setRecoveredDocs(rec);
        setRecoveryOpen(true);
      }
      void maybeCheckForUpdates(s.checkForUpdates !== false);
    } catch {
      console.warn("Backend state init fallback to local defaults");
    }

    try {
      const pending = await commands.takePendingOpenPaths();
      for (const path of pending) {
        await openRedocAtPath(path);
      }
    } catch {
      // Non-Tauri / older bindings: ignore OS open queue.
    }

    let unlistenOpen: (() => void) | undefined;
    try {
      unlistenOpen = await listenOpenFiles((paths) => {
        for (const path of paths) {
          void openRedocAtPath(path);
        }
      });
    } catch {
      // Event API unavailable outside Tauri shell.
    }

    const handleWindowDragOver = (e: DragEvent) => e.preventDefault();
    const handleWindowDrop = (e: DragEvent) => {
      e.preventDefault();
      const files = e.dataTransfer?.files;
      if (!files || files.length === 0) return;
      for (let i = 0; i < files.length; i++) {
        const file = files[i];
        const path = (file as any).path || file.name;
        const ext = path.split(".").pop()?.toLowerCase();
        if (ext && ["redoc", "csv", "docx", "xlsx"].includes(ext)) {
          void openFilePath(path);
          break;
        } else if (ext) {
          showToast(`Unsupported file format: .${ext}. Redoc supports .redoc, .csv, .docx, and .xlsx`, "error");
        }
      }
    };
    window.addEventListener("dragover", handleWindowDragOver);
    window.addEventListener("drop", handleWindowDrop);

    let unlistenDragDrop: (() => void) | undefined;
    try {
      import("@tauri-apps/api/event").then((mod) => {
        mod.listen<{ paths: string[] }>("tauri://drag-drop", (event) => {
          const paths = event.payload?.paths || (Array.isArray(event.payload) ? event.payload : []);
          for (const path of paths) {
            const ext = path.split(".").pop()?.toLowerCase();
            if (ext && ["redoc", "csv", "docx", "xlsx"].includes(ext)) {
              void openFilePath(path);
              break;
            } else if (ext) {
              showToast(`Unsupported file format: .${ext}. Redoc supports .redoc, .csv, .docx, and .xlsx`, "error");
            }
          }
        }).then((fn) => { unlistenDragDrop = fn; });
      }).catch(() => undefined);
    } catch {
      // outside Tauri
    }

    const handleGlobalKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPaletteOpen(!paletteOpen());
        return;
      }
      const mode = activeMode();
      shortcutRegistry.handleKeyDown(e, mode === "home" ? "doc" : mode);
    };
    window.addEventListener("keydown", handleGlobalKeyDown);

    const registered = [
      {
        id: "new-document",
        title: "New Document",
        shortcut: "Ctrl+Alt+1",
        mode: "global" as const,
        menuPath: ["File", "New Document"],
        action: () => void handleNewDoc("doc"),
      },
      {
        id: "new-spreadsheet",
        title: "New Spreadsheet",
        shortcut: "Ctrl+Alt+2",
        mode: "global" as const,
        menuPath: ["File", "New Spreadsheet"],
        action: () => void handleNewDoc("sheet"),
      },
      {
        id: "new-presentation",
        title: "New Presentation",
        shortcut: "Ctrl+Alt+3",
        mode: "global" as const,
        menuPath: ["File", "New Presentation"],
        action: () => void handleNewDoc("slide"),
      },
      {
        id: "save-as-document",
        title: "Save As…",
        shortcut: "Ctrl+Shift+S",
        mode: "global" as const,
        menuPath: ["File", "Save As…"],
        action: () => void handleSaveAs(),
      },
      {
        id: "export-as",
        title: "Export as…",
        mode: "global" as const,
        menuPath: ["File", "Export as…"],
        separatorBefore: true,
        action: () => setExportOpen(true),
      },
      {
        id: "go-home",
        title: "Go to Home",
        shortcut: "Ctrl+Alt+H",
        mode: "global" as const,
        menuPath: ["File", "Home"],
        separatorBefore: true,
        action: goHome,
      },
      {
        id: "undo",
        title: "Undo",
        shortcut: "Ctrl+Z",
        mode: "global" as const,
        menuPath: ["Edit", "Undo"],
        action: () => emitEditorCommand("undo"),
      },
      {
        id: "redo",
        title: "Redo",
        shortcut: "Ctrl+Y",
        mode: "global" as const,
        menuPath: ["Edit", "Redo"],
        action: () => emitEditorCommand("redo"),
      },
      {
        id: "cut",
        title: "Cut",
        shortcut: "Ctrl+X",
        mode: "global" as const,
        menuPath: ["Edit", "Cut"],
        separatorBefore: true,
        action: () => emitEditorCommand("cut"),
      },
      {
        id: "copy",
        title: "Copy",
        shortcut: "Ctrl+C",
        mode: "global" as const,
        menuPath: ["Edit", "Copy"],
        action: () => emitEditorCommand("copy"),
      },
      {
        id: "paste",
        title: "Paste",
        shortcut: "Ctrl+V",
        mode: "global" as const,
        menuPath: ["Edit", "Paste"],
        action: () => emitEditorCommand("paste"),
      },
      {
        id: "find",
        title: "Find…",
        shortcut: "Ctrl+F",
        mode: "global" as const,
        menuPath: ["Edit", "Find…"],
        separatorBefore: true,
        action: () => emitEditorCommand("find"),
      },
      {
        id: "replace",
        title: "Replace…",
        shortcut: "Ctrl+H",
        mode: "global" as const,
        menuPath: ["Edit", "Replace…"],
        action: () => emitEditorCommand("find-replace"),
      },
      {
        id: "command-palette",
        title: "Command Palette",
        shortcut: "Ctrl+K",
        mode: "global" as const,
        menuPath: ["Edit", "Command Palette"],
        separatorBefore: true,
        action: () => setPaletteOpen(true),
      },
      {
        id: "zoom-in",
        title: "Zoom In",
        mode: "global" as const,
        menuPath: ["View", "Zoom In"],
        action: () => {
          const next = Math.min(200, zoomLevel() + 10);
          setZoomLevel(next);
          void commands.updateSettings({ ...settings(), zoomLevel: next }).catch(() => undefined);
        },
      },
      {
        id: "zoom-out",
        title: "Zoom Out",
        mode: "global" as const,
        menuPath: ["View", "Zoom Out"],
        action: () => {
          const next = Math.max(50, zoomLevel() - 10);
          setZoomLevel(next);
          void commands.updateSettings({ ...settings(), zoomLevel: next }).catch(() => undefined);
        },
      },
      {
        id: "zoom-100",
        title: "Zoom 100%",
        mode: "global" as const,
        menuPath: ["View", "Zoom 100%"],
        action: () => {
          setZoomLevel(100);
          void commands.updateSettings({ ...settings(), zoomLevel: 100 }).catch(() => undefined);
        },
      },
      {
        id: "insert-image",
        title: "Image…",
        mode: "global" as const,
        menuPath: ["Insert", "Image…"],
        disabled: () => activeMode() === "home",
        action: () => emitEditorCommand("insert-image"),
      },
      {
        id: "insert-table",
        title: "Table",
        mode: "doc" as const,
        menuPath: ["Insert", "Table"],
        disabled: () => activeMode() !== "doc",
        action: () => emitEditorCommand("insert-table"),
      },
      {
        id: "insert-link",
        title: "Hyperlink…",
        mode: "doc" as const,
        menuPath: ["Insert", "Hyperlink…"],
        disabled: () => activeMode() !== "doc",
        action: () => emitEditorCommand("insert-link"),
      },
      {
        id: "insert-chart",
        title: "Chart…",
        mode: "sheet" as const,
        menuPath: ["Insert", "Chart…"],
        disabled: () => activeMode() !== "sheet",
        action: () => emitEditorCommand("insert-chart"),
      },
      {
        id: "insert-page-break",
        title: "Page Break",
        mode: "doc" as const,
        menuPath: ["Insert", "Page Break"],
        disabled: () => activeMode() !== "doc",
        action: () => emitEditorCommand("insert-page-break"),
      },
      {
        id: "clear-formatting",
        title: "Clear Direct Formatting",
        mode: "global" as const,
        menuPath: ["Format", "Clear Direct Formatting"],
        separatorBefore: true,
        action: () => emitEditorCommand("clear-formatting"),
      },
      {
        id: "style-default",
        title: "Default Paragraph Style",
        mode: "doc" as const,
        menuPath: ["Styles", "Default Paragraph Style"],
        action: () => emitEditorCommand("style-default"),
      },
      {
        id: "style-h1",
        title: "Heading 1",
        mode: "doc" as const,
        menuPath: ["Styles", "Heading 1"],
        action: () => emitEditorCommand("style-h1"),
      },
      {
        id: "style-h2",
        title: "Heading 2",
        mode: "doc" as const,
        menuPath: ["Styles", "Heading 2"],
        action: () => emitEditorCommand("style-h2"),
      },
      {
        id: "sheet-insert",
        title: "Insert Sheet…",
        mode: "sheet" as const,
        menuPath: ["Sheet", "Insert Sheet…"],
        action: () => emitEditorCommand("insert-sheet"),
      },
      {
        id: "sheet-delete",
        title: "Delete Sheet",
        mode: "sheet" as const,
        menuPath: ["Sheet", "Delete Sheet"],
        disabled: () => activeMode() !== "sheet" || (docContent()?.sheets?.length ?? 0) <= 1,
        action: () => emitEditorCommand("delete-sheet"),
      },
      {
        id: "sheet-sort-asc",
        title: "Sort Ascending",
        mode: "sheet" as const,
        menuPath: ["Sheet", "Sort Ascending"],
        separatorBefore: true,
        action: () => emitEditorCommand("sort-asc"),
      },
      {
        id: "sheet-sort-multi",
        title: "Sort…",
        mode: "sheet" as const,
        menuPath: ["Sheet", "Sort…"],
        action: () => emitEditorCommand("sort-multi"),
      },
      {
        id: "sheet-filter",
        title: "AutoFilter",
        mode: "sheet" as const,
        menuPath: ["Sheet", "AutoFilter"],
        action: () => emitEditorCommand("filter"),
      },
      {
        id: "slide-new",
        title: "New Slide",
        mode: "slide" as const,
        menuPath: ["Slide", "New Slide"],
        action: () => emitEditorCommand("new-slide"),
      },
      {
        id: "slide-delete",
        title: "Delete Slide",
        mode: "slide" as const,
        menuPath: ["Slide", "Delete Slide"],
        action: () => emitEditorCommand("delete-slide"),
      },
      {
        id: "slide-layout",
        title: "Layout…",
        mode: "slide" as const,
        menuPath: ["Slide", "Layout…"],
        separatorBefore: true,
        action: () => emitEditorCommand("slide-layout"),
      },
      {
        id: "slide-present",
        title: "Start from Beginning",
        mode: "slide" as const,
        menuPath: ["Slide", "Start from Beginning"],
        action: () => emitEditorCommand("present"),
      },
      {
        id: "table-insert",
        title: "Insert Table…",
        mode: "doc" as const,
        menuPath: ["Table", "Insert Table…"],
        disabled: () => activeMode() !== "doc",
        action: () => emitEditorCommand("insert-table"),
      },
      {
        id: "word-count",
        title: "Word Count",
        mode: "doc" as const,
        menuPath: ["Tools", "Word Count"],
        disabled: () => activeMode() !== "doc",
        action: () => statusInfo() && showToast(statusInfo(), "success"),
      },
      {
        id: "options",
        title: "Options…",
        mode: "global" as const,
        menuPath: ["Tools", "Options…"],
        action: () => setSettingsOpen(true),
      },
      {
        id: "window-home",
        title: "Home",
        mode: "global" as const,
        menuPath: ["Window", "Home"],
        action: goHome,
      },
      {
        id: "window-doc",
        title: "Document",
        mode: "global" as const,
        menuPath: ["Window", "Document"],
        separatorBefore: true,
        action: () => void handleSwitchMode("doc"),
      },
      {
        id: "window-sheet",
        title: "Spreadsheet",
        mode: "global" as const,
        menuPath: ["Window", "Spreadsheet"],
        action: () => void handleSwitchMode("sheet"),
      },
      {
        id: "window-slide",
        title: "Presentation",
        mode: "global" as const,
        menuPath: ["Window", "Presentation"],
        action: () => void handleSwitchMode("slide"),
      },
      {
        id: "help-shortcuts",
        title: "Keyboard Shortcuts",
        shortcut: "F1",
        mode: "global" as const,
        menuPath: ["Help", "Keyboard Shortcuts"],
        action: () => setHelpOpen(true),
      },
      {
        id: "help-about",
        title: "About Redoc",
        mode: "global" as const,
        menuPath: ["Help", "About Redoc"],
        action: () => setAboutOpen(true),
      },
      {
        id: "help-command-palette",
        title: "Command Palette",
        shortcut: "Ctrl+K",
        mode: "global" as const,
        menuPath: ["Help", "Command Palette"],
        action: () => setPaletteOpen(true),
      },
    ];
    registered.forEach((command) => shortcutRegistry.register(command));
    const shellShortcutIds = registerShellShortcuts(shortcutRegistry, {
      open: () => void handleOpenFile(),
      save: () => void handleSave(),
      saveAs: () => void handleSaveAs(),
    });
    // Canonical File menu order: New* → Open/Save/Save As/Print → Export → Home
    for (const id of [
      "new-document",
      "new-spreadsheet",
      "new-presentation",
      "open-document",
      "save-document",
      "save-as-document",
      "print",
      "export-as",
      "go-home",
    ]) {
      const cmd = shortcutRegistry.get(id);
      if (!cmd) continue;
      shortcutRegistry.unregister(id);
      shortcutRegistry.register({
        ...cmd,
        separatorBefore: id === "open-document" || id === "export-as" || id === "go-home",
      });
    }
    onCleanup(() => {
      mq.removeEventListener("change", mqHandler);
      window.removeEventListener("error", onWindowError);
      window.removeEventListener("unhandledrejection", onUnhandledRejection);
      unlistenClose?.();
      window.removeEventListener("keydown", handleGlobalKeyDown);
      window.removeEventListener("dragover", handleWindowDragOver);
      window.removeEventListener("drop", handleWindowDrop);
      registered.forEach((command) => shortcutRegistry.unregister(command.id));
      shellShortcutIds.forEach((id) => shortcutRegistry.unregister(id));
      cancelAutosave();
      unlistenOpen?.();
      unlistenDragDrop?.();
    });
  });

  const handleNewDoc = async (mode: EditorMode, templateId?: string) => {
    if (!confirmDiscardIfDirty()) return;
    cancelAutosave();
    if (activeMode() !== "home") {
      saveCurrentModeToBuffer();
    }
    const templateTitles: Record<string, string> = {
      resume: "Resume / CV",
      report: "Project Report",
      budget: "Monthly Budget",
      project_plan: "Project Plan",
      pitch_deck: "Pitch Deck",
    };

    const title = templateId && templateTitles[templateId] ? templateTitles[templateId] : DEFAULT_MODE_TITLES[mode];
    const initialBody = templateId ? getTemplateBody(templateId) : null;

    setActiveMode(mode);
    setDocTitle(title);
    setCurrentFilePath(null);
    setSaveState("Saved");

    let content: any = initialBody;
    let docId: string | null = null;
    try {
      const created = await commands.createNewDocument(mode, title);
      content = initialBody || created.body;
      docId = created.meta.id;
      setDocContent(content);
      setCurrentDocId(docId);
    } catch {
      setDocContent(initialBody || null);
    }
    updateModeBuffer(mode, {
      content,
      title,
      filePath: null,
      docId,
      saveState: "Saved",
    });
    showToast(`Created ${title}`, "success");
  };

  const requestCsvImport = (path: string) => {
    setCsvImportPath(path);
    setCsvImportOpen(true);
    return new Promise<any | undefined>((resolve) => {
      csvImportResolver = resolve;
    });
  };

  const openFilePath = async (path: string) => {
    if (!confirmDiscardIfDirty()) return;
    cancelAutosave();
    try {
      const extension = path.split(".").pop()?.toLowerCase();
      if (extension === "pptx") {
        showToast("PPTX import is not yet supported", "info");
        return;
      }
      if (extension === "csv" || extension === "xlsx") {
        if (extension === "csv") {
          const workbook = await requestCsvImport(path);
          if (!workbook) return;
          setActiveMode("sheet");
          setDocTitle(path.split(/[\\/]/).pop()?.replace(/\.csv$/i, "") || "Imported spreadsheet");
          setCurrentFilePath(null);
          setCurrentDocId(null);
          setDocContent(workbook);
          setSaveState("Dirty");
          updateModeBuffer("sheet", {
            content: workbook,
            title: path.split(/[\\/]/).pop()?.replace(/\.csv$/i, "") || "Imported spreadsheet",
            filePath: null,
            docId: null,
            saveState: "Dirty",
          });
          recordTelemetry("csv_imported");
          showToast("Imported CSV file", "success");
          return;
        }
        const imported = await commands.importXlsxFile(path);
        const title = path.split(/[\\/]/).pop()?.replace(/\.xlsx$/i, "") || "Imported spreadsheet";
        setActiveMode("sheet");
        setDocTitle(title);
        setCurrentFilePath(null);
        setCurrentDocId(null);
        setDocContent(imported.workbook);
        setImportWarnings(imported.warnings);
        setSaveState("Dirty");
        updateModeBuffer("sheet", {
          content: imported.workbook,
          title,
          filePath: null,
          docId: null,
          saveState: "Dirty",
        });
        recordTelemetry("xlsx_imported");
        showToast("Imported XLSX file", "success");
        return;
      }
      if (extension === "docx") {
        const imported = await commands.importDocxFile(path);
        const title = path.split(/[\\/]/).pop()?.replace(/\.docx$/i, "") || "Imported document";
        setActiveMode("doc");
        setDocTitle(title);
        setCurrentFilePath(null);
        setCurrentDocId(null);
        setDocContent(imported.document);
        setImportWarnings(imported.warnings);
        setSaveState("Dirty");
        updateModeBuffer("doc", {
          content: imported.document,
          title,
          filePath: null,
          docId: null,
          saveState: "Dirty",
        });
        recordTelemetry("docx_imported");
        showToast("Imported DOCX file", "success");
        return;
      }
      await openRedocAtPath(path);
    } catch (error) {
      showToast(`Could not open file: ${error}`, "error");
    }
  };

  const handleOpenFile = async () => {
    const path = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "Redoc and office files", extensions: ["redoc", "csv", "xlsx", "docx", "pptx"] }],
    });
    if (!path || Array.isArray(path)) return;
    await openFilePath(path);
  };

  const handleSave = async () => {
    cancelAutosave();
    const path =
      currentFilePath() ||
      (await save({
        defaultPath: `${docTitle().replace(/[^a-z0-9_-]+/gi, "_")}.redoc`,
        filters: [{ name: "Redoc document", extensions: ["redoc"] }],
      }));
    if (!path) return;
    setSaveState("Saving");
    try {
      const saved = await commands.saveDocument(path, activeMode() as any, docTitle(), docContent(), currentDocId());
      setCurrentDocId(saved.id);
      setCurrentFilePath(path);
      setRecents(await commands.getRecents());
      setSaveState("Saved");
      showToast("Document saved", "success");
    } catch {
      setSaveState("Error");
      showToast("Failed to save document", "error");
    }
  };

  const handleSaveAs = async () => {
    cancelAutosave();
    const path = await save({
      defaultPath: `${docTitle().replace(/[^a-z0-9_-]+/gi, "_")}.redoc`,
      filters: [{ name: "Redoc document", extensions: ["redoc"] }],
    });
    if (!path) return;
    setSaveState("Saving");
    try {
      const saved = await commands.saveDocument(path, activeMode() as any, docTitle(), docContent(), currentDocId());
      setCurrentDocId(saved.id);
      setCurrentFilePath(path);
      setRecents(await commands.getRecents());
      setSaveState("Saved");
      showToast("Document saved", "success");
    } catch {
      setSaveState("Error");
      showToast("Failed to save document", "error");
    }
  };

  const handleExport = async (format: string) => {
    try {
      const path = await save({
        defaultPath: `${docTitle().replace(/[^a-z0-9_-]+/gi, "_")}.${format}`,
        filters: [{ name: format.toUpperCase(), extensions: [format] }],
      });
      if (!path) return;
      await commands.exportDocumentToFile(path, activeMode(), format, docContent() || {}, docTitle());
      showToast(`Exported to ${format.toUpperCase()}`, "success");
      setExportOpen(false);
    } catch (err) {
      showToast(`Export failed: ${err}`, "error");
    }
  };

  const handleImportCsv = async () => {
    const path = await open({ multiple: false, directory: false, filters: [{ name: "CSV", extensions: ["csv"] }] });
    if (!path || Array.isArray(path)) return undefined;
    return requestCsvImport(path);
  };

  const cancelCsvImport = () => {
    setCsvImportOpen(false);
    setCsvImportPath(null);
    csvImportResolver?.(undefined);
    csvImportResolver = undefined;
  };

  const confirmCsvImport = (workbook: any) => {
    const path = csvImportPath();
    if (!path) return;
    setDocTitle(path.split(/[\\/]/).pop()?.replace(/\.csv$/i, "") || "Imported CSV");
    setCurrentFilePath(null);
    setCurrentDocId(null);
    setSaveState("Dirty");
    setCsvImportOpen(false);
    setCsvImportPath(null);
    csvImportResolver?.(workbook);
    csvImportResolver = undefined;
  };

  const handleExportCsv = async () => {
    const sheet = docContent()?.sheets?.[docContent()?.activeSheetIndex || 0];
    if (!sheet) return;
    const path = await save({
      defaultPath: `${docTitle().replace(/[^a-z0-9_-]+/gi, "_")}.csv`,
      filters: [{ name: "CSV", extensions: ["csv"] }],
    });
    if (!path) return;
    try {
      await commands.exportCsvToFile(path, sheet);
      showToast("CSV exported", "success");
    } catch (error) {
      showToast(`Could not export CSV: ${error}`, "error");
    }
  };

  const menus = () => buildMenus(activeMode() === "home" ? "home" : activeMode());

  return (
    <div
      data-theme={settings().theme === "system" ? systemTheme() : settings().theme}
      style={{
        display: "flex",
        "flex-direction": "column",
        height: "100vh",
        width: "100vw",
        background: "var(--bg-primary)",
        color: "var(--text-primary)",
        "--accent-color": modeAccent(),
        "--accent-hover": modeAccent(),
        "--accent-light":
          activeMode() === "sheet"
            ? "var(--g-green-light)"
            : activeMode() === "slide"
              ? "var(--g-yellow-light)"
              : "var(--g-blue-light)",
      } as any}
    >
      <Show when={activeMode() === "home"}>
        <HomeScreen
          recents={recents()}
          onNewDoc={handleNewDoc}
          onOpenFile={handleOpenFile}
          onOpenRecent={async (entry) => {
            if (!confirmDiscardIfDirty()) return;
            await openRedocAtPath(entry.path);
          }}
          onTogglePin={async (id) => {
            await commands.togglePinRecent(id);
            setRecents(await commands.getRecents());
          }}
          onDropFile={(file) => {
            const path = (file as any).path || file.name;
            const ext = path.split(".").pop()?.toLowerCase();
            if (ext && ["redoc", "csv", "docx", "xlsx"].includes(ext)) {
              void openFilePath(path);
            } else if (ext) {
              showToast(`Unsupported file format: .${ext}. Redoc supports .redoc, .csv, .docx, and .xlsx`, "error");
            }
          }}
        />
        <button
          type="button"
          class="g-icon-btn g-no-print"
          title="Settings"
          onClick={() => setSettingsOpen(true)}
          style={{ position: "fixed", top: "16px", right: "16px", "z-index": "40", background: "var(--bg-surface)", "box-shadow": "var(--shadow-sm)" }}
        >
          <IconSettings />
        </button>
      </Show>

      <Show when={activeMode() !== "home"}>
        {/* LibreOffice-style compact title + menu chrome */}
        <header
          class="g-no-print"
          style={{
            display: "flex",
            "flex-direction": "column",
            background: "var(--bg-menubar)",
            "border-bottom": "1px solid var(--border-color)",
            "flex-shrink": "0",
          }}
        >
          <div
            style={{
              display: "flex",
              "align-items": "center",
              "justify-content": "space-between",
              padding: "2px 8px",
              height: "var(--titlebar-h)",
              gap: "8px",
            }}
          >
            <div style={{ display: "flex", "align-items": "center", gap: "6px", "min-width": "0", flex: 1 }}>
              <button
                type="button"
                title="Home"
                onClick={goHome}
                style={{
                  width: "28px",
                  height: "28px",
                  "border-radius": "var(--radius-sm)",
                  display: "flex",
                  "align-items": "center",
                  "justify-content": "center",
                  "flex-shrink": 0,
                }}
                onMouseEnter={(e) => (e.currentTarget.style.background = "var(--bg-tertiary)")}
                onMouseLeave={(e) => (e.currentTarget.style.background = "transparent")}
              >
                <ModeIcon mode={activeMode() as "doc" | "sheet" | "slide"} size={20} />
              </button>
              <input
                class="g-title-input"
                type="text"
                value={docTitle()}
                onInput={(e) => {
                  setDocTitle(e.currentTarget.value);
                  setSaveState("Dirty");
                }}
                aria-label="Document title"
              />
              <span style={{ "font-size": "11px", color: "var(--text-muted)" }} aria-live="polite" aria-atomic="true">
                {saveState() === "Saved" ? "Saved" : saveState() === "Saving" ? "Saving…" : "Modified"}
              </span>
            </div>

            <div style={{ display: "flex", "align-items": "center", gap: "6px" }}>
              <div class="g-mode-pill" role="tablist" aria-label="Editor mode">
                <button
                  type="button"
                  role="tab"
                  class={activeMode() === "doc" ? "active" : ""}
                  aria-selected={activeMode() === "doc"}
                  onClick={() => {
                    void handleSwitchMode("doc");
                  }}
                  title="Writer (Ctrl+Alt+1)"
                >
                  <IconDoc width="14" height="14" color="var(--doc-accent)" /> Writer
                </button>
                <button
                  type="button"
                  role="tab"
                  class={activeMode() === "sheet" ? "active" : ""}
                  aria-selected={activeMode() === "sheet"}
                  onClick={() => {
                    void handleSwitchMode("sheet");
                  }}
                  title="Calc (Ctrl+Alt+2)"
                >
                  <IconSheet width="14" height="14" color="var(--sheet-accent)" /> Calc
                </button>
                <button
                  type="button"
                  role="tab"
                  class={activeMode() === "slide" ? "active" : ""}
                  aria-selected={activeMode() === "slide"}
                  onClick={() => {
                    void handleSwitchMode("slide");
                  }}
                  title="Impress (Ctrl+Alt+3)"
                >
                  <IconSlide width="14" height="14" color="var(--slide-accent)" /> Impress
                </button>
              </div>
              <Button variant="ghost" size="sm" onClick={() => setSettingsOpen(true)} title="Settings">
                <IconSettings />
              </Button>
              <Button variant="share" onClick={() => void handleSave()} title="Save (Ctrl+S)">
                <IconSave /> Save
              </Button>
            </div>
          </div>
          <MenuBar menus={menus()} />
        </header>

        <main style={{ flex: 1, position: "relative", overflow: "hidden", "min-height": "0" }}>
          <Suspense
            fallback={
              <div style={{ padding: "48px", "text-align": "center", color: "var(--text-muted)" }}>
                Loading editor…
              </div>
            }
          >
            <Show when={activeMode() === "doc"}>
              <DocEditor
                initialContent={docContent()}
                zoomLevel={zoomLevel()}
                onRequestNew={() => void handleNewDoc("doc")}
                onRequestOpen={() => void handleOpenFile()}
                onRequestSave={() => void handleSave()}
                onRequestExportPdf={() => void handleExport("pdf")}
                onChange={(json) => {
                  setDocContent(json);
                  setSaveState("Dirty");
                  scheduleAutosave(json);
                }}
                onWordCountChange={setStatusInfo}
              />
            </Show>
            <Show when={activeMode() === "sheet"}>
              <SheetEditor
                initialContent={docContent()}
                onImportCsv={handleImportCsv}
                onExportCsv={handleExportCsv}
                onRequestNew={() => void handleNewDoc("sheet")}
                onRequestOpen={() => void handleOpenFile()}
                onRequestSave={() => void handleSave()}
                onRequestExportPdf={() => void handleExport("pdf")}
                onChange={(data) => {
                  setDocContent(data);
                  setSaveState("Dirty");
                  scheduleAutosave(data);
                }}
                onCellInfoChange={setStatusInfo}
              />
            </Show>
            <Show when={activeMode() === "slide"}>
              <SlideEditor
                initialContent={docContent()}
                onRequestNew={() => void handleNewDoc("slide")}
                onRequestOpen={() => void handleOpenFile()}
                onRequestSave={() => void handleSave()}
                onRequestExportPdf={() => void handleExport("pdf")}
                onChange={(deck) => {
                  setDocContent(deck);
                  setSaveState("Dirty");
                  scheduleAutosave(deck);
                }}
                onSlideInfoChange={setStatusInfo}
              />
            </Show>
          </Suspense>
        </main>

        <StatusBar
          mode={activeMode() as any}
          saveState={saveState()}
          wordCountInfo={statusInfo()}
          zoomLevel={zoomLevel()}
          onZoomChange={setZoomLevel}
          pageStyle="Default"
          language="English"
        />
      </Show>

      <CommandPalette open={paletteOpen()} onClose={() => setPaletteOpen(false)} />
      <Dialog open={helpOpen()} title="Keyboard shortcuts" onClose={() => setHelpOpen(false)}>
        <div
          style={{
            "min-width": "420px",
            "max-width": "560px",
            "max-height": "60vh",
            overflow: "auto",
            display: "flex",
            "flex-direction": "column",
            gap: "2px",
          }}
        >
          <For each={shortcutRegistry.getAll().filter((c) => c.shortcut).slice().sort((a, b) => a.title.localeCompare(b.title))}>
            {(cmd) => (
              <div
                style={{
                  display: "flex",
                  "justify-content": "space-between",
                  gap: "16px",
                  padding: "6px 4px",
                  "border-bottom": "1px solid var(--border-color)",
                  "font-size": "13px",
                }}
              >
                <span>{cmd.title}</span>
                <kbd
                  style={{
                    "font-family": "var(--font-mono, monospace)",
                    "font-size": "12px",
                    color: "var(--text-muted)",
                    "white-space": "nowrap",
                  }}
                >
                  {cmd.shortcut}
                </kbd>
              </div>
            )}
          </For>
          <div style={{ "margin-top": "12px", "font-size": "12px", color: "var(--text-muted)" }}>
            Press Ctrl+K to open the command palette.
          </div>
        </div>
      </Dialog>
      <SettingsDialog
        open={settingsOpen()}
        settings={settings()}
        onClose={() => setSettingsOpen(false)}
        onSave={async (next) => {
          setSettings(next);
          await commands.updateSettings(next);
          void maybeCheckForUpdates(next.checkForUpdates !== false);
        }}
      />

      <Dialog open={exportOpen()} title="Download" onClose={() => setExportOpen(false)}>
        <div style={{ display: "flex", "flex-direction": "column", gap: "12px" }}>
          <p style={{ "font-size": "13px", color: "var(--text-secondary)" }}>
            Choose a format to download your file:
          </p>
          <div style={{ display: "flex", gap: "8px", "flex-wrap": "wrap" }}>
            <Button variant="primary" onClick={() => handleExport("pdf")}>
              PDF Document
            </Button>
            <Show when={activeMode() === "doc"}>
              <Button variant="secondary" onClick={() => handleExport("docx")}>
                Microsoft Word (.docx)
              </Button>
            </Show>
            <Show when={activeMode() === "sheet"}>
              <Button variant="secondary" onClick={() => handleExport("xlsx")}>
                Microsoft Excel (.xlsx)
              </Button>
            </Show>
            <Show when={activeMode() === "slide"}>
              <Button variant="secondary" onClick={() => handleExport("pptx")}>
                Microsoft PowerPoint (.pptx)
              </Button>
            </Show>
          </div>
        </div>
      </Dialog>

      <Dialog open={importWarnings().length > 0} title="Import report" onClose={() => setImportWarnings([])}>
        <div style={{ display: "flex", "flex-direction": "column", gap: "12px" }}>
          <p style={{ "font-size": "13px", color: "var(--text-secondary)" }}>
            The document was imported, but some unsupported constructs were simplified:
          </p>
          <ul style={{ margin: 0, padding: "0 0 0 20px", color: "var(--text-primary)" }}>
            <For each={importWarnings()}>{(warning) => <li>{warning}</li>}</For>
          </ul>
          <div style={{ display: "flex", "justify-content": "flex-end" }}>
            <Button variant="share" onClick={() => setImportWarnings([])}>
              Continue
            </Button>
          </div>
        </div>
      </Dialog>

      <CSVImportDialog 
        open={csvImportOpen()}
        path={csvImportPath()}
        onClose={cancelCsvImport}
        onImport={confirmCsvImport}
      />

      <Dialog open={recoveryOpen()} title="Recover unsaved work" onClose={() => setRecoveryOpen(false)}>
        <div style={{ display: "flex", "flex-direction": "column", gap: "12px" }}>
          <p style={{ "font-size": "13px", color: "var(--text-secondary)" }}>
            Redoc found unsaved work from a previous session:
          </p>
          <For each={recoveredDocs()}>
            {(doc) => (
              <div
                style={{
                  display: "flex",
                  "align-items": "center",
                  "justify-content": "space-between",
                  padding: "10px 12px",
                  background: "var(--bg-tertiary)",
                  "border-radius": "8px",
                }}
              >
                <span>{doc.title}</span>
                <Button
                  size="sm"
                  variant="share"
                  onClick={() => void openRecoveredDocument(doc.snapshotPath)}
                >
                  Restore
                </Button>
              </div>
            )}
          </For>
          <div style={{ display: "flex", "justify-content": "flex-end", gap: "8px" }}>
            <Button variant="secondary" onClick={() => void discardAllRecovery()}>
              Discard All
            </Button>
          </div>
        </div>
      </Dialog>

      <AboutDialog open={aboutOpen()} onClose={() => setAboutOpen(false)} />

      <ToastContainer />
    </div>
  );
}
