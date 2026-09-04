// Presenter sync uses Tauri events: the editor broadcasts full deck snapshots
// on `presenter-deck-changed` so the presenter window live-updates during
// edits. The localStorage stash below is only the initial hand-off when the
// presenter window is first opened (it needs the deck before any event
// listener runs).

export const PRESENTER_SESSION_KEY = "redoc-presenter-deck";

export type PresenterSessionPayload = {
  deck: unknown;
  startIndex: number;
};

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

export function stashPresenterDeck(deck: unknown, startIndex: number): void {
  void broadcastPresenterDeck(deck, startIndex);
  // Initial hand-off for a presenter window opened right now.
  try {
    const payload: PresenterSessionPayload = { deck, startIndex };
    localStorage.setItem(PRESENTER_SESSION_KEY, JSON.stringify(payload));
  } catch {
    // localStorage may be unavailable; ignore.
  }
}

export function loadPresenterSession(): PresenterSessionPayload | null {
  try {
    const raw = localStorage.getItem(PRESENTER_SESSION_KEY);
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
