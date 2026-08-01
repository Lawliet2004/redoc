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
});
