/** Normalize cell links before they reach canvas navigation or XLSX export. */
export function normalizeSheetHyperlink(value: string): string | null {
  const target = value.trim();
  if (!target || /[\u0000-\u001f\u007f]/.test(target)) return null;
  if (/^(?:internal:|#)/i.test(target)) {
    const location = target.replace(/^internal:/i, "").replace(/^#/, "").trim();
    return location ? `internal:${location}` : null;
  }
  if (/^https?:\/\//i.test(target)) {
    try {
      const url = new URL(target);
      return url.hostname ? target : null;
    } catch {
      return null;
    }
  }
  if (/^mailto:/i.test(target)) return /^mailto:[^\s@]+@[^\s@]+$/i.test(target) ? target : null;
  if (/^tel:/i.test(target)) return /^tel:[+0-9().\-\s]+$/i.test(target) ? target : null;
  return null;
}

export interface InternalSheetLocation {
  sheetName: string | null;
  row: number;
  col: number;
}

export function parseInternalSheetLocation(value: string): InternalSheetLocation | null {
  const normalized = normalizeSheetHyperlink(value);
  if (!normalized?.startsWith("internal:")) return null;
  const location = normalized.slice("internal:".length);
  const bang = location.lastIndexOf("!");
  const rawSheet = bang >= 0 ? location.slice(0, bang).trim() : "";
  const rawCell = (bang >= 0 ? location.slice(bang + 1) : location).trim();
  const cell = rawCell.match(/^\$?([A-Z]{1,3})\$?([1-9]\d{0,5})$/i);
  if (!cell) return null;
  let col = 0;
  for (const character of cell[1].toUpperCase()) col = col * 26 + character.charCodeAt(0) - 64;
  const row = Number(cell[2]);
  if (!Number.isSafeInteger(row) || row < 1 || row > 100_000 || col < 1 || col > 1_000) return null;
  const sheetName = rawSheet.replace(/^'|'$/g, "").trim();
  return { sheetName: sheetName || null, row, col };
}
