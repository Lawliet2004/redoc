import { describe, expect, it } from "vitest";
import { normalizeDocLink } from "./links";

describe("document links", () => {
  it("allows safe web and contact links", () => {
    expect(normalizeDocLink(" https://example.test/docs ")).toBe("https://example.test/docs");
    expect(normalizeDocLink("mailto:team@example.test")).toBe("mailto:team@example.test");
    expect(normalizeDocLink("tel:+15551212")).toBe("tel:+15551212");
    expect(normalizeDocLink("https://")).toBeNull();
  });

  it("canonicalizes internal bookmark targets and rejects unsafe schemes", () => {
    expect(normalizeDocLink("#Project plan")).toBe("internal:Project_plan");
    expect(normalizeDocLink("internal:Project_plan")).toBe("internal:Project_plan");
    expect(normalizeDocLink("javascript:alert(1)")).toBeNull();
    expect(normalizeDocLink("data:text/html,unsafe")).toBeNull();
  });
});
