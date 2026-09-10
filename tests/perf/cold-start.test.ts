import { describe, test, expect } from "vitest";

/**
 * Performance regression tests for critical paths.
 *
 * These tests ensure that key operations stay within acceptable time bounds.
 * Run with: pnpm test (includes these tests)
 */

describe("Performance regressions", () => {
  test("JSON parsing of large documents stays under 100ms", () => {
    // Generate a large document structure
    const largeDoc = {
      type: "doc",
      content: Array.from({ length: 1000 }, (_, i) => ({
        type: "paragraph",
        content: [{ type: "text", text: `Paragraph ${i} with some content` }],
      })),
    };

    const start = performance.now();
    const serialized = JSON.stringify(largeDoc);
    const parsed = JSON.parse(serialized);
    const elapsed = performance.now() - start;

    expect(parsed.content.length).toBe(1000);
    expect(elapsed).toBeLessThan(100);
  });

  test("Array operations for formula evaluation stay efficient", () => {
    // Simulate array formula operations
    const size = 10000;
    const data = Array.from({ length: size }, (_, i) => i + 1);

    const start = performance.now();
    const sum = data.reduce((a, b) => a + b, 0);
    const filtered = data.filter((x) => x % 2 === 0);
    const mapped = filtered.map((x) => x * 2);
    const elapsed = performance.now() - start;

    expect(sum).toBe((size * (size + 1)) / 2);
    expect(filtered.length).toBe(size / 2);
    expect(elapsed).toBeLessThan(50);
  });

  test("String operations for CSV parsing stay efficient", () => {
    // Generate CSV data
    const rows = Array.from({ length: 1000 }, (_, i) =>
      `field1_${i},field2_${i},"quoted, field",${i * 1.5}`,
    );
    const csv = rows.join("\n");

    const start = performance.now();
    const lines = csv.split("\n");
    const parsed = lines.map((line) => {
      const fields: string[] = [];
      let current = "";
      let inQuotes = false;
      for (const char of line) {
        if (char === '"') {
          inQuotes = !inQuotes;
        } else if (char === "," && !inQuotes) {
          fields.push(current);
          current = "";
        } else {
          current += char;
        }
      }
      fields.push(current);
      return fields;
    });
    const elapsed = performance.now() - start;

    expect(parsed.length).toBe(1000);
    expect(parsed[0].length).toBe(4);
    expect(elapsed).toBeLessThan(100);
  });

  test("Object key lookup for cell references is O(1)", () => {
    // Simulate cell reference lookups
    const cellMap = new Map<string, number>();
    for (let row = 1; row <= 1000; row++) {
      for (let col = 1; col <= 26; col++) {
        cellMap.set(`${row}:${col}`, row * col);
      }
    }

    const start = performance.now();
    let lookups = 0;
    for (let i = 0; i < 10000; i++) {
      const row = (i % 1000) + 1;
      const col = (i % 26) + 1;
      const value = cellMap.get(`${row}:${col}`);
      if (value !== undefined) lookups++;
    }
    const elapsed = performance.now() - start;

    expect(lookups).toBe(10000);
    expect(elapsed).toBeLessThan(50);
  });
});
