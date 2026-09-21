import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const sourcePath = resolve(process.cwd(), "crates", "formula", "src", "catalog.rs");
const outputPath = resolve(process.cwd(), "packages", "sheet-editor", "src", "formulaFunctions.ts");
const source = readFileSync(sourcePath, "utf8");
const match = source.match(/SUPPORTED_FUNCTION_NAMES:\s*&\[&str\]\s*=\s*&\[(.*?)\];/s);
if (!match) throw new Error(`Could not find SUPPORTED_FUNCTION_NAMES in ${sourcePath}`);

const names = [...match[1].matchAll(/"([A-Z][A-Z0-9.]*)"/g)].map((entry) => entry[1]);
if (names.length === 0) throw new Error("Formula catalog is empty");
if (new Set(names).size !== names.length) throw new Error("Formula catalog contains duplicate names");
const sorted = [...names].sort((a, b) => a.localeCompare(b));
if (JSON.stringify(names) !== JSON.stringify(sorted)) throw new Error("Formula catalog must be sorted");

const output = [
  "/** Generated from crates/formula/src/catalog.rs. Do not edit by hand. */",
  "export const FORMULA_FUNCTION_NAMES = [",
  ...names.map((name) => `  ${JSON.stringify(name)},`),
  "] as const;",
  "",
].join("\n");
if (process.argv.includes("--check")) {
  const current = readFileSync(outputPath, "utf8");
  if (current.replace(/\r\n/g, "\n") !== output) {
    throw new Error(`Generated formula catalog is stale; run node scripts/generate-formula-catalog.mjs`);
  }
  console.log(`Formula catalog is current: ${names.length} names.`);
} else {
  writeFileSync(outputPath, output);
  console.log(`Generated ${names.length} formula names at ${outputPath}`);
}
