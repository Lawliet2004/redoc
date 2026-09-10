// Presenter sync uses Tauri events: the editor broadcasts full deck snapshots
// on `presenter-deck-changed` so the presenter window live-updates during
// edits. The localStorage stash below is only the initial hand-off when the
// presenter window is first opened (it needs the deck before any event
// listener runs). Both channels are debounced: bursts of edits produce one
// broadcast/stash per debounce window instead of one per keystroke.

export const PRESENTER_SESSION_KEY = "redoc-presenter-deck";

export type PresenterSessionPayload = {
  deck: unknown;
  startIndex: number;
};

export const PRESENTER_STASH_DEBOUNCE_MS = 300;

export async function broadcastPresenterDeck(
  deck: unknown,
  startIndex: number,
): Promise<void> {
  try {
    const { emit } = await import("@tauri-apps/api/event");
    await emit("presenter-deck-changed", { deck, slideIndex: startIndex });
  } catch {
    // Not running under Tauri (e.g. unit tests); the presenter window reads
    // the localStorage hand-off instead.
  }
}

let stashTimer: ReturnType<typeof setTimeout> | undefined;

/** Write the presenter hand-off synchronously (used right before opening the
 * presenter window so the fresh window can read the deck immediately). */
export function stashPresenterDeckNow(deck: unknown, startIndex: number): void {
  if (stashTimer !== undefined) {
    clearTimeout(stashTimer);
    stashTimer = undefined;
  }
  const payload: PresenterSessionPayload = { deck, startIndex };
  try {
    (window as any).__REDOC_PRESENTER_DECK__ = payload;
  } catch {
    // Ignore memory attachment failure
  }
  void broadcastPresenterDeck(deck, startIndex);
  try {
    localStorage.setItem(PRESENTER_SESSION_KEY, JSON.stringify(payload));
  } catch {
    try {
      sessionStorage.setItem(PRESENTER_SESSION_KEY, JSON.stringify(payload));
    } catch {
      // Storage quota exceeded on large decks (>3.5MB); memory and Tauri event channel handle sync
    }
  }
}

export function stashPresenterDeck(deck: unknown, startIndex: number): void {
  if (stashTimer !== undefined) clearTimeout(stashTimer);
  stashTimer = setTimeout(() => {
    stashTimer = undefined;
    stashPresenterDeckNow(deck, startIndex);
  }, PRESENTER_STASH_DEBOUNCE_MS);
}

export function loadPresenterSession(): PresenterSessionPayload | null {
  try {
    const memory = (window as any).__REDOC_PRESENTER_DECK__;
    if (memory && typeof memory === "object" && memory.deck) {
      return {
        deck: memory.deck,
        startIndex: typeof memory.startIndex === "number" ? memory.startIndex : 0,
      };
    }
    const raw = localStorage.getItem(PRESENTER_SESSION_KEY) || sessionStorage.getItem(PRESENTER_SESSION_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as PresenterSessionPayload;
    if (!parsed || typeof parsed !== "object" || !parsed.deck) return null;
    return {
      deck: parsed.deck,
      startIndex: typeof parsed.startIndex === "number" ? parsed.startIndex : 0,
    };
  } catch {
    return null;
  }
}
