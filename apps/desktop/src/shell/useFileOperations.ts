import { createSignal, Accessor, Setter } from "solid-js";
import { commands } from "@redoc/api-client";
import { showToast } from "@redoc/ui";
import { open, save } from "@tauri-apps/plugin-dialog";
import type { EditorMode } from "./registerCommands";

export interface FileOperationsState {
  exportWarnings: Accessor<string[]>;
  exportProgress: Accessor<{ stage: string; percent: number } | null>;
  isExporting: Accessor<boolean>;
}

export interface FileOperations {
  state: FileOperationsState;
  openFile: () => Promise<void>;
  openFilePath: (path: string) => Promise<void>;
  saveFile: (saveAsCopy?: boolean) => Promise<void>;
  saveFileAs: () => Promise<void>;
  exportFile: (format: string) => Promise<void>;
  importCsv: () => Promise<string | undefined>;
  exportCsv: () => Promise<void>;
  runExport: (format: string, path: string) => Promise<void>;
}

export function createFileOperations(
  getActiveMode: () => EditorMode | "home",
  getDocTitle: () => string,
  getDocContent: () => unknown,
  getDocId: () => string | null,
  getFilePath: () => string | null,
  isReadOnly: () => boolean,
  getReadOnlyWarning: () => string | null,
  setActiveMode: Setter<EditorMode | "home">,
  setDocTitle: Setter<string>,
  setDocContent: Setter<unknown>,
  setDocId: Setter<string | null>,
  setFilePath: Setter<string | null>,
  setSaveState: Setter<"Saved" | "Saving" | "Dirty" | "Error">,
  setReadOnly: Setter<boolean>,
  setReadOnlyWarning: Setter<string | null>,
  setRecents: Setter<unknown[]>,
  syncCurrentSession: () => void,
  cancelAutosave: () => void,
  confirmDialog: (options: {
    title: string;
    message: string;
    confirmLabel: string;
    danger?: boolean;
  }) => Promise<boolean>,
  registerSession: (mode: EditorMode, buffer: {
    content: unknown;
    title: string;
    filePath: string | null;
    docId: string | null;
    saveState: "Saved" | "Saving" | "Dirty" | "Error";
  }) => string,
): FileOperations {
  const [exportWarnings, setExportWarnings] = createSignal<string[]>([]);
  const [exportProgress, setExportProgress] = createSignal<{ stage: string; percent: number } | null>(null);
  const [isExporting, setIsExporting] = createSignal(false);
  const [pendingExport, setPendingExport] = createSignal<{ format: string; path: string } | null>(null);

  const getFileFilters = () => [
    { name: "Redoc and office files", extensions: ["redoc", "csv", "xlsx", "docx", "pptx"] },
  ];

  const sanitizeFileName = (name: string) =>
    name.replace(/[^a-z0-9_-]+/gi, "_");

  const openFile = async () => {
    const path = await open({
      multiple: false,
      directory: false,
      filters: getFileFilters(),
    });
    if (!path || Array.isArray(path)) return;
    await openFilePath(path);
  };

  const openFilePath = async (path: string) => {
    try {
      const opened = await commands.openDocument(path);
      if (!opened) {
        showToast("Could not open file", "error");
        return;
      }

      const meta = opened.meta;
      const mode = meta.mode as EditorMode;
      setActiveMode(mode);
      setDocTitle(meta.title || "Untitled");
      setDocContent(opened.body);
      setDocId(meta.id);
      setFilePath(path);
      setSaveState("Saved");
      setReadOnly(meta.readOnly === true);
      setReadOnlyWarning(meta.warning ?? null);

      registerSession(mode, {
        content: opened.body,
        title: meta.title || "Untitled",
        filePath: path,
        docId: meta.id,
        saveState: "Saved",
      });

      setRecents(await commands.getRecents());
    } catch (error) {
      showToast(`Could not open file: ${error}`, "error");
    }
  };

  const saveFile = async (saveAsCopy = false) => {
    if (isReadOnly() && !saveAsCopy) {
      const shouldSaveAsCopy = await confirmDialog({
        title: "Document is read-only",
        message:
          (getReadOnlyWarning() ?? "This document was opened from a newer Redoc format and is read-only.") +
          " Saving would rewrite it in the current format and may lose content. Save a copy under a new name instead?",
        confirmLabel: "Save As a copy",
        danger: false,
      });
      if (shouldSaveAsCopy) {
        await saveFileAs();
      }
      return;
    }

    cancelAutosave();
    const path =
      getFilePath() ||
      (await save({
        defaultPath: `${sanitizeFileName(getDocTitle())}.redoc`,
        filters: [{ name: "Redoc document", extensions: ["redoc"] }],
      }));

    if (!path) return;

    setSaveState("Saving");
    try {
      const saved = await commands.saveDocument(
        path,
        getActiveMode() as EditorMode,
        getDocTitle(),
        getDocContent(),
        getDocId(),
      );
      setDocId(saved.id);
      setFilePath(path);
      setRecents(await commands.getRecents());
      setSaveState("Saved");
      syncCurrentSession();
      showToast("Document saved", "success");
    } catch {
      setSaveState("Error");
      showToast("Failed to save document", "error");
    }
  };

  const saveFileAs = async () => {
    cancelAutosave();
    const path = await save({
      defaultPath: `${sanitizeFileName(getDocTitle())}.redoc`,
      filters: [{ name: "Redoc document", extensions: ["redoc"] }],
    });
    if (!path) return;

    setSaveState("Saving");
    try {
      const saved = await commands.saveDocument(
        path,
        getActiveMode() as EditorMode,
        getDocTitle(),
        getDocContent(),
        getDocId(),
      );
      setDocId(saved.id);
      setFilePath(path);
      setReadOnly(false);
      setReadOnlyWarning(null);
      setRecents(await commands.getRecents());
      setSaveState("Saved");
      syncCurrentSession();
      showToast("Document saved", "success");
    } catch {
      setSaveState("Error");
      showToast("Failed to save document", "error");
    }
  };

  const runExport = async (format: string, path: string) => {
    setExportProgress(null);
    setIsExporting(true);
    let unlistenProgress: (() => void) | undefined;

    try {
      const mod = await import("@tauri-apps/api/event");
      unlistenProgress = await mod.listen<{ stage: string; percent: number }>(
        "export-progress",
        (event) => setExportProgress(event.payload),
      );
    } catch {
      // Event API unavailable outside Tauri shell.
    }

    try {
      await commands.exportDocumentToFile(
        path,
        getActiveMode(),
        format,
        getDocContent() || {},
        getDocTitle(),
      );
      const compatibilityWarnings = await commands.inspectExportCompatibility(
        getActiveMode(),
        format,
        getDocContent() || {},
      );
      if (compatibilityWarnings.length > 0) {
        setExportWarnings(compatibilityWarnings);
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
      setIsExporting(false);
    }
  };

  const exportFile = async (format: string) => {
    try {
      const path = await save({
        defaultPath: `${sanitizeFileName(getDocTitle())}.${format}`,
        filters: [{ name: format.toUpperCase(), extensions: [format] }],
      });
      if (!path) return;

      const compatibilityWarnings = await commands.inspectExportCompatibility(
        getActiveMode(),
        format,
        getDocContent() || {},
      );

      if (compatibilityWarnings.length > 0) {
        setExportWarnings(compatibilityWarnings);
        setPendingExport({ format, path });
      } else {
        await runExport(format, path);
      }
    } catch (err) {
      showToast(`Export failed: ${err}`, "error");
    }
  };

  const importCsv = async () => {
    const path = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "CSV", extensions: ["csv"] }],
    });
    if (!path || Array.isArray(path)) return undefined;
    return path;
  };

  const exportCsv = async () => {
    const content = getDocContent() as { sheets?: unknown[]; activeSheetIndex?: number } | null;
    const sheet = content?.sheets?.[content?.activeSheetIndex || 0];
    if (!sheet) return;

    const path = await save({
      defaultPath: `${sanitizeFileName(getDocTitle())}.csv`,
      filters: [{ name: "CSV", extensions: ["csv"] }],
    });
    if (!path) return;

    try {
      await commands.exportCsvToFile(path, sheet);
      showToast("CSV exported", "success");
    } catch (err) {
      showToast(`CSV export failed: ${err}`, "error");
    }
  };

  return {
    state: {
      exportWarnings,
      exportProgress,
      isExporting,
    },
    openFile,
    openFilePath,
    saveFile,
    saveFileAs,
    exportFile,
    importCsv,
    exportCsv,
    runExport,
  };
}
