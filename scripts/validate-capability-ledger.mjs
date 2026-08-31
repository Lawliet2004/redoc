import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

const ledgerPath = resolve(process.cwd(), "docs", "capability-ledger.json");
const allowedStatuses = new Set(["absent", "partial", "implemented", "verified", "interoperable"]);
const allowedAreas = new Set(["shared", "document", "spreadsheet", "presentation", "reliability", "performance", "interoperability"]);

function fail(message) {
  console.error(`Capability ledger: ${message}`);
  process.exitCode = 1;
}

if (!existsSync(ledgerPath)) {
  fail(`missing ${ledgerPath}`);
} else {
  let ledger;
  try {
    ledger = JSON.parse(readFileSync(ledgerPath, "utf8"));
  } catch (error) {
    fail(`invalid JSON (${error instanceof Error ? error.message : String(error)})`);
  }

  if (ledger) {
    const features = Array.isArray(ledger.features) ? ledger.features : [];
    const ids = new Set();
    if (ledger.schemaVersion !== 1) fail("schemaVersion must be 1");
    if (!ledger.statusDefinitions || typeof ledger.statusDefinitions !== "object") fail("statusDefinitions is required");
    if (features.length === 0) fail("features must contain at least one entry");

    for (const [index, feature] of features.entries()) {
      const prefix = `features[${index}]`;
      if (!feature || typeof feature !== "object") {
        fail(`${prefix} must be an object`);
        continue;
      }
      for (const field of ["id", "area", "title", "status", "evidence", "targetPhase"]) {
        if (!(field in feature)) fail(`${prefix}.${field} is required`);
      }
      if (typeof feature.id !== "string" || feature.id.length === 0) fail(`${prefix}.id must be a non-empty string`);
      else if (ids.has(feature.id)) fail(`${prefix}.id duplicates ${feature.id}`);
      else ids.add(feature.id);
      if (!allowedAreas.has(feature.area)) fail(`${prefix}.area is invalid: ${feature.area}`);
      if (!allowedStatuses.has(feature.status)) fail(`${prefix}.status is invalid: ${feature.status}`);
      if (!Array.isArray(feature.evidence)) fail(`${prefix}.evidence must be an array`);
      if (feature.status !== "absent" && (!Array.isArray(feature.evidence) || feature.evidence.length === 0)) {
        fail(`${prefix} needs evidence unless status is absent`);
      }
      if (feature.status === "interoperable" && feature.evidence.every((item) => !String(item).match(/(fixture|roundtrip|interop|docx|xlsx|pptx)/i))) {
        fail(`${prefix} marked interoperable without an interoperability evidence reference`);
      }
    }
    if (process.exitCode !== 1) console.log(`Capability ledger valid: ${features.length} feature entries.`);
  }
}
