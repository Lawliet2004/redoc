export type SupportedFileType = "redoc" | "csv" | "xlsx" | "docx" | "pptx";

/** Resolve a local path to the file adapter that should handle it. */
export function supportedFileType(path: string): SupportedFileType | null {
  const name = path.trim().split(/[\\/]/).pop() || "";
  const extension = name.includes(".") ? name.slice(name.lastIndexOf(".") + 1).toLowerCase() : "";
  return ["redoc", "csv", "xlsx", "docx", "pptx"].includes(extension)
    ? (extension as SupportedFileType)
    : null;
}

const ZIP_MAGIC = [0x50, 0x4b, 0x03, 0x04];
const UTF8_BOM = [0xef, 0xbb, 0xbf];

/** True when the leading bytes match every expected byte (short prefixes match). */
function hasMagicPrefix(bytes: Uint8Array, magic: number[]): boolean {
  if (bytes.length < 2) return false;
  const bound = Math.min(bytes.length, magic.length);
  for (let i = 0; i < bound; i += 1) {
    if (bytes[i] !== magic[i]) return false;
  }
  return true;
}

export type MagicMismatch = {
  type: SupportedFileType;
  reason: string;
};

/**
 * Second pass after extension routing: verify container magic bytes before
 * import. `.redoc`/`.docx`/`.xlsx`/`.pptx` are ZIP containers and must start
 * with the local-file-header magic `PK\x03\x04` (an empty-archive `PK\x05\x06`
 * or spanned `PK\x07\x08` is not a document); `.csv` must be text (UTF-8 BOM
 * allowed, no NUL bytes, no binary ZIP magic).
 */
export function verifyMagicBytes(
  type: SupportedFileType,
  bytes: Uint8Array,
): MagicMismatch | null {
  if (type === "csv") {
    if (hasMagicPrefix(bytes, ZIP_MAGIC)) {
      return { type, reason: "CSV file starts with ZIP bytes; it is probably a renamed .zip/.xlsx file" };
    }
    let start = 0;
    if (hasMagicPrefix(bytes, UTF8_BOM)) start = UTF8_BOM.length;
    const bound = Math.min(bytes.length, 512);
    for (let i = start; i < bound; i += 1) {
      if (bytes[i] === 0x00) {
        return { type, reason: "CSV file contains NUL bytes; it is not plain text" };
      }
    }
    return null;
  }
  if (!hasMagicPrefix(bytes, ZIP_MAGIC)) {
    return {
      type,
      reason: `${type.toUpperCase()} file does not start with ZIP bytes (PK); it may be corrupt or a different format`,
    };
  }
  return null;
}

