import {
  ToolbarRow, ToolbarButton, ToolbarSep, ToolbarSelect, ToolbarColor
} from "@redoc/editor-common";
import {
  IconBold, IconItalic, IconUnderline, IconStrikethrough, IconAlignLeft, IconAlignCenter,
  IconAlignRight, IconAlignJustify, IconList, IconOrderedList, IconUndo, IconRedo, IconSearch,
  IconNew, IconFolderOpen, IconSave, IconPdf, IconPrint, IconCut, IconCopy, IconPaste,
  IconTable, IconImage, IconLink, IconIndent, IconOutdent, IconClearFormat, IconSuperscript,
  IconSubscript, IconTextColor, IconHighlight, IconLineSpacing, IconPage,
} from "@redoc/icons";

export interface DocToolbarProps {
  onRequestNew?: () => void;
  onRequestOpen?: () => void;
  onRequestSave?: () => void;
  onRequestExportPdf?: () => void;
  onPageSetup: () => void;
  onCut: () => void;
  onCopy: () => void;
  onPaste: () => void;
  onUndo: () => void;
  onRedo: () => void;
  findOpen: boolean;
  onToggleFind: () => void;
  onInsertTable: () => void;
  onInsertImage: () => void;
  onInsertLink: () => void;
  onInsertPageBreak: () => void;
  onAddTableRow: () => void;
  onDeleteTableRow: () => void;
  onAddTableColumn: () => void;
  onDeleteTableColumn: () => void;
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

export function DocToolbar(props: DocToolbarProps) {
  return (
    <>
      <ToolbarRow>
        <ToolbarButton title="New" onClick={() => props.onRequestNew?.()}><IconNew /></ToolbarButton>
        <ToolbarButton title="Open" onClick={() => props.onRequestOpen?.()}><IconFolderOpen /></ToolbarButton>
        <ToolbarButton title="Save" onClick={() => props.onRequestSave?.()}><IconSave /></ToolbarButton>
        <ToolbarButton title="Page Setup" onClick={props.onPageSetup}><IconPage /></ToolbarButton>
        <ToolbarButton title="Export PDF" onClick={() => props.onRequestExportPdf?.()}><IconPdf /></ToolbarButton>
        <ToolbarButton title="Print" onClick={() => window.print()}><IconPrint /></ToolbarButton>
        <ToolbarSep />
        <ToolbarButton title="Cut" onClick={props.onCut}><IconCut /></ToolbarButton>
        <ToolbarButton title="Copy" onClick={props.onCopy}><IconCopy /></ToolbarButton>
        <ToolbarButton title="Paste" onClick={props.onPaste}><IconPaste /></ToolbarButton>
        <ToolbarSep />
        <ToolbarButton title="Undo (Ctrl+Z)" onClick={props.onUndo}><IconUndo /></ToolbarButton>
        <ToolbarButton title="Redo (Ctrl+Y)" onClick={props.onRedo}><IconRedo /></ToolbarButton>
        <ToolbarSep />
        <ToolbarButton title="Find and Replace (Ctrl+F)" onClick={props.onToggleFind} active={props.findOpen}><IconSearch /></ToolbarButton>
        <ToolbarSep />
        <ToolbarButton title="Insert Table" onClick={props.onInsertTable}><IconTable /></ToolbarButton>
        <ToolbarButton title="Insert Image" onClick={props.onInsertImage}><IconImage /></ToolbarButton>
        <ToolbarButton title="Insert Link" onClick={props.onInsertLink}><IconLink /></ToolbarButton>
        <ToolbarButton title="Insert Page Break (Ctrl+Enter)" onClick={props.onInsertPageBreak}>¶</ToolbarButton>
        <ToolbarSep />
        <ToolbarButton title="Add table row" onClick={props.onAddTableRow}>+Row</ToolbarButton>
        <ToolbarButton title="Delete table row" onClick={props.onDeleteTableRow}>−Row</ToolbarButton>
        <ToolbarButton title="Add table column" onClick={props.onAddTableColumn}>+Col</ToolbarButton>
        <ToolbarButton title="Delete table column" onClick={props.onDeleteTableColumn}>−Col</ToolbarButton>
        <ToolbarButton title="Merge cells" onClick={props.onMergeCells}>Merge</ToolbarButton>
        <ToolbarButton title="Split cell" onClick={props.onSplitCell}>Split</ToolbarButton>
        <ToolbarButton title="Toggle header row" onClick={props.onToggleHeaderRow}>Hdr</ToolbarButton>
      </ToolbarRow>

      <ToolbarRow>
        <ToolbarSelect
          ariaLabel="Paragraph style"
          width="150px"
          value={props.currentBlockType}
          onChange={props.onChangeBlockType}
          options={[
            { value: "paragraph", label: "Default Paragraph Style" },
            { value: "heading1", label: "Heading 1" },
            { value: "heading2", label: "Heading 2" },
            { value: "heading3", label: "Heading 3" },
            { value: "heading4", label: "Heading 4" },
            { value: "heading5", label: "Heading 5" },
            { value: "heading6", label: "Heading 6" },
            { value: "blockquote", label: "Block Quote" },
            { value: "code_block", label: "Preformatted Text" },
          ]}
        />
        <ToolbarSelect
          ariaLabel="Font"
          width="120px"
          value={props.fontFamily}
          onChange={props.onChangeFontFamily}
          options={[
            { value: "Liberation Serif", label: "Liberation Serif" },
            { value: "Liberation Sans", label: "Liberation Sans" },
            { value: "Times New Roman", label: "Times New Roman" },
            { value: "Arial", label: "Arial" },
            { value: "Georgia", label: "Georgia" },
            { value: "Courier New", label: "Courier New" },
          ]}
        />
        <ToolbarSelect
          ariaLabel="Font size"
          width="52px"
          value={props.fontSize}
          onChange={props.onChangeFontSize}
          options={["8", "9", "10", "11", "12", "14", "16", "18", "20", "24", "28", "36"].map((s) => ({
            value: s,
            label: s,
          }))}
        />
        <ToolbarButton title="Shrink font size (Ctrl+[)" onClick={props.onShrinkFontSize}>A-</ToolbarButton>
        <ToolbarButton title="Grow font size (Ctrl+])" onClick={props.onGrowFontSize}>A+</ToolbarButton>
        <ToolbarSep />
        <ToolbarButton title="Bold (Ctrl+B)" onClick={props.onToggleBold}><IconBold /></ToolbarButton>
        <ToolbarButton title="Italic (Ctrl+I)" onClick={props.onToggleItalic}><IconItalic /></ToolbarButton>
        <ToolbarButton title="Underline (Ctrl+U)" onClick={props.onToggleUnderline}><IconUnderline /></ToolbarButton>
        <ToolbarButton title="Strikethrough" onClick={props.onToggleStrike}><IconStrikethrough /></ToolbarButton>
        <ToolbarButton title="Superscript (Ctrl+.)" onClick={props.onToggleSuper}><IconSuperscript /></ToolbarButton>
        <ToolbarButton title="Subscript (Ctrl+,)" onClick={props.onToggleSub}><IconSubscript /></ToolbarButton>
        <ToolbarButton title="Clear Direct Formatting" onClick={props.onClearFormatting}><IconClearFormat /></ToolbarButton>
        <ToolbarButton title="Format Painter" active={props.painterActive} onClick={props.onToggleFormatPainter}><IconClearFormat /></ToolbarButton>
        <ToolbarSep />
        <ToolbarColor title="Font Color" value={props.textColor} onChange={props.onChangeTextColor}>
          <IconTextColor />
          <span style={{ width: "14px", height: "3px", background: props.textColor, display: "block", "margin-top": "-2px" }} />
        </ToolbarColor>
        <ToolbarColor title="Highlight Color" value={props.highlightColor} onChange={props.onChangeHighlightColor}>
          <IconHighlight />
          <span style={{ width: "14px", height: "3px", background: props.highlightColor, display: "block", "margin-top": "-2px" }} />
        </ToolbarColor>
        <ToolbarSep />
        <ToolbarButton title="Align Left" onClick={props.onAlignLeft}><IconAlignLeft /></ToolbarButton>
        <ToolbarButton title="Align Center" onClick={props.onAlignCenter}><IconAlignCenter /></ToolbarButton>
        <ToolbarButton title="Align Right" onClick={props.onAlignRight}><IconAlignRight /></ToolbarButton>
        <ToolbarButton title="Justify" onClick={props.onAlignJustify}><IconAlignJustify /></ToolbarButton>
        <ToolbarSep />
        <ToolbarButton title="Bullets" onClick={props.onListBullet}><IconList /></ToolbarButton>
        <ToolbarButton title="Numbering" onClick={props.onListOrdered}><IconOrderedList /></ToolbarButton>
        <ToolbarButton title="Decrease Indent" onClick={props.onDecreaseIndent}><IconOutdent /></ToolbarButton>
        <ToolbarButton title="Increase Indent" onClick={props.onIncreaseIndent}><IconIndent /></ToolbarButton>
        <ToolbarSep />
        <ToolbarButton title="Line Spacing" onClick={() => props.onChangeLineSpacing(1.5)}>
          <IconLineSpacing />
        </ToolbarButton>
        <ToolbarSelect
          ariaLabel="Line spacing"
          width="64px"
          value={props.currentLineSpacing}
          onChange={(v) => props.onChangeLineSpacing(Number(v))}
          options={[
            { value: "1", label: "Single" },
            { value: "1.15", label: "1.15" },
            { value: "1.5", label: "1.5" },
            { value: "2", label: "Double" },
          ]}
        />
        <ToolbarSep />
        <span style={{ "font-size": "11px", color: "var(--text-secondary)" }}>Space</span>
        <input
          type="number"
          min={0}
          max={72}
          aria-label="Spacing before"
          class="g-toolbar-input"
          value={props.spacingBefore}
          onInput={(e) => props.onChangeSpacingBefore(Math.max(0, Number(e.currentTarget.value) || 0))}
          style={{ width: "44px", height: "24px", padding: "0 4px" }}
        />
        <span style={{ "font-size": "10px", color: "var(--text-muted)" }}>before</span>
        <input
          type="number"
          min={0}
          max={72}
          aria-label="Spacing after"
          class="g-toolbar-input"
          value={props.spacingAfter}
          onInput={(e) => props.onChangeSpacingAfter(Math.max(0, Number(e.currentTarget.value) || 0))}
          style={{ width: "44px", height: "24px", padding: "0 4px" }}
        />
        <span style={{ "font-size": "10px", color: "var(--text-muted)" }}>after</span>
      </ToolbarRow>
    </>
  );
}
