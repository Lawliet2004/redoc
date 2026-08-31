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
import { getTemplateBody } from "./shell/templates";
import { buildAppCommands, reorderFileMenuCommands, type EditorMode } from "./shell/registerCommands";
const DocEditor = lazy(() => import("@redoc/doc-editor").then((m) => ({ default: m.DocEditor })));
const SheetEditor = lazy(() => import("@redoc/sheet-editor").then((m) => ({ default: m.SheetEditor })));
const SlideEditor = lazy(() => import("@redoc/slide-editor").then((m) => ({ default: m.SlideEditor })));
const PresenterView = lazy(() => import("@redoc/slide-editor").then((m) => ({ default: m.PresenterView })));

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
    return (
      <Suspense fallback={<div style={{ padding: "24px", color: "#94a3b8" }}>Loading presenter…</div>}>
        <PresenterView />
      </Suspense>
    );
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
        await openFilePath(path);
      }
    } catch {
      // Non-Tauri / older bindings: ignore OS open queue.
    }

    let unlistenOpen: (() => void) | undefined;
    try {
      unlistenOpen = await listenOpenFiles((paths) => {
        for (const path of paths) {
          void openFilePath(path);
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
        if (ext && ["redoc", "csv", "docx", "xlsx", "pptx"].includes(ext)) {
          void openFilePath(path);
          break;
        } else if (ext) {
          showToast(`Unsupported file format: .${ext}. Redoc supports .redoc, .csv, .docx, .xlsx, and .pptx`, "error");
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

    const registered = buildAppCommands({
      activeMode,
      zoomLevel,
      setZoomLevel,
      settings,
      statusInfo,
      docContent,
      handleNewDoc,
      handleSaveAs,
      handleSave,
      handleOpenFile,
      setExportOpen,
      goHome,
      setPaletteOpen,
      setSettingsOpen,
      setHelpOpen,
      setAboutOpen,
      handleSwitchMode,
    });
    registered.forEach((command) => shortcutRegistry.register(command));
    const shellShortcutIds = registerShellShortcuts(shortcutRegistry, {
      open: () => void handleOpenFile(),
      save: () => void handleSave(),
      saveAs: () => void handleSaveAs(),
    });
    reorderFileMenuCommands(shortcutRegistry);

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
        const imported = await commands.importPptxFile(path);
        const title = path.split(/[\\/]/).pop()?.replace(/\.pptx$/i, "") || "Imported presentation";
        setActiveMode("slide");
        setDocTitle(title);
        setCurrentFilePath(null);
        setCurrentDocId(null);
        setDocContent(imported.deck);
        setImportWarnings(imported.warnings);
        setSaveState("Dirty");
        updateModeBuffer("slide", {
          content: imported.deck,
          title,
          filePath: null,
          docId: null,
          saveState: "Dirty",
        });
        recordTelemetry("pptx_imported");
        showToast("Imported PPTX file; save as .redoc to continue editing", "success");
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
