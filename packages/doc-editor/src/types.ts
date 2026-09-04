import type { Footnote } from "./footnotes";

export interface DocContent {
  type: string;
  content?: DocContent[];
  text?: string;
  marks?: Array<{ type: string; attrs?: Record<string, unknown> }>;
  attrs?: Record<string, unknown>;
  comments?: ReviewComment[];
  footnotes?: Footnote[];
  [key: string]: unknown;
}

export interface CommentReply {
  id: string;
  author: string;
  text: string;
  createdAt: string;
}

export interface ReviewComment {
  id: string;
  author: string;
  text: string;
  from: number;
  to: number;
  resolved: boolean;
  createdAt: string;
  /** Threaded replies, newest last; absent in older files. */
  replies?: CommentReply[];
}

export interface DocEditorProps {
  initialContent?: DocContent;
  /** Other open Writer sessions that can be used as a compare baseline. */
  compareDocuments?: Array<{ id: string; title: string; content: DocContent | null }>;
  onChange?: (jsonContent: DocContent) => void;
  onWordCountChange?: (info: string) => void;
  zoomLevel?: number;
  /** Enables the platform spellchecker on the editing surface. */
  spellcheckEnabled?: boolean;
  onRequestNew?: () => void;
  onRequestOpen?: () => void;
  onRequestSave?: () => void;
  onRequestExportPdf?: () => void;
  onRequestExportDocx?: () => void;
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
