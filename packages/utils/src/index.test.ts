import { describe, expect, it } from "vitest";
import { generateUuid, encodeBase64, decodeBase64, parseTsv, serializeTsv } from "./index";

describe("utils", () => {
  it("generates UUID-shaped document ids", () => {
    expect(generateUuid()).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i);
  });

  it("encodes and decodes base64", () => {
    const original = "Hello World! 😊";
    const encoded = encodeBase64(original);
    const decoded = decodeBase64(encoded);
    expect(decoded).toBe(original);
  });

  it("parses and serializes TSV", () => {
    const data = [
      ["a", "b", "c"],
      ["1", "2", "3"]
    ];
    const tsv = serializeTsv(data);
    expect(tsv).toBe("a\tb\tc\n1\t2\t3");
    
    const parsed = parseTsv(tsv);
    expect(parsed).toEqual(data);
  });

  it("unquotes RFC-4180-style TSV fields with embedded tabs, newlines, and quotes", () => {
    const tsv = 'plain\t"has\ttab"\n"multi\nline"\t"say ""hi"""';
    expect(parseTsv(tsv)).toEqual([
      ["plain", "has\ttab"],
      ["multi\nline", 'say "hi"'],
    ]);
  });

  it("keeps a trailing empty row out but preserves empty fields", () => {
    expect(parseTsv("a\t\tb")).toEqual([["a", "", "b"]]);
    expect(parseTsv("a\tb\n")).toEqual([["a", "b"]]);
  });

  it("normalizes CRLF inside quoted fields and row terminators", () => {
    expect(parseTsv('"a\r\nb"\tc\r\n1\t2')).toEqual([
      ["a\nb", "c"],
      ["1", "2"],
    ]);
  });

  it("leaves literal quotes untouched when a field is not quoted", () => {
    expect(parseTsv('he said "hi"\tok')).toEqual([['he said "hi"', "ok"]]);
    expect(parseTsv('mid"quote\tok')).toEqual([['mid"quote', "ok"]]);
  });

  it("round-trips against the SheetEditor copy quoting semantics", () => {
    const cells = [
      ['with\ttab', 'multi\nline', 'say "hi"', 'plain', ''],
      ['second\r\nrow', 'x"y', '1', '2', '3'],
    ];
    const quoted = cells.map((row) =>
      row.map((raw) =>
        raw.includes('\n') || raw.includes('\t') || raw.includes('"')
          ? `"${raw.replace(/"/g, '""')}"`
          : raw
      ).join("\t")
    ).join("\n");
    // Embedded CRLF inside quoted fields is canonicalized to \n on paste,
    // matching Excel/Sheets clipboard behavior.
    const canonical = cells.map((row) => row.map((v) => v.replace(/\r\n/g, '\n')));
    expect(parseTsv(quoted)).toEqual(canonical);
  });
});
