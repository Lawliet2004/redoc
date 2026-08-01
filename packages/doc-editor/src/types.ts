export interface DocContent {
  type: string;
  content?: DocContent[];
  text?: string;
  marks?: Array<{ type: string; attrs?: Record<string, unknown> }>;
  attrs?: Record<string, unknown>;
  [key: string]: unknown;
}

export interface DocEditorProps {
  initialContent?: DocContent;
  onChange?: (jsonContent: DocContent) => void;
  onWordCountChange?: (info: string) => void;
  zoomLevel?: number;
  onRequestNew?: () => void;
  onRequestOpen?: () => void;
  onRequestSave?: () => void;
  onRequestExportPdf?: () => void;
}

export interface SelectionState {
  from: number;
  to: number;
}

export interface TableCommandState {
  tablePos: number | null;
  cellPos: number | null;
  rowPos: number | null;
}
