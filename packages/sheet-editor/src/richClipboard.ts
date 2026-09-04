/**
 * Internal rich clipboard payload for in-app copy/cut/paste.
 * Carries raw values, display values and styles so a Redoc-to-Redoc paste
 * keeps formatting — the system clipboard only carries TSV text.
 */
export type RichClipboard = {
  /** Clipboard kind marker (future-proofing for other editors). */
  kind: "sheet";
  /** Source selection bounds. */
  startRow: number;
  startCol: number;
  endRow: number;
  endCol: number;
  /** Cells keyed `${rowOffset}:${colOffset}` relative to the selection origin. */
  cells: Record<string, { raw: string; display: string; style?: Record<string, unknown> }>;
  /** Monotonic stamp; cleared when content changes underneath (cut). */
  cut: boolean;
};

let internalClipboard: RichClipboard | null = null;

export function setRichClipboard(payload: RichClipboard | null) {
  internalClipboard = payload;
}

export function takeRichClipboard(): RichClipboard | null {
  const payload = internalClipboard;
  if (payload?.cut) {
    // A cut stash is single-use, like Excel.
    internalClipboard = null;
  }
  return payload;
}

export function peekRichClipboard(): RichClipboard | null {
  return internalClipboard;
}

/**
 * Shift relative A1 references in a formula by (deltaRow, deltaCol), honoring
 * `$` absoluteness. Mirrors the Rust `adjust_formula_references` rules for the
 * same-sheet case; quoted strings are skipped.
 */
export function shiftFormulaReferences(formula: string, deltaRow: number, deltaCol: number): string {
  if (!formula.startsWith("=")) return formula;
  let result = "=";
  let rest = formula.slice(1);
  while (rest.length > 0) {
    const quote = rest.indexOf('"');
    const ref = rest.search(/(\$?[A-Za-z]{1,3}\$?[0-9]+)/);
    if (quote !== -1 && (ref === -1 || quote < ref)) {
      const endQuote = rest.indexOf('"', quote + 1);
      const stop = endQuote === -1 ? rest.length : endQuote + 1;
      result += rest.slice(0, stop);
      rest = rest.slice(stop);
      continue;
    }
    if (ref === -1) {
      result += rest;
      break;
    }
    result += rest.slice(0, ref);
    const match = rest.slice(ref).match(/^(\$?)([A-Za-z]{1,3})(\$?)([0-9]+)/);
    if (!match) {
      // Advance one character to avoid an infinite loop on a partial match.
      result += rest[ref];
      rest = rest.slice(ref + 1);
      continue;
    }
    const [whole, colDollar, colLetters, rowDollar, rowDigits] = match;
    const parsed = parseCol(colLetters);
    if (parsed === null) {
      result += whole;
    } else {
      const shiftedCol = colDollar ? parsed : Math.max(1, parsed + deltaCol);
      const shiftedRow = rowDollar
        ? Number(rowDigits)
        : Math.max(1, Number(rowDigits) + deltaRow);
      result += `${colDollar}${colToLetters(shiftedCol)}${rowDollar}${shiftedRow}`;
    }
    rest = rest.slice(ref + whole.length);
  }
  return result;
}

function parseCol(letters: string): number | null {
  let col = 0;
  for (const c of letters.toUpperCase()) {
    const value = c.charCodeAt(0) - 64; // 'A' = 1
    if (value < 1 || value > 26) return null;
    col = col * 26 + value;
  }
  return col || null;
}

function colToLetters(col: number): string {
  let letters = "";
  while (col > 0) {
    const rem = (col - 1) % 26;
    letters = String.fromCharCode(65 + rem) + letters;
    col = Math.floor((col - 1) / 26);
  }
  return letters;
}
