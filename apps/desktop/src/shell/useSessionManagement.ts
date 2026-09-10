import { createSignal, Accessor, Setter } from "solid-js";
import {
  chooseAdjacentSession,
  removeDocumentSession,
  upsertDocumentSession,
  type DocumentSession,
} from "@redoc/editor-common";
import { commands } from "@redoc/api-client";
import type { EditorMode } from "./registerCommands";

export type ModeBuffer = {
  content: unknown | null;
  title: string;
  filePath: string | null;
  docId: string | null;
  saveState: "Saved" | "Saving" | "Dirty" | "Error";
  readOnly?: boolean;
  readOnlyReason?: string | null;
};

export interface SessionManagement {
  openSessions: Accessor<DocumentSession<unknown>[]>;
  activeSessionId: Accessor<string | null>;
  modeBuffers: Accessor<Record<EditorMode, ModeBuffer>>;
  hasDirtySession: () => boolean;
  registerSession: (mode: EditorMode, buffer: ModeBuffer, id?: string) => string;
  activateSession: (id: string) => void;
  closeSession: (id: string, confirmDiscard: () => Promise<boolean>) => Promise<void>;
  syncCurrentSession: () => void;
  updateModeBuffer: (mode: EditorMode, buffer: Partial<ModeBuffer>) => void;
  getModeBuffer: (mode: EditorMode) => ModeBuffer;
  newSessionId: () => string;
}

export function createSessionManagement(
  getActiveMode: () => EditorMode | "home",
  getActiveSessionData: () => Omit<ModeBuffer, "readOnly" | "readOnlyReason"> & {
    readOnly?: boolean;
    readOnlyReason?: string | null;
  },
  setActiveMode: Setter<EditorMode | "home">,
  setActiveSessionId: Setter<string | null>,
  loadSessionData: (session: DocumentSession<unknown>) => void,
): SessionManagement {
  const [openSessions, setOpenSessions] = createSignal<DocumentSession<unknown>[]>([]);
  const [activeSessionId, setActiveSessionIdInternal] = createSignal<string | null>(null);
  const [modeBuffers, setModeBuffers] = createSignal<Record<EditorMode, ModeBuffer>>({
    doc: emptyModeBuffer("doc"),
    sheet: emptyModeBuffer("sheet"),
    slide: emptyModeBuffer("slide"),
  });

  const newSessionId = () => `session-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;

  const hasDirtySession = () =>
    openSessions().some((session) => session.saveState === "Dirty" || session.saveState === "Error");

  const snapshotCurrentSession = (): DocumentSession<unknown> | undefined => {
    const mode = getActiveMode();
    const id = activeSessionId();
    if (mode === "home" || !id) return undefined;
    const data = getActiveSessionData();
    return {
      id,
      mode,
      ...data,
    };
  };

  const syncCurrentSession = () => {
    const snapshot = snapshotCurrentSession();
    if (!snapshot) return;
    setOpenSessions((previous) => upsertDocumentSession(previous, snapshot));
    setModeBuffers((previous) => ({ ...previous, [snapshot.mode]: snapshot }));
  };

  const registerSession = (mode: EditorMode, buffer: ModeBuffer, id = newSessionId()): string => {
    const session: DocumentSession<unknown> = { id, mode, ...buffer };
    setOpenSessions((previous) => upsertDocumentSession(previous, session));
    setActiveSessionIdInternal(id);
    setModeBuffers((previous) => ({ ...previous, [mode]: buffer }));
    return id;
  };

  const activateSession = (id: string) => {
    if (id === activeSessionId()) return;
    syncCurrentSession();
    const target = openSessions().find((session) => session.id === id);
    if (target) {
      setActiveSessionIdInternal(target.id);
      loadSessionData(target);
    }
  };

  const closeSession = async (id: string, confirmDiscard: () => Promise<boolean>) => {
    const target = openSessions().find((session) => session.id === id);
    if (!target) return;

    let discardedDirty = false;

    if (id === activeSessionId()) {
      const mode = getActiveMode();
      const data = getActiveSessionData();
      const dirty = mode !== "home" && data.saveState === "Dirty";
      const confirmed = await confirmDiscard();
      if (!confirmed) return;
      discardedDirty = dirty;
    }

    if (id !== activeSessionId() && (target.saveState === "Dirty" || target.saveState === "Error")) {
      const ok = await confirmDiscard();
      if (!ok) return;
      discardedDirty = true;
    }

    syncCurrentSession();
    const next = chooseAdjacentSession(openSessions(), id);
    setOpenSessions((previous) => removeDocumentSession(previous, id));

    // Discard autosave snapshot for explicitly discarded sessions
    if (discardedDirty && target.docId) {
      try {
        await commands.discardDocSnapshot(target.docId);
      } catch {
        // Snapshot cleanup is best-effort; recovery list tolerates misses.
      }
    }

    if (id !== activeSessionId()) return;

    if (next && next.id !== id) {
      setActiveSessionIdInternal(next.id);
      loadSessionData(next);
    } else {
      setActiveSessionIdInternal(null);
      setActiveMode("home");
    }
  };

  const updateModeBuffer = (mode: EditorMode, buffer: Partial<ModeBuffer>) => {
    setModeBuffers((prev) => ({
      ...prev,
      [mode]: { ...prev[mode], ...buffer },
    }));
  };

  const getModeBuffer = (mode: EditorMode): ModeBuffer => {
    return modeBuffers()[mode] || emptyModeBuffer(mode);
  };

  return {
    openSessions,
    activeSessionId,
    modeBuffers,
    hasDirtySession,
    registerSession,
    activateSession,
    closeSession,
    syncCurrentSession,
    updateModeBuffer,
    getModeBuffer,
    newSessionId,
  };
}

const DEFAULT_MODE_TITLES: Record<EditorMode, string> = {
  doc: "Untitled document",
  sheet: "Untitled spreadsheet",
  slide: "Untitled presentation",
};

function emptyModeBuffer(mode: EditorMode): ModeBuffer {
  return {
    content: null,
    title: DEFAULT_MODE_TITLES[mode],
    filePath: null,
    docId: null,
    saveState: "Saved",
  };
}
