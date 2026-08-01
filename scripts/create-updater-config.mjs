import { writeFileSync } from "node:fs";
import { join } from "node:path";

const publicKey = process.env.TAURI_UPDATER_PUBLIC_KEY;
const endpoint = process.env.TAURI_UPDATER_ENDPOINT;
if (!publicKey || !endpoint) {
  console.error("Updater release requires TAURI_UPDATER_PUBLIC_KEY and TAURI_UPDATER_ENDPOINT.");
  process.exit(1);
}

const path = join(process.cwd(), "apps", "desktop", "src-tauri", "tauri.generated.release.conf.json");
writeFileSync(
  path,
  `${JSON.stringify({
    bundle: { createUpdaterArtifacts: "v1Compatible" },
    plugins: { updater: { pubkey: publicKey, endpoints: [endpoint] } },
  }, null, 2)}\n`,
  "utf8",
);
console.log(`Wrote release updater configuration to ${path}`);
