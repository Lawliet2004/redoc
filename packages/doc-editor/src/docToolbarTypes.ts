export interface DocToolbarProps {
  onRequestNew?: () => void;
  onRequestOpen?: () => void;
  onRequestSave?: () => void;
  onRequestExportPdf?: () => void;
  onRequestExportDocx?: () => void;
  onPageSetup: () => void;
  onCut: () => void;
  onCopy: () => void;
  onPaste: () => void;
  onUndo: () => void;
  onRedo: () => void;
  /** Opens the paginated print preview overlay. */
  onPrintPreview?: () => void;
  findOpen: boolean;
  onToggleFind: () => void;
  /** Marks active at the caret / across the selection (bold, italic, …). */
  activeMarks?: Record<string, boolean>;
  /** Selection is inside a bullet list. */
  bulletListActive?: boolean;
  /** Selection is inside an ordered list. */
  orderedListActive?: boolean;
  /** Adds a comment anchored to the current selection. */
  onAddComment?: () => void;
  onInsertTable: () => void;
  onInsertImage: () => void;
  onInsertLink: () => void;
  onInsertBookmark: () => void;
  onInsertPageBreak: () => void;
  onInsertPageField: () => void;
  onInsertNumPagesField: () => void;
  onInsertSectionBreak: () => void;
  onInsertToc: () => void;
  onAddTableRow: () => void;
  onDeleteTableRow: () => void;
  onAddTableColumn: () => void;
  onDeleteTableColumn: () => void;
  onDeleteTable: () => void;
  onMergeCells: () => void;
  onSplitCell: () => void;
  onToggleHeaderRow: () => void;
  currentBlockType: string;
  onChangeBlockType: (type: string) => void;
  fontFamily: string;
  onChangeFontFamily: (family: string) => void;
  fontSize: string;
  onChangeFontSize: (size: string) => void;
  onShrinkFontSize: () => void;
  onGrowFontSize: () => void;
  onToggleBold: () => void;
  onToggleItalic: () => void;
  onToggleUnderline: () => void;
  onToggleStrike: () => void;
  onToggleSuper: () => void;
  onToggleSub: () => void;
  onClearFormatting: () => void;
  painterActive: boolean;
  onToggleFormatPainter: () => void;
  textColor: string;
  onChangeTextColor: (c: string) => void;
  highlightColor: string;
  onChangeHighlightColor: (c: string) => void;
  onAlignLeft: () => void;
  onAlignCenter: () => void;
  onAlignRight: () => void;
  onAlignJustify: () => void;
  onListBullet: () => void;
  onListOrdered: () => void;
  onDecreaseIndent: () => void;
  onIncreaseIndent: () => void;
  currentLineSpacing: string;
  onChangeLineSpacing: (spacing: number) => void;
  spacingBefore: number;
  spacingAfter: number;
  onChangeSpacingBefore: (pts: number) => void;
  onChangeSpacingAfter: (pts: number) => void;
}
