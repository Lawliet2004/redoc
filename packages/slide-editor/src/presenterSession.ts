export const PRESENTER_SESSION_KEY = "redoc-presenter-deck";

export type PresenterSessionPayload = {
  deck: unknown;
  startIndex: number;
};

export function stashPresenterDeck(deck: unknown, startIndex: number): void {
  const payload: PresenterSessionPayload = { deck, startIndex };
  sessionStorage.setItem(PRESENTER_SESSION_KEY, JSON.stringify(payload));
}

export function loadPresenterSession(): PresenterSessionPayload | null {
  try {
    const raw = sessionStorage.getItem(PRESENTER_SESSION_KEY);
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
