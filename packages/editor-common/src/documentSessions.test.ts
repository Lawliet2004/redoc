import { describe, expect, it } from "vitest";
import { chooseAdjacentSession, removeDocumentSession, upsertDocumentSession, type DocumentSession } from "./documentSessions";

const session = (id: string): DocumentSession => ({
  id,
  mode: "doc",
  content: { id },
  title: id,
  filePath: null,
  docId: id,
  saveState: "Saved",
});

describe("document session helpers", () => {
  it("inserts and replaces by stable session id without mutating", () => {
    const original = [session("one")];
    const inserted = upsertDocumentSession(original, session("two"));
    const replaced = upsertDocumentSession(inserted, { ...session("one"), title: "Updated" });
    expect(original).toHaveLength(1);
    expect(inserted.map((item) => item.id)).toEqual(["one", "two"]);
    expect(replaced[0].title).toBe("Updated");
  });

  it("removes only the requested session", () => {
    const sessions = [session("one"), session("two")];
    expect(removeDocumentSession(sessions, "one").map((item) => item.id)).toEqual(["two"]);
    expect(sessions).toHaveLength(2);
  });

  it("chooses the next tab, then the previous tab at the end", () => {
    const sessions = [session("one"), session("two"), session("three")];
    expect(chooseAdjacentSession(sessions, "two")?.id).toBe("three");
    expect(chooseAdjacentSession(sessions, "three")?.id).toBe("two");
    expect(chooseAdjacentSession(sessions, "missing")?.id).toBe("one");
  });
});
