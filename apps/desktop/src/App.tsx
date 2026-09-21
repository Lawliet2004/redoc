import { createSignal, onMount, onCleanup, Show, For, lazy, Suspense, createEffect } from "solid-js";
import { Button, ToastContainer, showToast, Dialog, ConfirmDialog, ErrorBoundary, t } from "@redoc/ui";
import { IconDoc, IconSheet, IconSlide, IconSettings, IconSave } from "@redoc/icons";
import {
  CommandPalette,
  CommandBar,
  Inspector,
  InspectorSection,
  StatusBar,
  shortcutRegistry,
  registerShellShortcuts,
  buildMenus,
  emitEditorCommand,
  handleF6,
  cyclePane,
  chooseAdjacentSession,
  removeDocumentSession,
  upsertDocumentSession,
  supportedFileType,
  type DocumentSession,
} from "@redoc/editor-common";
import { commands, AppSettings, RecentEntry, RecoveredDoc } from "@redoc/api-client";
import { HomeScreen } from "./HomeScreen";
import { SettingsDialog } from "./SettingsDialog";
import { AboutDialog } from "./AboutDialog";
import "@redoc/ui/theme.css";
import "./app.css";
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
  const [docMounted, setDocMounted] = createSignal(false);
  const [sheetMounted, setSheetMounted] = createSignal(false);
  const [slideMounted, setSlideMounted] = createSignal(false);
  createEffect(() => {
    const mode = activeMode();
    if (mode === "doc") setDocMounted(true);
    if (mode === "sheet") setSheetMounted(true);
    if (mode === "slide") setSlideMounted(true);
  });
  const [systemTheme, setSystemTheme] = createSignal(window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light");
  const [docTitle, setDocTitle] = createSignal("Untitled document");
  const [currentFilePath, setCurrentFilePath] = createSignal<string | null>(null);
  const [currentDocId, setCurrentDocId] = createSignal<string | null>(null);
  const [docReadOnly, setDocReadOnly] = createSignal(false);
  const [docReadOnlyWarning, setDocReadOnlyWarning] = createSignal<string | null>(null);
  const [saveState, setSaveState] = createSignal<"Saved" | "Saving" | "Dirty" | "Error">("Saved");
  const [zoomLevel, setZoomLevel] = createSignal(100);
  const [statusInfo, setStatusInfo] = createSignal("");
  const [inspectorOpen, setInspectorOpen] = createSignal(true);
  const [selectionSummary, setSelectionSummary] = createSignal("");
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

  const [confirmState, setConfirmState] = createSignal<{
    open: boolean;
    title: string;
    message: string;
    confirmLabel: string;
    danger: boolean;
  }>({ open: false, title: "", message: "", confirmLabel: "", danger: false });
  let confirmResolver: ((ok: boolean) => void) | undefined;

  const confirmDialog = (opts: { title: string; message: string; confirmLabel?: string; danger?: boolean }) => {
    if (confirmResolver) confirmResolver(false);
    return new Promise<boolean>((resolve) => {
      confirmResolver = resolve;
      setConfirmState({
        open: true,
        title: opts.title,
        message: opts.message,
        confirmLabel: opts.confirmLabel || "Discard changes",
        danger: opts.danger !== false,
      });
    });
  };

  const settleConfirm = (ok: boolean) => {
    setConfirmState((prev) => ({ ...prev, open: false }));
    const resolve = confirmResolver;
    confirmResolver = undefined;
    resolve?.(ok);
  };

  const [settings, setSettings] = createSignal<AppSettings>({
    theme: "dark",
    autosaveIntervalMs: 2000,
    spellcheckEnabled: true,
    fontSizeDefault: 12,
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
  const [openSessions, setOpenSessions] = createSignal<DocumentSession<any>[]>([]);
  const [activeSessionId, setActiveSessionId] = createSignal<string | null>(null);
  let sessionCounter = 0;
  const lastSessionByMode: Record<EditorMode, string | null> = { doc: null, sheet: null, slide: null };

  const rememberCurrentSession = () => {
    const mode = activeMode();
    const id = activeSessionId();
    if (mode !== "home" && id) lastSessionByMode[mode] = id;
  };
  let autosaveTimer: number | undefined;
  let autosaveMaxWaitTimer: number | undefined;
  let firstPendingChangeAt: number | undefined;
  const AUTOSAVE_MAX_WAIT_MS = 10_000;

  const newSessionId = () => {
    sessionCounter += 1;
    try {
      return `session-${crypto.randomUUID()}`;
    } catch {
      return `session-${Date.now()}-${sessionCounter}`;
    }
  };

  const cancelAutosave = () => {
    if (autosaveTimer !== undefined) {
      window.clearTimeout(autosaveTimer);
      autosaveTimer = undefined;
    }
    if (autosaveMaxWaitTimer !== undefined) {
      window.clearTimeout(autosaveMaxWaitTimer);
      autosaveMaxWaitTimer = undefined;
    }
    firstPendingChangeAt = undefined;
  };

  const clearAutosaveTimers = () => {
    if (autosaveTimer !== undefined) {
      window.clearTimeout(autosaveTimer);
      autosaveTimer = undefined;
    }
    if (autosaveMaxWaitTimer !== undefined) {
      window.clearTimeout(autosaveMaxWaitTimer);
      autosaveMaxWaitTimer = undefined;
    }
  };

  const updateWindowTitle = async () => {
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      // Home has no active document — reset to the bare app name so a stale
      // document title doesn't linger after returning home.
      if (activeMode() === "home") {
        await getCurrentWindow().setTitle("Redoc");
        return;
      }
      const path = currentFilePath();
      const base = path ? path.split(/[\\/]/).pop()! : docTitle();
      const prefix = saveState() === "Dirty" ? "• " : "";
      await getCurrentWindow().setTitle(`${prefix}${base} - Redoc`);
    } catch {
      // Not running inside Tauri.
    }
  };

  let titleRafId: number | undefined;
  createEffect(() => {
    docTitle();
    currentFilePath();
    saveState();
    activeMode();
    if (titleRafId !== undefined) cancelAnimationFrame(titleRafId);
    titleRafId = requestAnimationFrame(() => {
      titleRafId = undefined;
      void updateWindowTitle();
    });
  });

  createEffect(() => {
    const t = settings().theme === "system" ? systemTheme() : settings().theme;
    document.documentElement.setAttribute("data-theme", t);
  });

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

  const confirmDiscardIfDirty = async (): Promise<boolean> => {
    if (activeMode() !== "home" && (saveState() === "Dirty" || saveState() === "Error")) {
      return confirmDialog({
        title: "Unsaved changes",
        message: "You have unsaved changes. Do you want to discard them?",
        confirmLabel: "Discard changes",
      });
    }
    return true;
  };

  const hasDirtySession = () => openSessions().some((session) => session.saveState === "Dirty" || session.saveState === "Error");

  const snapshotCurrentSession = (): DocumentSession<any> | undefined => {
    const mode = activeMode();
    const id = activeSessionId();
    if (mode === "home" || !id) return undefined;
    return {
      id,
      mode,
      content: docContent(),
      title: docTitle(),
      filePath: currentFilePath(),
      docId: currentDocId(),
      saveState: saveState(),
      readOnly: docReadOnly(),
      readOnlyReason: docReadOnlyWarning(),
    };
  };

  const syncCurrentSession = () => {
    const snapshot = snapshotCurrentSession();
    if (!snapshot) return;
    setOpenSessions((previous) => upsertDocumentSession(previous, snapshot));
    setModeBuffers((previous) => ({ ...previous, [snapshot.mode]: snapshot }));
  };

  const registerSession = (mode: EditorMode, buffer: ModeBuffer, id = newSessionId()) => {
    const session: DocumentSession<any> = { id, mode, ...buffer };
    setOpenSessions((previous) => upsertDocumentSession(previous, session));
    setActiveSessionId(id);
    setModeBuffers((previous) => ({ ...previous, [mode]: buffer }));
    return id;
  };

  const loadSession = (session: DocumentSession<any>) => {
    setActiveSessionId(session.id);
    setActiveMode(session.mode);
    setDocContent(session.content);
    setDocTitle(session.title);
    setCurrentFilePath(session.filePath);
    setCurrentDocId(session.docId);
    setSaveState(session.saveState);
    setDocReadOnly(session.readOnly === true);
    setDocReadOnlyWarning(session.readOnlyReason ?? null);
    setModeBuffers((previous) => ({ ...previous, [session.mode]: session }));
  };

  const activateSession = async (id: string) => {
    if (id === activeSessionId()) return;
    rememberCurrentSession();
    syncCurrentSession();
    const target = openSessions().find((session) => session.id === id);
    if (target) loadSession(target);
  };

  const closeSession = async (id: string) => {
    const target = openSessions().find((session) => session.id === id);
    if (!target) return;
    let discardedDirty = false;
    if (id === activeSessionId()) {
      const dirty = activeMode() !== "home" && (saveState() === "Dirty" || saveState() === "Error");
      if (dirty) {
        const confirmed = await confirmDiscardIfDirty();
        if (!confirmed) return;
      }
      discardedDirty = dirty;
    }
    if (id !== activeSessionId() && (target.saveState === "Dirty" || target.saveState === "Error")) {
      const ok = await confirmDialog({
        title: "Unsaved changes",
        message: `"${target.title || "Untitled document"}" has unsaved changes. Close it anyway?`,
        confirmLabel: "Close anyway",
      });
      if (!ok) return;
      discardedDirty = true;
    }
    syncCurrentSession();
    const next = chooseAdjacentSession(openSessions(), id);
    if (lastSessionByMode[target.mode] === id) lastSessionByMode[target.mode] = null;
    setOpenSessions((previous) => removeDocumentSession(previous, id));
    // The user explicitly discarded this session's unsaved work; drop its
    // autosave snapshot so it never resurfaces in the recovery list.
    if (discardedDirty && target.docId) {
      try {
        await commands.discardDocSnapshot(target.docId);
      } catch {
        // Snapshot cleanup is best-effort; recovery list already tolerates misses.
      }
    }
    if (id !== activeSessionId()) return;
    if (next && next.id !== id) {
      loadSession(next);
    } else {
      setActiveSessionId(null);
      setActiveMode("home");
      setDocContent(null);
    }
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
    const id = activeSessionId();
    if (id) {
      setOpenSessions((prev) =>
        upsertDocumentSession(prev, {
          id,
          mode,
          ...snapshot,
          readOnly: docReadOnly(),
          readOnlyReason: docReadOnlyWarning(),
        }),
      );
    }
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
      setDocReadOnly(false);
      setDocReadOnlyWarning(null);
      registerSession(mode, buffer);
    } catch {
      const buffer = emptyModeBuffer(mode);
      updateModeBuffer(mode, buffer);
      setDocTitle(buffer.title);
      setCurrentFilePath(buffer.filePath);
      setCurrentDocId(buffer.docId);
      setDocContent(buffer.content);
      setSaveState(buffer.saveState);
      registerSession(mode, buffer);
    }
  };

  const handleSwitchMode = async (mode: EditorMode) => {
    const current = activeMode();
    if (current === mode) return;
    if (current !== "home") {
      rememberCurrentSession();
      saveCurrentModeToBuffer();
    }
    const lastId = lastSessionByMode[mode];
    const sessions = openSessions();
    const target =
      (lastId ? sessions.find((session) => session.id === lastId && session.mode === mode) : undefined) ||
      sessions.find((session) => session.mode === mode);
    if (target) {
      loadSession(target);
      return;
    }
    const buf = modeBuffers()[mode];
    setActiveMode(mode);
    if (buf.content === null && buf.docId === null) {
      await seedModeBuffer(mode);
    } else {
      const id = registerSession(mode, buf);
      loadSession({ id, mode, ...buf });
    }
  };

  const goHome = async () => {
    if (activeMode() !== "home") {
      rememberCurrentSession();
      saveCurrentModeToBuffer();
    }
    setActiveMode("home");
  };

  const openRedocAtPath = async (path: string) => {
    cancelAutosave();
    try {
      const opened = await commands.openDocument(path);
      setImportWarnings(opened.warnings ?? []);
      const mode = opened.meta.mode;
      const readOnly = opened.meta.readOnly === true;
      const readOnlyWarning = readOnly ? opened.meta.warning ?? null : null;
      if (mode === "doc" || mode === "sheet" || mode === "slide") {
        const buffer: ModeBuffer = {
          content: opened.body,
          title: opened.meta.title,
          filePath: path,
          docId: opened.meta.id,
          saveState: "Saved",
        };
        const id = registerSession(mode, buffer);
        loadSession({ id, mode, ...buffer, readOnly, readOnlyReason: readOnlyWarning });
      }
      else {
        setDocTitle(opened.meta.title);
        setCurrentFilePath(path);
        setCurrentDocId(opened.meta.id);
        setDocContent(opened.body);
        setSaveState("Saved");
        setDocReadOnly(readOnly);
        setDocReadOnlyWarning(readOnlyWarning);
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
    clearAutosaveTimers();
    const id = currentDocId();
    if (!id) return;
    const mode = activeMode();
    const title = docTitle();
    if (mode === "home") return;
    if (firstPendingChangeAt === undefined) {
      firstPendingChangeAt = Date.now();
    }
    const firstChange = firstPendingChangeAt;
    const flush = () => {
      clearAutosaveTimers();
      firstPendingChangeAt = undefined;
      void commands.autosaveDocument(id, mode, title, content);
    };
    const debounceMs = Math.max(500, settings().autosaveIntervalMs);
    autosaveTimer = window.setTimeout(flush, debounceMs);
    const remainingMaxWait = AUTOSAVE_MAX_WAIT_MS - (Date.now() - firstChange);
    if (remainingMaxWait <= 0) {
      flush();
      return;
    }
    autosaveMaxWaitTimer = window.setTimeout(() => {
      if (autosaveTimer !== undefined) {
        window.clearTimeout(autosaveTimer);
      }
      flush();
    }, remainingMaxWait);
  };

  const applyEditorContent = (sessionId: string, content: any) => {
    let background: DocumentSession<any> | undefined;
    setOpenSessions((previous) => {
      const target = previous.find((session) => session.id === sessionId);
      if (!target) return previous;
      background = { ...target, content, saveState: "Dirty" };
      return upsertDocumentSession(previous, background);
    });
    if (activeSessionId() === sessionId) {
      setDocContent(content);
      setSaveState("Dirty");
      scheduleAutosave(content);
    } else if (background?.docId) {
      void commands.autosaveDocument(background.docId, background.mode, background.title, content);
    }
  };

  const openRecoveredDocument = async (snapshotPath: string) => {
    try {
      const opened = await commands.openRecoveredDocument(snapshotPath);
      const mode = opened.meta.mode;
      const readOnly = opened.meta.readOnly === true;
      const readOnlyWarning = readOnly ? opened.meta.warning ?? null : null;
      setImportWarnings(opened.warnings ?? []);
      if (mode === "doc" || mode === "sheet" || mode === "slide") {
        const buffer: ModeBuffer = {
          content: opened.body,
          title: opened.meta.title,
          filePath: null,
          docId: opened.meta.id,
          saveState: "Dirty",
        };
        const id = registerSession(mode, buffer);
        loadSession({ id, mode, ...buffer, readOnly, readOnlyReason: readOnlyWarning });
      } else {
        setActiveMode(mode as any);
        setDocTitle(opened.meta.title);
        setCurrentFilePath(null);
        setCurrentDocId(opened.meta.id);
        setDocContent(opened.body);
        setSaveState("Dirty");
        setDocReadOnly(readOnly);
        setDocReadOnlyWarning(readOnlyWarning);
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
    // Startup budget record (prompt.md §10: cold start < 1.5 s).
    const evalMs = (window as unknown as Record<string, number>).__redocEvalMs;
    if (typeof evalMs === "number") {
      console.info(`[redoc-perf] script-eval→mounted: ${Math.round(performance.now() - evalMs)} ms`);
    }

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
        if (hasDirtySession() || (activeMode() !== "home" && saveState() === "Dirty")) {
          const discard = await confirmDialog({
            title: "Unsaved changes",
            message: "You have unsaved changes. Quit without saving?",
            confirmLabel: "Quit without saving",
          });
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
        const path = (file as File & { path?: string }).path || file.name;
        const fileType = supportedFileType(path);
        if (fileType) {
          void openFilePath(path);
          break;
        } else {
          showToast("Unsupported file format. Redoc supports .redoc, .csv, .docx, .xlsx, and .pptx", "error");
        }
      }
    };
    window.addEventListener("dragover", handleWindowDragOver);
    window.addEventListener("drop", handleWindowDrop);

    let unlistenDragDrop: (() => void) | undefined;
    try {
      import("@tauri-apps/api/webviewWindow").then((mod) => {
        mod.getCurrentWebviewWindow().onDragDropEvent((event) => {
          if (event.payload.type === "drop") {
            const paths = event.payload.paths || [];
            for (const path of paths) {
              const fileType = supportedFileType(path);
              if (fileType) {
                void openFilePath(path);
                break;
              } else {
                showToast("Unsupported file format. Redoc supports .redoc, .csv, .docx, .xlsx, and .pptx", "error");
              }
            }
          }
        }).then((fn) => { unlistenDragDrop = fn; })
          .catch(() => undefined);
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
      // F1 cheatsheet, F4 repeat is registered via shortcutRegistry; F6 pane cycle is global.
      if (e.key === "F1") {
        e.preventDefault();
        setHelpOpen(true);
        return;
      }
      if (handleF6(e)) return;
      const mode = activeMode();
      if (mode === "home") {
        shortcutRegistry.handleKeyDown(e, "home" as any);
      } else {
        shortcutRegistry.handleKeyDown(e, mode);
      }
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
    cancelAutosave();
    if (activeMode() !== "home") {
      rememberCurrentSession();
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
    setImportWarnings([]);
    setDocTitle(title);
    setCurrentFilePath(null);
    setCurrentDocId(null);
    setSaveState("Saved");
    setDocReadOnly(false);
    setDocReadOnlyWarning(null);

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
    const buffer: ModeBuffer = {
      content,
      title,
      filePath: null,
      docId,
      saveState: "Saved",
    };
    const id = registerSession(mode, buffer);
    loadSession({ id, mode, ...buffer });
    showToast(`Created ${title}`, "success");
  };

  const requestCsvImport = (path: string) => {
    setCsvImportPath(path);
    setCsvImportOpen(true);
    return new Promise<any | undefined>((resolve) => {
      csvImportResolver = resolve;
    });
  };

  const openImportedDocument = async (
    mode: EditorMode,
    title: string,
    content: any,
    warnings: string[] = [],
  ) => {
    let docId: string | null = null;
    try {
      const created = await commands.createNewDocument(mode, title);
      docId = created.meta.id;
    } catch {
      // Recovery id is best-effort; the import still opens as an unsaved tab.
    }
    const buffer: ModeBuffer = { content, title, filePath: null, docId, saveState: "Dirty" };
    const id = registerSession(mode, buffer);
    loadSession({ id, mode, ...buffer });
    if (warnings.length > 0) setImportWarnings(warnings);
  };

  const openFilePath = async (path: string) => {
    if (activeMode() !== "home") {
      rememberCurrentSession();
      saveCurrentModeToBuffer();
    }
    cancelAutosave();
    try {
      const fileType = supportedFileType(path);
      if (fileType === "pptx") {
        const imported = await commands.importPptxFile(path);
        const title = path.split(/[\\/]/).pop()?.replace(/\.pptx$/i, "") || "Imported presentation";
        await openImportedDocument("slide", title, imported.deck, imported.warnings);
        showToast("Imported PPTX file; save as .redoc to continue editing", "success");
        return;
      }
      if (fileType === "csv" || fileType === "xlsx") {
        if (fileType === "csv") {
          const workbook = await requestCsvImport(path);
          if (!workbook) return;
          const title = path.split(/[\\/]/).pop()?.replace(/\.csv$/i, "") || "Imported spreadsheet";
          await openImportedDocument("sheet", title, workbook);
          showToast("Imported CSV file", "success");
          return;
        }
        const imported = await commands.importXlsxFile(path);
        const title = path.split(/[\\/]/).pop()?.replace(/\.xlsx$/i, "") || "Imported spreadsheet";
        await openImportedDocument("sheet", title, imported.workbook, imported.warnings);
        showToast("Imported XLSX file", "success");
        return;
      }
      if (fileType === "docx") {
        const imported = await commands.importDocxFile(path);
        const title = path.split(/[\\/]/).pop()?.replace(/\.docx$/i, "") || "Imported document";
        await openImportedDocument("doc", title, imported.document, imported.warnings);
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
    if (docReadOnly()) {
      const saveAsCopy = await confirmDialog({
        title: "Document is read-only",
        message:
          (docReadOnlyWarning() ?? "This document was opened from a newer Redoc format and is read-only.") +
          " Saving would rewrite it in the current format and may lose content. Save a copy under a new name instead?",
        confirmLabel: "Save As a copy",
        danger: false,
      });
      if (saveAsCopy) await handleSaveAs();
      return;
    }
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
      const saved = await commands.saveDocument(path, activeMode() as EditorMode, docTitle(), docContent(), currentDocId());
      setCurrentDocId(saved.id);
      setCurrentFilePath(path);
      setRecents(await commands.getRecents());
      setSaveState("Saved");
      syncCurrentSession();
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
      const saved = await commands.saveDocument(path, activeMode() as EditorMode, docTitle(), docContent(), currentDocId());
      setCurrentDocId(saved.id);
      setCurrentFilePath(path);
      setDocReadOnly(false);
      setDocReadOnlyWarning(null);
      setRecents(await commands.getRecents());
      setSaveState("Saved");
      syncCurrentSession();
      showToast("Document saved", "success");
    } catch {
      setSaveState("Error");
      showToast("Failed to save document", "error");
    }
  };

  const [exportWarnings, setExportWarnings] = createSignal<string[]>([]);
  const [pendingExport, setPendingExport] = createSignal<{ format: string; path: string } | null>(null);
  const [exportProgress, setExportProgress] = createSignal<{ stage: string; percent: number } | null>(null);

  const runExport = async (format: string, path: string) => {
    setExportProgress(null);
    let unlistenProgress: (() => void) | undefined;
    try {
      const mod = await import("@tauri-apps/api/event");
      unlistenProgress = await mod.listen<{ stage: string; percent: number }>("export-progress", (event) => {
        setExportProgress(event.payload);
      });
    } catch {
      // Event API unavailable outside Tauri shell.
    }
    try {
      await commands.exportDocumentToFile(path, activeMode(), format, docContent() || {}, docTitle());
      const compatibilityWarnings = await commands.inspectExportCompatibility(activeMode(), format, docContent() || {});
      if (compatibilityWarnings.length > 0) {
        setImportWarnings(compatibilityWarnings);
      }
      showToast(
        compatibilityWarnings.length > 0
          ? `Exported to ${format.toUpperCase()} with ${compatibilityWarnings.length} compatibility note${compatibilityWarnings.length === 1 ? "" : "s"}`
          : `Exported to ${format.toUpperCase()}`,
        "success",
      );
    } catch (err) {
      showToast(`Export failed: ${err}`, "error");
    } finally {
      unlistenProgress?.();
      setExportProgress(null);
    }
  };

  const handleExport = async (format: string) => {
    try {
      const path = await save({
        defaultPath: `${docTitle().replace(/[^a-z0-9_-]+/gi, "_")}.${format}`,
        filters: [{ name: format.toUpperCase(), extensions: [format] }],
      });
      if (!path) return;
      const compatibilityWarnings = await commands.inspectExportCompatibility(
        activeMode(),
        format,
        docContent() || {},
      );
      if (compatibilityWarnings.length > 0) {
        setExportWarnings(compatibilityWarnings);
        setPendingExport({ format, path });
      } else {
        await runExport(format, path);
      }
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
    if (!csvImportPath()) return;
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
      } as import("solid-js").JSX.CSSProperties}
    >
      {/* Slim shell titlebar — always mounted so the mode switcher also works
          as a launcher from the Home surface. */}
      <header class="app-titlebar g-no-print">
        <div class="app-titlebar-row">
          <div class="app-titlebar-left">
            <button
              type="button"
              class="app-home-btn"
              data-testid="mode-home"
              title={t("shell.titlebar.home")}
              aria-label={t("shell.titlebar.home")}
              onClick={() => void goHome()}
            >
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path d="M3 10.5 12 3l9 7.5" />
                <path d="M5 9.5V21h14V9.5" />
              </svg>
            </button>
            <Show when={activeMode() !== "home"}>
              <input
                class="g-title-input"
                type="text"
                value={docTitle()}
                onInput={(e) => {
                  setDocTitle(e.currentTarget.value);
                  setSaveState("Dirty");
                  syncCurrentSession();
                }}
                aria-label={t("shell.titlebar.documentTitle")}
              />
              <span
                role="status"
                class="app-savestate"
                aria-live="polite"
                aria-atomic="true"
              >
                {saveState() === "Saved"
                  ? t("shell.titlebar.saveStateSaved")
                  : saveState() === "Saving"
                    ? t("shell.titlebar.saveStateSaving")
                    : saveState() === "Error"
                      ? t("shell.titlebar.saveStateError")
                      : t("shell.titlebar.saveStateModified")}
              </span>
            </Show>
          </div>

          <div class="app-titlebar-right">
            <div class="app-mode-pill" role="tablist" aria-label={t("shell.titlebar.modeSwitcher")}>
              <button
                type="button"
                role="tab"
                id="mode-tab-doc"
                data-testid="mode-tab-doc"
                aria-controls="editor-pane-doc"
                class={activeMode() === "doc" ? "active" : ""}
                aria-selected={activeMode() === "doc"}
                onClick={() => {
                  void handleSwitchMode("doc");
                }}
                title={`${t("shell.titlebar.modeWriter")} (Ctrl+Alt+1)`}
              >
                <IconDoc width="14" height="14" color="var(--doc-accent)" /> {t("shell.titlebar.modeWriter")}
              </button>
              <button
                type="button"
                role="tab"
                id="mode-tab-sheet"
                data-testid="mode-tab-sheet"
                aria-controls="editor-pane-sheet"
                class={activeMode() === "sheet" ? "active" : ""}
                aria-selected={activeMode() === "sheet"}
                onClick={() => {
                  void handleSwitchMode("sheet");
                }}
                title={`${t("shell.titlebar.modeCalc")} (Ctrl+Alt+2)`}
              >
                <IconSheet width="14" height="14" color="var(--sheet-accent)" /> {t("shell.titlebar.modeCalc")}
              </button>
              <button
                type="button"
                role="tab"
                id="mode-tab-slide"
                data-testid="mode-tab-slide"
                aria-controls="editor-pane-slide"
                class={activeMode() === "slide" ? "active" : ""}
                aria-selected={activeMode() === "slide"}
                onClick={() => {
                  void handleSwitchMode("slide");
                }}
                title={`${t("shell.titlebar.modeImpress")} (Ctrl+Alt+3)`}
              >
                <IconSlide width="14" height="14" color="var(--slide-accent)" /> {t("shell.titlebar.modeImpress")}
              </button>
            </div>
            <span
              class="g-offline-badge"
              title={t("shell.titlebar.shareDisabledTooltip")}
              aria-label={t("shell.titlebar.shareDisabledTooltip")}
            >
              <span class="g-offline-dot" aria-hidden="true" />
              {t("shell.titlebar.offline")}
            </span>
            <Button variant="ghost" size="sm" onClick={() => setSettingsOpen(true)} title={t("shell.titlebar.settings")} aria-label={t("shell.titlebar.settings")}>
              <IconSettings />
            </Button>
            <Show when={activeMode() !== "home"}>
              <Button variant="share" onClick={() => void handleSave()} title={t("shell.titlebar.save")}>
                <IconSave /> {t("common.save")}
              </Button>
            </Show>
          </div>
        </div>
        <Show when={activeMode() !== "home"}>
          {/* Phase 3: ONE compact command bar (menus preserve MENU_ORDER) + session tabs */}
          <CommandBar
            menus={menus()}
            onOpenPalette={() => setPaletteOpen(true)}
            inspectorOpen={inspectorOpen()}
            onToggleInspector={() => setInspectorOpen(!inspectorOpen())}
            actions={[
              { id: "cmd-new", label: "New", action: () => void handleNewDoc(activeMode() as EditorMode) },
              { id: "cmd-open", label: "Open", action: () => void handleOpenFile() },
              { id: "cmd-save", label: "Save", shortcut: "Ctrl+S", action: () => void handleSave() },
              { id: "cmd-undo", label: "Undo", shortcut: "Ctrl+Z", action: () => emitEditorCommand("undo") },
              { id: "cmd-redo", label: "Redo", shortcut: "Ctrl+Y", action: () => emitEditorCommand("redo") },
              { id: "cmd-find", label: "Find", shortcut: "Ctrl+F", action: () => emitEditorCommand("find") },
            ]}
            overflowActions={shortcutRegistry
              .getAll()
              .filter((c) => c.menuPath && !["cmd-new", "cmd-open", "cmd-save", "cmd-undo", "cmd-redo", "cmd-find"].includes(c.id))
              .slice(0, 24)
              .map((c) => ({ id: c.id, label: c.title, shortcut: c.shortcut, action: c.action }))}
          />
          <Show when={openSessions().length > 0}>
            <div
              role="tablist"
              aria-label="Open documents"
              class="app-session-tabs"
            >
              <For each={openSessions()}>
                {(session) => (
                  <div
                    role="presentation"
                    class="app-session-tab"
                  >
                    <button
                      type="button"
                      role="tab"
                      class="app-session-tab-btn"
                      aria-selected={activeSessionId() === session.id}
                      title={session.filePath || session.title}
                      onClick={() => void activateSession(session.id)}
                      style={{
                        "--tab-accent":
                          session.mode === "sheet"
                            ? "var(--sheet-accent)"
                            : session.mode === "slide"
                              ? "var(--slide-accent)"
                              : "var(--doc-accent)",
                      }}
                    >
                      <ModeIcon mode={session.mode} size={14} />
                      <span style={{ overflow: "hidden", "text-overflow": "ellipsis" }}>{session.title || "Untitled"}</span>
                      <Show when={session.saveState === "Dirty" || session.saveState === "Error"}>
                        <span aria-label={session.saveState === "Error" ? "Save error" : "Unsaved changes"}>•</span>
                      </Show>
                    </button>
                    <button
                      type="button"
                      class="app-session-close"
                      aria-label={`Close ${session.title || "document"}`}
                      title="Close document"
                      onClick={(event) => {
                        event.stopPropagation();
                        void closeSession(session.id);
                      }}
                    >
                      ×
                    </button>
                  </div>
                )}
              </For>
              <button
                type="button"
                class="app-session-add"
                aria-label="New document"
                title="New document"
                onClick={() => void handleNewDoc(activeMode() as EditorMode)}
              >
                +
              </button>
            </div>
          </Show>
        </Show>
      </header>

      <Show when={activeMode() === "home"}>
        <HomeScreen
          recents={recents()}
          onNewDoc={handleNewDoc}
          onOpenFile={handleOpenFile}
          onOpenRecent={async (entry) => {
            if (activeMode() !== "home") {
              rememberCurrentSession();
              saveCurrentModeToBuffer();
            }
            await openRedocAtPath(entry.path);
          }}
          onTogglePin={async (id) => {
            await commands.togglePinRecent(id);
            setRecents(await commands.getRecents());
          }}
          onDropFile={(file) => {
            const path = (file as any).path || file.name;
            const fileType = supportedFileType(path);
            if (fileType) {
              void openFilePath(path);
            } else {
              showToast("Unsupported file format. Redoc supports .redoc, .csv, .docx, .xlsx, and .pptx", "error");
            }
          }}
        />
      </Show>

      <Show when={activeMode() !== "home"}>
        <a href="#canvas-pane" class="g-skip-link g-no-print">
          {t("a11y.skipToCanvas")}
        </a>
        <Show when={docReadOnly()}>
          <div
            role="status"
            class="g-no-print"
            style={{
              display: "flex",
              "align-items": "center",
              "justify-content": "center",
              gap: "8px",
              padding: "6px 12px",
              "font-size": "12px",
              background: "var(--g-yellow-light)",
              color: "var(--text-primary)",
              "border-bottom": "1px solid var(--border-color)",
              "flex-shrink": "0",
            }}
          >
            Read-only: {docReadOnlyWarning() || "This document uses a newer Redoc format than this version supports."}
            <Button size="sm" variant="secondary" onClick={() => void handleSaveAs()}>
              Save As a copy
            </Button>
          </div>
        </Show>

        <div style={{ flex: 1, display: "flex", "min-height": "0", overflow: "hidden" }}>
        <main id="canvas-pane" data-pane="canvas" aria-label={t("shell.panes.canvas")} style={{ flex: 1, position: "relative", overflow: "hidden", "min-height": "0", "min-width": "0" }}>
          <Suspense
            fallback={
              <div class="g-editor-skeleton" role="status" aria-label={t("common.loading")}>
                <div class="g-editor-skeleton-line" style={{ width: "38%" }} />
                <div class="g-editor-skeleton-line" style={{ width: "72%" }} />
                <div class="g-editor-skeleton-line" style={{ width: "55%" }} />
                <div class="g-editor-skeleton-line" style={{ width: "80%" }} />
                <div class="g-editor-skeleton-line" style={{ width: "46%" }} />
              </div>
            }
          >
            <Show keyed when={docMounted() && activeMode() === "doc" ? activeSessionId() : null}>
              {(sessionId) => (
                <div id="editor-pane-doc" data-testid="editor-pane-doc" role="tabpanel" aria-labelledby="mode-tab-doc" aria-label={t("canvas.docLabel")} style={{ height: "100%", width: "100%" }}>
                  <ErrorBoundary
                    onError={(err) => {
                      showToast(`Document editor error: ${err.message}`, "error", 5000);
                    }}
                  >
                  <DocEditor
                    initialContent={docContent()}
                    compareDocuments={openSessions()
                      .filter((session) => session.mode === "doc" && session.id !== activeSessionId())
                      .map((session) => ({ id: session.id, title: session.title, content: session.content }))}
                    zoomLevel={zoomLevel()}
                    spellcheckEnabled={settings().spellcheckEnabled !== false}
                    authorName={settings().author?.displayName}
                    onRequestNew={() => void handleNewDoc("doc")}
                    onRequestOpen={() => void handleOpenFile()}
                    onRequestSave={() => void handleSave()}
                    onRequestExportPdf={() => void handleExport("pdf")}
                    onChange={(json) => applyEditorContent(sessionId, json)}
                    onWordCountChange={(info) => {
                      setStatusInfo(info);
                      setSelectionSummary(info);
                    }}
                  />
                  </ErrorBoundary>
                </div>
              )}
            </Show>
            <Show keyed when={sheetMounted() && activeMode() === "sheet" ? activeSessionId() : null}>
              {(sessionId) => (
                <div id="editor-pane-sheet" data-testid="editor-pane-sheet" role="tabpanel" aria-labelledby="mode-tab-sheet" aria-label={t("canvas.gridLabel")} style={{ height: "100%", width: "100%" }}>
                  <ErrorBoundary
                    onError={(err) => {
                      showToast(`Spreadsheet editor error: ${err.message}`, "error", 5000);
                    }}
                  >
                  <SheetEditor
                    initialContent={docContent()}
                    zoomLevel={zoomLevel()}
                    spellcheckEnabled={settings().spellcheckEnabled !== false}
                    authorName={settings().author?.displayName}
                    onImportCsv={handleImportCsv}
                    onExportCsv={handleExportCsv}
                    onRequestNew={() => void handleNewDoc("sheet")}
                    onRequestOpen={() => void handleOpenFile()}
                    onRequestSave={() => void handleSave()}
                    onRequestExportPdf={() => void handleExport("pdf")}
                    onChange={(data) => applyEditorContent(sessionId, data)}
                    onCellInfoChange={(info) => {
                      setStatusInfo(info);
                      setSelectionSummary(info);
                    }}
                  />
                  </ErrorBoundary>
                </div>
              )}
            </Show>
            <Show keyed when={slideMounted() && activeMode() === "slide" ? activeSessionId() : null}>
              {(sessionId) => (
                <div id="editor-pane-slide" data-testid="editor-pane-slide" role="tabpanel" aria-labelledby="mode-tab-slide" aria-label={t("canvas.slideLabel")} style={{ height: "100%", width: "100%" }}>
                  <ErrorBoundary
                    onError={(err) => {
                      showToast(`Presentation editor error: ${err.message}`, "error", 5000);
                    }}
                  >
                  <SlideEditor
                    initialContent={docContent()}
                    zoomLevel={zoomLevel()}
                    onZoomChange={setZoomLevel}
                    authorName={settings().author?.displayName}
                    onRequestNew={() => void handleNewDoc("slide")}
                    onRequestOpen={() => void handleOpenFile()}
                    onRequestSave={() => void handleSave()}
                    onRequestExportPdf={() => void handleExport("pdf")}
                    onRequestExportPptx={() => void handleExport("pptx")}
                    onChange={(deck) => applyEditorContent(sessionId, deck)}
                    onSlideInfoChange={(info) => {
                      setStatusInfo(info);
                      setSelectionSummary(info);
                    }}
                  />
                  </ErrorBoundary>
                </div>
              )}
            </Show>
          </Suspense>
        </main>

        <Show when={inspectorOpen()}>
          <Inspector
            title={t("inspector.title")}
            selectionSummary={selectionSummary()}
            onClose={() => setInspectorOpen(false)}
          >
            <InspectorSection title={t("inspector.selection")}>
              <div style={{ "font-size": "12px", color: "var(--text-secondary)" }}>
                {selectionSummary() || t("inspector.empty")}
              </div>
            </InspectorSection>
            <InspectorSection title={t("inspector.document")}>
              <div style={{ display: "flex", "flex-direction": "column", gap: "6px", "font-size": "12px", color: "var(--text-secondary)" }}>
                <div>{docTitle()}</div>
                <div>{currentFilePath() || t("placeholder.untitled")}</div>
                <div>Zoom: {zoomLevel()}% · {saveState() === "Dirty" ? t("statusbar.save.modified") : saveState() === "Saving" ? t("statusbar.save.saving") : t("statusbar.save.saved")}</div>
              </div>
            </InspectorSection>
            <InspectorSection title={t("inspector.properties")}>
              <div style={{ display: "flex", gap: "6px", "flex-wrap": "wrap" }}>
                <Button size="sm" variant="secondary" onClick={() => emitEditorCommand("bold")}>{t("format.bold")} (Ctrl+B)</Button>
                <Button size="sm" variant="secondary" onClick={() => emitEditorCommand("italic")}>{t("format.italic")} (Ctrl+I)</Button>
                <Button size="sm" variant="secondary" onClick={() => emitEditorCommand("underline")}>{t("format.underline")} (Ctrl+U)</Button>
                <Button size="sm" variant="secondary" onClick={() => setPaletteOpen(true)}>Ctrl+K</Button>
              </div>
              <div style={{ "font-size": "11px", color: "var(--text-muted)", "margin-top": "8px" }}>
                {t("palette.hint")}
              </div>
            </InspectorSection>
          </Inspector>
        </Show>
        </div>

        <StatusBar
          mode={activeMode() as any}
          saveState={saveState()}
          wordCountInfo={statusInfo()}
          zoomLevel={zoomLevel()}
          onZoomChange={setZoomLevel}
          pageStyle={t("statusbar.defaultStyle")}
          language={t("statusbar.english")}
        />
      </Show>

      <CommandPalette open={paletteOpen()} onClose={() => setPaletteOpen(false)} activeMode={activeMode()} />
      <Dialog open={helpOpen()} title={t("shell.cheatsheet.title")} onClose={() => setHelpOpen(false)}>
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
              <div class="app-shortcut-row">
                <span>{cmd.title}</span>
                <kbd class="app-kbd">{cmd.shortcut}</kbd>
              </div>
            )}
          </For>
          <div style={{ "margin-top": "12px", "font-size": "12px", color: "var(--text-muted)" }}>
            {t("shell.cheatsheet.hint")} {t("a11y.f6Hint")} · F1 {t("shell.cheatsheet.title")} · F4 Repeat
          </div>
        </div>
      </Dialog>
      <SettingsDialog
        open={settingsOpen()}
        settings={settings()}
        onClose={() => setSettingsOpen(false)}
        onSave={async (next) => {
          const previous = settings();
          setSettings(next);
          try {
            await commands.updateSettings(next);
            void maybeCheckForUpdates(next.checkForUpdates !== false);
          } catch (error) {
            setSettings(previous);
            showToast(`Could not save settings: ${error}`, "error");
          }
        }}
      />

      <Dialog open={exportOpen()} title="Download" onClose={() => setExportOpen(false)}>
        <div style={{ display: "flex", "flex-direction": "column", gap: "12px" }}>
          <p class="app-dialog-text">
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
          <Show when={exportProgress() !== null}>
            <div
              role="status"
              aria-live="polite"
              style={{ display: "flex", "flex-direction": "column", gap: "4px" }}
            >
              <span style={{ "font-size": "12px", color: "var(--text-secondary)" }}>
                Exporting… {exportProgress()?.stage} ({exportProgress()?.percent}%)
              </span>
              <div
                style={{
                  height: "6px",
                  "border-radius": "3px",
                  background: "var(--bg-tertiary)",
                  overflow: "hidden",
                }}
              >
                <div
                  style={{
                    width: `${exportProgress()?.percent ?? 0}%`,
                    height: "100%",
                    background: "var(--accent-color)",
                    transition: "width 120ms ease-out",
                  }}
                />
              </div>
            </div>
          </Show>
        </div>
      </Dialog>

      <Dialog open={importWarnings().length > 0} title="Compatibility report" onClose={() => setImportWarnings([])}>        <div style={{ display: "flex", "flex-direction": "column", gap: "12px" }}>
          <p class="app-dialog-text">
            Some Office constructs were simplified or may not round-trip natively:
          </p>
          <ul class="app-dialog-list">
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

      <Dialog
        open={pendingExport() !== null}
        title="Export compatibility check"
        onClose={() => {
          setPendingExport(null);
          setExportWarnings([]);
        }}
      >
        <div style={{ display: "flex", "flex-direction": "column", gap: "12px" }}>
          <p class="app-dialog-text">
            The following content will be simplified or may not round-trip natively in{" "}
            {pendingExport()?.format?.toUpperCase()}:
          </p>
          <ul class="app-dialog-list" style={{ "max-height": "40vh", overflow: "auto" }}>
            <For each={exportWarnings()}>{(warning) => <li>{warning}</li>}</For>
          </ul>
          <div style={{ display: "flex", "justify-content": "flex-end", gap: "8px" }}>
            <Button
              variant="secondary"
              onClick={() => {
                setPendingExport(null);
                setExportWarnings([]);
              }}
            >
              Cancel
            </Button>
            <Button
              variant="share"
              onClick={() => {
                const pending = pendingExport();
                setPendingExport(null);
                setExportWarnings([]);
                if (pending) void runExport(pending.format, pending.path);
              }}
            >
              Export anyway
            </Button>
          </div>
        </div>
      </Dialog>

      <Dialog open={recoveryOpen()} title="Recover unsaved work" onClose={() => setRecoveryOpen(false)}>
        <div style={{ display: "flex", "flex-direction": "column", gap: "12px" }}>
          <p class="app-dialog-text">
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

      <ConfirmDialog
        open={confirmState().open}
        title={confirmState().title}
        message={confirmState().message}
        confirmLabel={confirmState().confirmLabel}
        danger={confirmState().danger}
        onConfirm={() => settleConfirm(true)}
        onCancel={() => settleConfirm(false)}
      />

      <ToastContainer />
    </div>
  );
}
