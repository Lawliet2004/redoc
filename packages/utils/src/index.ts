export function formatDate(timestampSecs: number): string {
  const date = new Date(timestampSecs * 1000);
  return date.toLocaleDateString(undefined, {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}
export function parseTsv(tsv: string): string[][] {
  const rows: string[][] = [];
  let row: string[] = [];
  let field = "";
  let inQuotes = false;
  const pushField = () => {
    row.push(field);
    field = "";
  };
  const pushRow = () => {
    pushField();
    rows.push(row);
    row = [];
  };
  for (let index = 0; index < tsv.length; index += 1) {
    const ch = tsv[index];
    if (inQuotes) {
      if (ch === '"') {
        if (tsv[index + 1] === '"') {
          field += '"';
          index += 1;
        } else {
          inQuotes = false;
        }
      } else if (ch === '\r' && tsv[index + 1] === '\n') {
        field += '\n';
        index += 1;
      } else {
        field += ch;
      }
      continue;
    }
    if (ch === '"' && field === "") {
      inQuotes = true;
      continue;
    }
    if (ch === '\t') {
      pushField();
      continue;
    }
    if (ch === '\n') {
      pushRow();
      continue;
    }
    if (ch === '\r') {
      pushRow();
      if (tsv[index + 1] === '\n') index += 1;
      continue;
    }
    field += ch;
  }
  if (field !== "" || row.length > 0) {
    pushRow();
  }
  return rows;
}
