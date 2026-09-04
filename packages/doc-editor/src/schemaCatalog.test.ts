import { describe, it, expect } from "vitest";
import { mySchema, customMarks } from "./schema";
import catalog from "./schema-catalog.json";

/**
 * Drift guard: the ProseMirror schema used by the editor must expose exactly
 * the marks and nodes declared by the authoritative Rust catalog
 * (`crates/doc-engine/src/schema.rs`), which exports
 * `schema-catalog.json` via `cargo test -p redoc-doc-engine`.
 *
 * If this test fails after changing schema.ts, the Rust catalog or the
 * TypeScript spec drifted; update both sides in the same change.
 */

describe("schema-catalog parity", () => {
  it("catalog version is 1", () => {
    expect(catalog.version).toBe(1);
  });

  it("editor schema marks match the catalog marks exactly", () => {
    const catalogMarks = [...catalog.marks].sort();
    const editorMarks = Object.keys(customMarks).sort();
    expect(editorMarks).toEqual(catalogMarks);
  });

  it("editor schema nodes match the catalog nodes exactly", () => {
    const catalogNodes = [...catalog.nodes].sort();
    const editorNodes: string[] = [];
    (mySchema.spec.nodes as unknown as { forEach: (f: (k: string) => void) => void })
      .forEach((name) => editorNodes.push(name));
    expect(editorNodes.sort()).toEqual(catalogNodes);
  });

  it("legacy mark aliases resolve through the catalog", () => {
    const aliases = catalog.markAliases as Record<string, string>;
    for (const [alias, canonical] of Object.entries(aliases)) {
      expect(catalog.marks).toContain(canonical);
      expect(mySchema.spec.marks.get(canonical)).toBeTruthy();
      expect(alias).toBeTruthy();
    }
  });
});
