export type SessionMode = "doc" | "sheet" | "slide";

export type DocumentSession<T = unknown> = {
  id: string;
  mode: SessionMode;
  content: T | null;
  title: string;
  filePath: string | null;
  docId: string | null;
  saveState: "Saved" | "Saving" | "Dirty" | "Error";
  readOnly?: boolean;
  readOnlyReason?: string | null;
};

/** Returns a new list with the session inserted or replaced by id. */
export function upsertDocumentSession<T>(sessions: readonly DocumentSession<T>[], session: DocumentSession<T>): DocumentSession<T>[] {
  const index = sessions.findIndex((candidate) => candidate.id === session.id);
  if (index < 0) return [...sessions, session];
  return sessions.map((candidate, candidateIndex) => (candidateIndex === index ? session : candidate));
}

/** Removes a session without mutating the caller's list. */
export function removeDocumentSession<T>(sessions: readonly DocumentSession<T>[], id: string): DocumentSession<T>[] {
  return sessions.filter((session) => session.id !== id);
}

/** Picks the nearest surviving tab when a session is closed. */
export function chooseAdjacentSession<T>(sessions: readonly DocumentSession<T>[], closingId: string): DocumentSession<T> | undefined {
  const index = sessions.findIndex((session) => session.id === closingId);
  if (index < 0) return sessions[0];
  return sessions[index + 1] ?? sessions[index - 1];
}
