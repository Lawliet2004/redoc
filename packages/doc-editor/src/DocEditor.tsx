import { createEffect, createSignal, onCleanup, onMount, Show, For } from "solid-js";
import { EditorState, Plugin, TextSelection, NodeSelection } from "prosemirror-state";
import { Decoration, DecorationSet, EditorView } from "prosemirror-view";
import { Schema, MarkSpec, NodeSpec, Mark, Node, Fragment, Slice, DOMSerializer } from "prosemirror-model";
import { schema as basicSchema } from "prosemirror-schema-basic";
import { addListNodes, wrapInList, sinkListItem, liftListItem } from "prosemirror-schema-list";
import { history, undo, redo } from "prosemirror-history";
import { inputRules, wrappingInputRule, smartQuotes, emDash, ellipsis } from "prosemirror-inputrules";
import { keymap } from "prosemirror-keymap";
import { baseKeymap, toggleMark, setBlockType } from "prosemirror-commands";
import {
  columnResizing,
  tableEditing,
  mergeCells,
  splitCell,
  addRowAfter,
  deleteRow,
  addColumnAfter,
  deleteColumn,
  tableNodes,
  toggleHeaderRow as pmToggleHeaderRow,
  deleteTable,
} from "prosemirror-tables";
import { Dialog, showToast } from "@redoc/ui";
import { PrintPreview } from "./PrintPreview";
import {
  IconBold, IconItalic, IconUnderline, IconStrikethrough, IconAlignLeft, IconAlignCenter,
  IconAlignRight, IconAlignJustify, IconList, IconOrderedList, IconUndo, IconRedo, IconSearch,
  IconNew, IconFolderOpen, IconSave, IconPdf, IconPrint, IconCut, IconCopy, IconPaste,
  IconTable, IconImage, IconLink, IconIndent, IconOutdent, IconClearFormat, IconSuperscript,
  IconSubscript, IconTextColor, IconHighlight, IconLineSpacing, IconProperties, IconPage,
  IconStyles, IconGallery, IconNavigator,
} from "@redoc/icons";
import {
  ToolbarRow, ToolbarButton, ToolbarSep, ToolbarSelect, ToolbarColor,
  Ruler, FindBar, IconSidebar, type SidebarPanel,
  ContextMenu, type ContextMenuItem,
  EDITOR_COMMAND, type EditorCommandDetail, emitEditorCommand,
} from "@redoc/editor-common";
import { commands } from "@redoc/api-client";
import { BubbleToolbar } from "./BubbleToolbar";
import { PageSetupDialog, type PageSetupConfig } from "./PageSetupDialog";
import { DEFAULT_PAGE_SETUP, normalizePageSetupConfig, formatPrintHeaderFooter } from "./pageSetup";
import { DocToolbar } from "./DocToolbar";
import { FindReplace } from "./FindReplace";
import { transformPastedHTML } from "./pasteSanitizer";
import { findBookmarkPosition, normalizeBookmarkName } from "./bookmarks";
import { normalizeDocLink } from "./links";
import { documentFieldResult, normalizeDocumentFieldKind, type DocumentFieldKind } from "./fields";
import { buildTocContent, sanitizeTocAnchor, type TocEntry } from "./toc";
import type { DocContent, DocEditorProps, ReviewComment, TableCommandState } from "./types";
import { mapReviewCommentAnchors } from "./review";
import { compareDocContent, findCompareSource } from "./compare";
import {
  BUILT_IN_STYLES,
  customStylesForDoc,
  normalizeStyleName,
  styleBlockAttrs,
  styleIdFromName,
  styleInlineAttrs,
  type NamedStyleDef,
} from "./styles";
import {
  newFootnoteId,
  readFootnotes,
  syncFootnotesWithRefs,
  type Footnote,
} from "./footnotes";
import {
  getTrackedChangeStats,
  getTrackedChangeSummaries,
  resolveTrackedChange,
  resolveTrackedChanges,
  insertedRangesFromTransaction,
  TRACK_DELETE_MARK,
  TRACK_INSERT_MARK,
  type TrackedChangeDecision,
  type TrackedChangeKind,
} from "./trackedChanges";

import { mySchema } from "./schema";

function buildDocJson(
  doc: Node,
  setup: PageSetupConfig,
  comments: ReviewComment[] = [],
  styles: NamedStyleDef[] = [],
  footnotes: Footnote[] = [],
): DocContent {
  return {
    ...(doc.toJSON() as DocContent),
    pageSetup: setup,
    ...(comments.length ? { comments } : {}),
    ...(styles.length ? { styles } : {}),
    ...(footnotes.length ? { footnotes } : {}),
  };
}

function docJsonForCompare(json: DocContent): string {
  const copy = { ...json };
  delete copy.pageSetup;
  delete (copy as Record<string, unknown>).styles;
  delete copy.footnotes;
  return JSON.stringify(copy);
}

function prunePastedSlice(slice: Slice, schema: Schema): Slice {
  const mapFragment = (fragment: Fragment): Fragment => {
    const nodes: Node[] = [];
    fragment.forEach((node) => {
      if (!schema.nodes[node.type.name]) return;
      if (node.isText) {
        const marks = node.marks.filter((m) => schema.marks[m.type.name]);
        nodes.push(schema.text(node.text ?? "", marks));
      } else {
        const content = mapFragment(node.content);
        nodes.push(node.copy(content));
      }
    });
    return Fragment.fromArray(nodes);
  };
  return new Slice(mapFragment(slice.content), slice.openStart, slice.openEnd);
}

function syncToolbarFromSelection(
  state: EditorState,
  setFontFamily: (v: string) => void,
  setFontSize: (v: string) => void,
  setTextColor: (v: string) => void,
  setHighlightColor: (v: string) => void,
) {
  const { $from, empty } = state.selection;
  const marks = empty ? [...$from.marks(), ...state.storedMarks ?? []] : $from.marks();
  const fontFamilyMark = marks.find((m) => m.type.name === "fontFamily");
  setFontFamily(fontFamilyMark?.attrs.family ?? "Liberation Serif");
  const fontSizeMark = marks.find((m) => m.type.name === "fontSize");
  setFontSize(fontSizeMark?.attrs.size ?? "12");
  const colorMark = marks.find((m) => m.type.name === "color");
  if (colorMark) setTextColor(colorMark.attrs.color);
  const highlightMark = marks.find((m) => m.type.name === "highlight");
  if (highlightMark) setHighlightColor(highlightMark.attrs.color);
}

function collectMatches(
  doc: EditorState["doc"],
  query: string,
  matchCase: boolean
): Array<{ from: number; to: number }> {
  if (!query) return [];
  const q = matchCase ? query : query.toLowerCase();
  const matches: Array<{ from: number; to: number }> = [];
  doc.descendants((node, pos) => {
    if (!node.isText || !node.text) return;
    const text = matchCase ? node.text : node.text.toLowerCase();
    let index = text.indexOf(q);
    while (index >= 0) {
      matches.push({ from: pos + index, to: pos + index + query.length });
      index = text.indexOf(q, index + q.length);
    }
  });
  return matches;
}

/**
 * Best-effort text extraction from a raw document JSON body. Used only when
 * the schema cannot parse the document — we keep the user's words instead of
 * silently substituting an empty page.
 */
function extractPlainText(content: unknown): string {
  const parts: string[] = [];
  const walk = (value: any) => {
    if (!value || typeof value !== "object") return;
    if (Array.isArray(value)) {
      value.forEach(walk);
      return;
    }
    if (typeof value.text === "string") parts.push(value.text);
    if (value.content) walk(value.content);
  };
  walk(content);
  return parts.join("").trim();
}

function bookmarkNamesInDocument(doc: Node): Set<string> {
  const names = new Set<string>();
  doc.descendants((node) => {
    if (!node.isText) return;
    for (const mark of node.marks) {
      if (mark.type.name !== "bookmark") continue;
      const name = normalizeBookmarkName(String(mark.attrs.name || ""));
      if (name) names.add(name.toLowerCase());
    }
  });
  return names;
}

function findPlugin(getQuery: () => string, getMatchCase: () => boolean, getActive: () => number) {
  // Matches depend only on (doc, query, matchCase) — memoize so selection
  // changes and cursor blinks don't rescan the whole document.
  let cacheDoc: EditorState["doc"] | null = null;
  let cacheQuery = "";
  let cacheMatchCase = false;
  let cacheMatches: Array<{ from: number; to: number }> = [];
  return new Plugin({
    props: {
      decorations(state) {
        const query = getQuery();
        if (!query) return null;
        const matchCase = getMatchCase();
        if (
          state.doc !== cacheDoc || query !== cacheQuery || matchCase !== cacheMatchCase
        ) {
          cacheMatches = collectMatches(state.doc, query, matchCase);
          cacheDoc = state.doc;
          cacheQuery = query;
          cacheMatchCase = matchCase;
        }
        const active = getActive();
        const decos = cacheMatches.map((m, i) =>
          Decoration.inline(m.from, m.to, {
            class: i === active ? "doc-find-active" : "doc-find-match",
            style:
              i === active
                ? "background:#f9ab00;outline:1px solid #e37400"
                : "background:#fef08a",
          })
        );
        return DecorationSet.create(state.doc, decos);
      },
    },
  });
}

/**
 * Word-style automatic track-changes: while the toggle is on, every inserted
 * range produced by user transactions gets a trackInsert mark (skipping
 * changes the plugin itself appends, marked with the "auto-track" meta).
 */
function autoTrackChangesPlugin(getEnabled: () => boolean, createMark: () => any) {
  return new Plugin({
    appendTransaction: (transactions, _oldState, newState) => {
      if (!getEnabled()) return null;
      let tr = newState.tr;
      let marked = false;
      for (const transaction of transactions) {
        if (transaction.getMeta("auto-track")) continue;
        if (!transaction.docChanged) continue;
        for (const range of insertedRangesFromTransaction(transaction)) {
          const from = Math.max(0, Math.min(range.from, newState.doc.content.size));
          const to = Math.max(from, Math.min(range.to, newState.doc.content.size));
          if (to <= from) continue;
          // Only mark ranges that contain text (typing/paste). Block-level
          // inserts such as tables, rules and hard breaks are left alone.
          let hasText = false;
          let hasBlock = false;
          newState.doc.nodesBetween(from, to, (node) => {
            if (node.isText) hasText = true;
            if (node.isBlock && node.type.name !== "paragraph") hasBlock = true;
            return true;
          });
          if (!hasText || hasBlock) continue;
          tr = tr.addMark(from, to, createMark());
          marked = true;
        }
      }
      if (!marked) return null;
      tr.setMeta("auto-track", true);
      // Keep our appended mark transaction out of undo-history groups that
      // would otherwise split it from the edit it decorates.
      tr.setMeta("addToHistory", false);
      return tr;
    },
  });
}

function reviewCommentPlugin(getComments: () => ReviewComment[]) {
  return new Plugin({
    props: {
      decorations(state) {
        const decorations: Decoration[] = [];
        for (const comment of getComments()) {
          if (comment.resolved || comment.from >= comment.to) continue;
          if (comment.from < 0 || comment.to > state.doc.content.size) continue;
          try {
            decorations.push(
              Decoration.inline(comment.from, comment.to, {
                class: "doc-comment-highlight",
                "data-comment-id": comment.id,
              }),
            );
          } catch {
            // Stale anchors are ignored until the next comment edit.
          }
        }
        return decorations.length ? DecorationSet.create(state.doc, decorations) : null;
      },
    },
  });
}

export function DocEditor(props: DocEditorProps) {
  let editorRef!: HTMLDivElement;
  let view: EditorView | null = null;
  let lastEmittedJson = "";
  const [findOpen, setFindOpen] = createSignal(false);
  const [findQuery, setFindQuery] = createSignal("");
  const [replaceWith, setReplaceWith] = createSignal("");
  const [matchCount, setMatchCount] = createSignal(0);
  const [matchIndex, setMatchIndex] = createSignal(0);
  const [matchCase, setMatchCase] = createSignal(false);
  const [layoutMode, setLayoutMode] = createSignal<"paginated" | "focused">("paginated");
  const [fontFamily, setFontFamily] = createSignal("Liberation Serif");
  const [fontSize, setFontSize] = createSignal("12");
  const [textColor, setTextColor] = createSignal("#000000");
  const [highlightColor, setHighlightColor] = createSignal("#ffff00");
  const [currentBlockType, setCurrentBlockType] = createSignal("paragraph");
  const [currentLineSpacing, setCurrentLineSpacing] = createSignal("1.5");
  const [spacingBefore, setSpacingBefore] = createSignal(0);
  const [spacingAfter, setSpacingAfter] = createSignal(0);
  const [tabStops, setTabStops] = createSignal<number[]>([96, 192, 288]);
  const [pageCount, setPageCount] = createSignal(1);
  // Browsers repeat position:fixed print headers on every page but expose no
  // page counter to DOM content. For multi-page prints we omit the page
  // number rather than print a wrong one; DOCX/PDF exports carry real fields.
  const printPageIndex = () => (pageCount() > 1 ? "" : "1");
  const [headings, setHeadings] = createSignal<{ level: number; text: string; pos: number }[]>([]);
  const [linkDialogOpen, setLinkDialogOpen] = createSignal(false);
  const [linkHref, setLinkHref] = createSignal("https://");
  const [bookmarkDialogOpen, setBookmarkDialogOpen] = createSignal(false);
  const [bookmarkName, setBookmarkName] = createSignal("");
  const [bookmarkError, setBookmarkError] = createSignal("");
  const [linkError, setLinkError] = createSignal("");
  const [imageDialogOpen, setImageDialogOpen] = createSignal(false);
  const [imageSrc, setImageSrc] = createSignal("");
  const [printPreviewOpen, setPrintPreviewOpen] = createSignal(false);
  const [selectedImage, setSelectedImage] = createSignal<{ pos: number; width: number; height: number } | null>(null);
  const [contextMenu, setContextMenu] = createSignal<{ x: number; y: number; items: ContextMenuItem[] } | null>(null);

  const [painterState, setPainterState] = createSignal<"idle" | "armed" | "locked">("idle");
  const [storedMarks, setStoredMarks] = createSignal<Mark[]>([]);
  const [storedAttrs, setStoredAttrs] = createSignal<Record<string, unknown>>({});

  const [bubbleVisible, setBubbleVisible] = createSignal(false);
  const [bubbleTop, setBubbleTop] = createSignal(0);
  const [bubbleLeft, setBubbleLeft] = createSignal(0);
  const [comments, setComments] = createSignal<ReviewComment[]>(props.initialContent?.comments ?? []);
  const [commentDialogOpen, setCommentDialogOpen] = createSignal(false);
  const [commentDraft, setCommentDraft] = createSignal("");
  const [commentAnchor, setCommentAnchor] = createSignal<{ from: number; to: number } | null>(null);
  const [selectedCommentId, setSelectedCommentId] = createSignal<string | null>(null);
  const [trackedChangeRevision, setTrackedChangeRevision] = createSignal(0);
  const [compareBaseline, setCompareBaseline] = createSignal<DocContent | null>(null);
  const [compareBaselineLabel, setCompareBaselineLabel] = createSignal("Since opened");
  const [compareSourceId, setCompareSourceId] = createSignal("");
  const [compareDetailsOpen, setCompareDetailsOpen] = createSignal(false);
  const [loadWarning, setLoadWarning] = createSignal<string | null>(null);
  // Word-style "Track Changes" toggle: while on, typing is captured as
  // trackInsert marks instead of silently editing the document.
  const [trackChangesOn, setTrackChangesOn] = createSignal(false);

  const [pageSetupOpen, setPageSetupOpen] = createSignal(false);
  const [pageSetup, setPageSetup] = createSignal<PageSetupConfig>(DEFAULT_PAGE_SETUP);
  // Named paragraph styles: built-ins plus user-defined styles persisted on the doc.
  const [customStyles, setCustomStyles] = createSignal<NamedStyleDef[]>(
    Array.isArray((props.initialContent as Record<string, unknown> | null | undefined)?.styles)
      ? customStylesForDoc((props.initialContent as Record<string, unknown>).styles as NamedStyleDef[])
      : [],
  );
  const availableStyles = (): NamedStyleDef[] => [...BUILT_IN_STYLES, ...customStyles()];
  const [activeStyleId, setActiveStyleId] = createSignal<string | null>(null);
  // Footnotes: refs are inline footnote_ref nodes; texts live on the doc.
  const [footnotes, setFootnotes] = createSignal<Footnote[]>(readFootnotes(props.initialContent));
  const [footnoteEditingId, setFootnoteEditingId] = createSignal<string | null>(null);

  const insertFootnote = () => {
    if (!view) return;
    const { state } = view;
    const id = newFootnoteId();
    const nextLabel = footnotes().length + 1;
    const refType = state.schema.nodes.footnote_ref;
    if (!refType) return;
    const node = refType.create({ id, label: nextLabel });
    view.dispatch(
      state.tr.replaceSelectionWith(node).scrollIntoView(),
    );
    setFootnotes((prev) => [...prev, { id, label: nextLabel, text: "" }]);
    setFootnoteEditingId(id);
  };

  const updateFootnoteText = (id: string, text: string) => {
    setFootnotes((prev) => prev.map((note) => (note.id === id ? { ...note, text } : note)));
    if (view) emitDocChangeDebounced(view.state.doc, pageSetup());
  };

  const removeFootnote = (id: string) => {
    if (!view) return;
    const { state } = view;
    let tr = state.tr;
    state.doc.descendants((node, pos) => {
      if (node.type.name === "footnote_ref" && node.attrs.id === id) {
        tr = tr.delete(tr.mapping.map(pos), tr.mapping.map(pos + node.nodeSize));
      }
    });
    view.dispatch(tr);
    setFootnotes((prev) => prev.filter((note) => note.id !== id));
  };

  const applyNamedStyle = (styleId: string) => {
    const style = availableStyles().find((entry) => entry.id === styleId);
    if (!style || !view) return;
    const { state, dispatch } = view;
    const blockAttrs = styleBlockAttrs(style);
    const inlineMarks = styleInlineAttrs(style).map((mark) => state.schema.marks[mark.type].create(mark.attrs));
    const { from, to } = state.selection;
    let tr = state.tr;
    if (style.blockType === "heading") {
      const headingType = state.schema.nodes.heading;
      if (headingType) {
        state.doc.nodesBetween(from, to, (node, pos) => {
          if (node.isBlock && pos >= from - 1 && pos < to) {
            tr = tr.setNodeMarkup(tr.mapping.map(pos), headingType, { ...node.attrs, ...blockAttrs, level: style.headingLevel ?? 1 });
          }
        });
      }
    } else {
      state.doc.nodesBetween(from, to, (node, pos) => {
        if ((node.type.name === "paragraph" || node.type.name === "heading") && pos >= from - 1 && pos < to) {
          tr = tr.setNodeMarkup(tr.mapping.map(pos), state.schema.nodes.paragraph, {
            ...node.attrs,
            ...blockAttrs,
          });
        }
      });
    }
    if (inlineMarks.length && !state.selection.empty) {
      for (const mark of inlineMarks) tr = tr.addMark(from, to, mark);
    }
    dispatch(tr.scrollIntoView());
    setActiveStyleId(styleId);
  };

  const createStyleFromSelection = () => {
    if (!view) return;
    const { state } = view;
    const parent = state.selection.$from.parent;
    const marks = state.storedMarks || state.selection.$from.marks();
    const bold = marks.some((mark) => mark.type.name === "bold");
    const italic = marks.some((mark) => mark.type.name === "italic");
    const colorMark = marks.find((mark) => mark.type.name === "color");
    const sizeMark = marks.find((mark) => mark.type.name === "fontSize");
    const baseName = "Custom style";
    const name = window.prompt("Style name", baseName);
    if (!name) return;
    const normalizedName = normalizeStyleName(name) || baseName;
    const next: NamedStyleDef = {
      id: styleIdFromName(normalizedName),
      name: normalizedName,
      blockType: parent.type.name === "heading" ? "heading" : "paragraph",
      headingLevel: parent.type.name === "heading" ? Number(parent.attrs.level) || 1 : undefined,
      align: (parent.attrs.align as NamedStyleDef["align"]) || "left",
      bold,
      italic,
      color: colorMark?.attrs.color as string | undefined,
      fontSize: sizeMark ? Number(sizeMark.attrs.size) || undefined : undefined,
      builtIn: false,
    };
    setCustomStyles((prev) => [...customStylesForDoc([...prev, next])]);
    setActiveStyleId(next.id);
    showToast(`Style "${next.name}" created`, "success");
  };

  const countPageBreaks = (json: DocContent): number => {
    let count = 0;
    const visit = (node: any) => {
      if (!node || typeof node !== "object") return;
      const breakType = node.attrs?.pageSetup?.breakType;
      if (node.type === "page_break" || (node.type === "section_break" && breakType !== "continuous" && breakType !== "nextColumn")) {
        count += 1;
      }
      if (Array.isArray(node.content)) {
        for (const child of node.content) visit(child);
      }
    };
    visit(json);
    return count;
  };

  const paperDimensions = () => {
    const config = pageSetup();
    let w = 816;
    let h = 1056;
    switch (config.paperSize) {
      case "a4": w = 794; h = 1123; break;
      case "legal": w = 816; h = 1344; break;
      case "executive": w = 696; h = 1008; break;
      case "letter":
      default: w = 816; h = 1056; break;
    }
    if (config.orientation === "landscape") {
      return { width: h, height: w };
    }
    return { width: w, height: h };
  };

  const refreshHeadings = (doc: EditorState["doc"]) => {
    const newHeadings: { level: number; text: string; pos: number }[] = [];
    doc.descendants((node, pos) => {
      if (node.type.name === "heading") {
        newHeadings.push({
          level: node.attrs.level,
          text: node.textContent,
          pos,
        });
      }
    });
    setHeadings(newHeadings);
  };

  let lastWords = 0;
  let lastChars = 0;
  let wordCountTimeout: number | undefined;
  let currentCurPage = 1;
  let currentPages = 1;
  let fieldRefreshScheduled = false;
  let pendingFieldPages = 1;
  let pendingFieldPageHeight = 1;
  let documentHasFields = false;

  const scanDocumentForFields = (doc: Node) => {
    let found = false;
    doc.descendants((node) => {
      if (node.type.name === "field") found = true;
      return !found;
    });
    return found;
  };

  const applyDocumentFieldResults = (pages: number, contentPageHeight: number) => {
    if (!view || !editorRef) return;
    const bounds = editorRef.getBoundingClientRect();
    let transaction = view.state.tr;
    let found = false;
    view.state.doc.descendants((node, pos) => {
      if (node.type.name !== "field") return;
      found = true;
      const kind = normalizeDocumentFieldKind(node.attrs.kind);
      if (!kind) return;
      let fieldPage = currentCurPage;
      if (kind === "page") {
        try {
          const coords = view!.coordsAtPos(pos);
          const relativeY = coords.top - bounds.top + editorRef.scrollTop;
          fieldPage = Math.max(1, Math.min(pages, Math.floor(relativeY / contentPageHeight) + 1));
        } catch {
          fieldPage = currentCurPage;
        }
      }
      const result = documentFieldResult(kind, pages, fieldPage);
      if (String(node.attrs.result ?? "") !== result) {
        transaction = transaction.setNodeMarkup(pos, node.type, { ...node.attrs, result }, node.marks);
      }
    });
    documentHasFields = found;
    if (!transaction.docChanged) return;
    view.dispatch(transaction);
  };

  const queueDocumentFieldRefresh = (pages: number, contentPageHeight: number) => {
    pendingFieldPages = pages;
    pendingFieldPageHeight = contentPageHeight;
    if (fieldRefreshScheduled) return;
    fieldRefreshScheduled = true;
    window.setTimeout(() => {
      fieldRefreshScheduled = false;
      applyDocumentFieldResults(pendingFieldPages, pendingFieldPageHeight);
    }, 0);
  };

  const refreshStatus = (json: DocContent, docChanged = true) => {
    const paper = paperDimensions();
    const config = pageSetup();
    const topMarginPx = (config.margins.top || 1) * 96;
    const bottomMarginPx = (config.margins.bottom || 1) * 96;
    const contentPageHeight = Math.max(300, paper.height - topMarginPx - bottomMarginPx);

    const domHeight = editorRef ? editorRef.scrollHeight : 0;
    const estimatedPages = Math.max(1, Math.ceil(domHeight / contentPageHeight));
    const explicitPageBreaks = countPageBreaks(json);
    const pages = Math.max(1, explicitPageBreaks + 1, estimatedPages);
    setPageCount(pages);

    let curPage = 1;
    if (view && editorRef) {
      const { from } = view.state.selection;
      const coords = view.coordsAtPos(from);
      const editorBounds = editorRef.getBoundingClientRect();
      const relativeY = coords.top - editorBounds.top + editorRef.scrollTop;
      curPage = Math.max(1, Math.min(pages, Math.floor(relativeY / contentPageHeight) + 1));
    }

    currentCurPage = curPage;
    currentPages = pages;
    if (documentHasFields) queueDocumentFieldRefresh(pages, contentPageHeight);

    const updateLabel = (w: number, c: number) => {
      props.onWordCountChange?.(
        `Page ${currentCurPage} of ${currentPages} · ${w} words, ${c} characters`
      );
    };

    if (!docChanged) {
      updateLabel(lastWords, lastChars);
      return;
    }

    if (wordCountTimeout) window.clearTimeout(wordCountTimeout);
    wordCountTimeout = window.setTimeout(() => {
      commands.computeDocWordCount(json).then((wc) => {
        lastWords = wc.words;
        lastChars = wc.characters;
        updateLabel(lastWords, lastChars);
      });
    }, 2000);
  };

  const compareSummary = () => {
    trackedChangeRevision();
    return compareDocContent(
      compareBaseline(),
      view ? (view.state.doc.toJSON() as DocContent) : null,
    );
  };

  const comparePreview = (text: string) => {
    const limit = 16_384;
    return text.length > limit ? `${text.slice(0, limit)}\n…` : text;
  };

  const resetCompareBaseline = () => {
    if (!view) return;
    setCompareBaseline(view.state.doc.toJSON() as DocContent);
    setCompareBaselineLabel("Since opened");
    setCompareSourceId("");
    setTrackedChangeRevision((revision) => revision + 1);
  };

  const compareWithDocument = (sourceId: string) => {
    if (!sourceId || !view) return;
    const source = findCompareSource(props.compareDocuments, sourceId);
    if (!source?.content) return;
    setCompareSourceId(sourceId);
    setCompareBaseline(source.content);
    setCompareBaselineLabel(`Compared with ${source.title || "open document"}`);
    setTrackedChangeRevision((revision) => revision + 1);
  };

  const emitDocChange = (doc: Node, setup: PageSetupConfig) => {
    if (!props.onChange) return;
    const payload = buildDocJson(doc, setup, comments(), customStyles(), footnotes());
    const serialized = JSON.stringify(payload);
    if (serialized === lastEmittedJson) return;
    lastEmittedJson = serialized;
    props.onChange(payload);
  };

  // Trailing debounce for keystroke-driven emits: serializing the whole doc
  // on every character is O(doc) per key; 300ms batching keeps typing smooth
  // on large documents while autosave (2s) stays far behind the final emit.
  let emitTimer: ReturnType<typeof setTimeout> | null = null;
  const emitDocChangeDebounced = (doc: Node, setup: PageSetupConfig) => {
    if (!props.onChange) return;
    if (emitTimer) clearTimeout(emitTimer);
    emitTimer = setTimeout(() => {
      emitTimer = null;
      emitDocChange(view?.state.doc ?? doc, pageSetup());
    }, 300);
  };
  onCleanup(() => {
    if (emitTimer) {
      clearTimeout(emitTimer);
      // Flush the pending emit so the shell's autosave sees the final doc.
      emitDocChange(view?.state.doc ?? mySchema.node("doc"), pageSetup());
    }
  });

  const refreshCommentDecorations = () => {
    withView((v) => v.dispatch(v.state.tr.setMeta("comments-refresh", true)));
  };

  const openCommentDialog = () => {
    withView((v) => {
      const { from, to } = v.state.selection;
      if (from === to) return;
      setCommentAnchor({ from, to });
      setCommentDraft("");
      setCommentDialogOpen(true);
    });
  };

  const saveComment = () => {
    const text = commentDraft().trim();
    const anchor = commentAnchor();
    if (!text || !anchor || !view) return;
    const comment: ReviewComment = {
      id: `comment-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
      author: "You",
      text,
      from: anchor.from,
      to: anchor.to,
      resolved: false,
      createdAt: new Date().toISOString(),
    };
    setComments((previous) => [...previous, comment]);
    setSelectedCommentId(comment.id);
    setCommentDialogOpen(false);
    setCommentAnchor(null);
    refreshCommentDecorations();
    emitDocChange(view.state.doc, pageSetup());
  };

  const updateComment = (id: string, update: Partial<ReviewComment>) => {
    setComments((previous) => previous.map((comment) => (comment.id === id ? { ...comment, ...update } : comment)));
    refreshCommentDecorations();
    if (view) emitDocChange(view.state.doc, pageSetup());
  };

  // Reply drafts are transient UI state; they are not part of the document.
  const [replyDrafts, setReplyDrafts] = createSignal<Record<string, string>>({});
  const submitReply = (commentId: string) => {
    const text = (replyDrafts()[commentId] ?? "").trim();
    if (!text) return;
    setComments((previous) =>
      previous.map((comment) =>
        comment.id === commentId
          ? {
              ...comment,
              replies: [
                ...(comment.replies || []),
                {
                  id: `reply-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
                  author: "You",
                  text,
                  createdAt: new Date().toISOString(),
                },
              ],
            }
          : comment,
      ),
    );
    setReplyDrafts((prev) => ({ ...prev, [commentId]: "" }));
    refreshCommentDecorations();
    if (view) emitDocChange(view.state.doc, pageSetup());
  };

  const removeComment = (id: string) => {
    setComments((previous) => previous.filter((comment) => comment.id !== id));
    if (selectedCommentId() === id) setSelectedCommentId(null);
    refreshCommentDecorations();
    if (view) emitDocChange(view.state.doc, pageSetup());
  };

  const selectComment = (comment: ReviewComment) => {
    setSelectedCommentId(comment.id);
    withView((v) => {
      if (comment.from < 0 || comment.to > v.state.doc.content.size || comment.from >= comment.to) return;
      v.dispatch(v.state.tr.setSelection(TextSelection.create(v.state.doc, comment.from, comment.to)).scrollIntoView());
    });
  };

  onMount(() => {
    const initial = props.initialContent;
    if (initial?.pageSetup && typeof initial.pageSetup === "object") {
      setPageSetup(normalizePageSetupConfig(initial.pageSetup));
    }

    let docNode;
    let loadWarning: string | null = null;
    try {
      const docJson = { ...(initial || {}) } as DocContent;
      delete docJson.pageSetup;
      delete docJson.comments;
      delete docJson.footnotes;
      delete (docJson as Record<string, unknown>).styles;
      docNode = mySchema.nodeFromJSON(
        docJson.type ? docJson : { type: "doc", content: [{ type: "paragraph", content: [] }] }
      );
    } catch (err) {
      // Never silently replace an unreadable document with an empty one: keep
      // the raw text content so the user can recover instead of losing data.
      console.error("DocEditor: failed to parse initial content", err);
      loadWarning = "This document contained parts the editor could not read. " +
        "Unknown elements were kept out of the editable view; save a copy before continuing.";
      const fallbackText = extractPlainText(initial);
      docNode = mySchema.node("doc", null, fallbackText.length
        ? fallbackText.split(/\n{2,}/).map((para) =>
            mySchema.node("paragraph", null, para.trim()
              ? [mySchema.text(para.trim())]
              : [])
          )
        : [mySchema.node("paragraph")]);
    }
    if (loadWarning) setLoadWarning(loadWarning);
    setCompareBaseline(docNode.toJSON() as DocContent);

    const state = EditorState.create({
      doc: docNode,
      plugins: [
        history(),
        columnResizing({ handleWidth: 5, cellMinWidth: 48 }),
        tableEditing(),
        findPlugin(findQuery, matchCase, matchIndex),
        autoTrackChangesPlugin(trackChangesOn, () =>
          mySchema.marks[TRACK_INSERT_MARK].create({
            author: "You",
            changeId: `change-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
            createdAt: new Date().toISOString(),
          }),
        ),
        reviewCommentPlugin(comments),
        new Plugin({
          props: {
            transformPastedHTML: (html) => transformPastedHTML(html),
            transformPasted: (slice) => prunePastedSlice(slice, mySchema),
          },
        }),
        inputRules({ rules: [
          ...smartQuotes, emDash, ellipsis,
          wrappingInputRule(/^-\s$/, mySchema.nodes.bullet_list),
          wrappingInputRule(/^(\d+)\.\s$/, mySchema.nodes.ordered_list, (match) => ({ order: +match[1] }), (match, node) => node.childCount + node.attrs.order === +match[1]),
        ] }),
        keymap({
          "Tab": (state, dispatch) => {
            const { $from } = state.selection;
            if ($from.parent.type.name === "list_item") {
              return sinkListItem(mySchema.nodes.list_item)(state, dispatch);
            }
            if (dispatch) {
              dispatch(state.tr.insertText("\t"));
              return true;
            }
            return false;
          },
          "Shift-Tab": liftListItem(mySchema.nodes.list_item),
          "Mod-b": toggleMark(mySchema.marks.bold),
          "Mod-i": toggleMark(mySchema.marks.italic),
          "Mod-u": toggleMark(mySchema.marks.underline),
          "Mod-.": toggleMark(mySchema.marks.superscript),
          "Mod-,": toggleMark(mySchema.marks.subscript),
          "Mod-]": (state, dispatch) => {
            const mark = state.selection.$from.marks().find(m => m.type.name === "fontSize");
            const current = mark ? Number(mark.attrs.size) : 12;
            const FONT_SIZES = [8, 9, 10, 11, 12, 14, 16, 18, 20, 24, 28, 36, 48, 72];
            const next = FONT_SIZES.find(s => s > current) || FONT_SIZES[FONT_SIZES.length - 1];
            if (dispatch) {
              setFontSize(String(next));
              const { from, to } = state.selection;
              if (from === to) {
                dispatch(state.tr.addStoredMark(mySchema.marks.fontSize.create({ size: String(next) })));
              } else {
                toggleMark(mySchema.marks.fontSize, { size: String(next) })(state, dispatch);
              }
            }
            return true;
          },
          "Mod-[": (state, dispatch) => {
            const mark = state.selection.$from.marks().find(m => m.type.name === "fontSize");
            const current = mark ? Number(mark.attrs.size) : 12;
            const FONT_SIZES = [8, 9, 10, 11, 12, 14, 16, 18, 20, 24, 28, 36, 48, 72];
            const next = [...FONT_SIZES].reverse().find(s => s < current) || FONT_SIZES[0];
            if (dispatch) {
              setFontSize(String(next));
              const { from, to } = state.selection;
              if (from === to) {
                dispatch(state.tr.addStoredMark(mySchema.marks.fontSize.create({ size: String(next) })));
              } else {
                toggleMark(mySchema.marks.fontSize, { size: String(next) })(state, dispatch);
              }
            }
            return true;
          },
          "Mod-Enter": (state, dispatch) => {
            if (!dispatch) return false;
            const node = mySchema.nodes.page_break.create();
            dispatch(state.tr.replaceSelectionWith(node).scrollIntoView());
            return true;
          },
          "Mod-f": () => {
            setFindOpen(true);
            return true;
          },
          "Mod-z": undo,
          "Mod-y": redo,
          "Mod-Shift-z": redo,
        }),
        keymap(baseKeymap),
      ],
    });

    view = new EditorView(editorRef, {
      state,
      dispatchTransaction(transaction) {
        const newState = view!.state.apply(transaction);
        view!.updateState(newState);

        const parent = newState.selection.$from.parent;
        let bType = "paragraph";
        if (parent.type.name === "heading") {
          bType = `heading${parent.attrs.level || 1}`;
        } else if (parent.type.name === "code_block") {
          bType = "code_block";
        } else if (parent.type.name === "blockquote") {
          bType = "blockquote";
        } else if (parent.type.name === "paragraph") {
          bType = "paragraph";
        }
        setCurrentBlockType(bType);

        if (parent.attrs && parent.attrs.lineHeight !== undefined) {
          setCurrentLineSpacing(String(parent.attrs.lineHeight));
        }
        if (parent.attrs?.spacingBefore !== undefined) {
          setSpacingBefore(Number(parent.attrs.spacingBefore) || 0);
        }
        if (parent.attrs?.spacingAfter !== undefined) {
          setSpacingAfter(Number(parent.attrs.spacingAfter) || 0);
        }

        if (transaction.selectionSet || transaction.docChanged) {
          syncToolbarFromSelection(newState, setFontFamily, setFontSize, setTextColor, setHighlightColor);
        }

        if (transaction.docChanged && comments().length > 0) {
          setComments((previous) => mapReviewCommentAnchors(
            previous,
            (position) => transaction.mapping.map(position, 1),
            (position) => transaction.mapping.map(position, -1),
          ));
        }

        const json = buildDocJson(newState.doc, pageSetup(), comments(), customStyles(), footnotes());
        refreshStatus(json, transaction.docChanged);
        // Keep footnote labels in sync with reference order after edits.
        if (transaction.docChanged) {
          const synced = syncFootnotesWithRefs(footnotes(), newState.doc.toJSON() as DocContent);
          if (JSON.stringify(synced) !== JSON.stringify(footnotes())) {
            setFootnotes(synced);
          }
        }
        // Track the active paragraph's named style for the Styles sidebar.
        const activeParent = newState.selection.$from.parent;
        setActiveStyleId(
          activeParent.attrs?.styleName
            ? String(activeParent.attrs.styleName)
            : activeParent.type.name === "heading"
              ? `Heading${activeParent.attrs.level || 1}`
              : null,
        );

        if (transaction.docChanged) {
          setTrackedChangeRevision((revision) => revision + 1);
          refreshHeadings(newState.doc);
          emitDocChangeDebounced(newState.doc, pageSetup());
        }

        if (view!.hasFocus() && !newState.selection.empty) {
          const { from, to } = newState.selection;
          const pState = painterState();
          if ((pState === "armed" || pState === "locked") && transaction.selectionSet && from !== to) {
            let tr = newState.tr;
            const sAttrs = storedAttrs();

            newState.doc.nodesBetween(from, to, (node, pos, parent) => {
              if (node.isText && parent) {
                const nodeFrom = Math.max(from, pos);
                const nodeTo = Math.min(to, pos + node.nodeSize);
                tr = tr.removeMark(nodeFrom, nodeTo);
                for (const mark of storedMarks()) {
                  if (parent.type.allowsMarkType(mark.type)) {
                    tr = tr.addMark(nodeFrom, nodeTo, mark);
                  }
                }
              } else if (node.isBlock && (node.type.name === "paragraph" || node.type.name === "heading")) {
                if (Object.keys(sAttrs).length > 0) {
                  tr = tr.setNodeMarkup(pos, undefined, { ...node.attrs, ...sAttrs });
                }
              }
            });

            if (pState === "armed") setPainterState("idle");
            if (tr.docChanged) {
              setTimeout(() => {
                if (view) view.dispatch(tr);
              }, 0);
            }
          }
        }

        if (view!.hasFocus() && !newState.selection.empty) {
          const { from } = newState.selection;
          const coords = view!.coordsAtPos(from);
          setBubbleVisible(true);
          setBubbleTop(coords.top);
          setBubbleLeft(coords.left);
        } else {
          setBubbleVisible(false);
        }

        const sel = newState.selection;
        if (sel instanceof NodeSelection && sel.node.type.name === "image") {
          setSelectedImage({
            pos: sel.from,
            width: Number(sel.node.attrs.width) || 320,
            height: Number(sel.node.attrs.height) || 240,
          });
        } else {
          setSelectedImage(null);
        }
      },
      handlePaste(_view, event) {
        const items = event.clipboardData?.items;
        if (!items) return false;
        for (const item of Array.from(items)) {
          if (item.type.startsWith("image/")) {
            event.preventDefault();
            const file = item.getAsFile();
            if (!file) return true;
            const reader = new FileReader();
            reader.onload = () => {
              const src = String(reader.result || "");
              if (!src || !view) return;
              view.dispatch(
                view.state.tr.replaceSelectionWith(mySchema.nodes.image.create({ src, alt: file.name }))
              );
            };
            reader.readAsDataURL(file);
            return true;
          }
        }
        return false;
      },
      handleClick(clickedView, _position, event) {
        const target = event.target instanceof Element ? event.target.closest("a") : null;
        const href = target?.getAttribute("href");
        const normalized = href ? normalizeDocLink(href) : null;
        if (!normalized?.startsWith("internal:")) return false;
        const bookmarkPosition = findBookmarkPosition(clickedView.state.doc, normalized.slice("internal:".length));
        if (bookmarkPosition == null) return false;
        clickedView.dispatch(
          clickedView.state.tr
            .setSelection(TextSelection.create(clickedView.state.doc, bookmarkPosition))
            .scrollIntoView(),
        );
        return true;
      },
    });

    documentHasFields = scanDocumentForFields(view.state.doc);
    refreshStatus(buildDocJson(view.state.doc, pageSetup(), comments(), customStyles(), footnotes()));
    refreshHeadings(view.state.doc);
    syncToolbarFromSelection(view.state, setFontFamily, setFontSize, setTextColor, setHighlightColor);
  });

  createEffect(() => {
    const content = props.initialContent;
    if (!view || !content) return;
    if (JSON.stringify(content) === lastEmittedJson) return;
    try {
      if (content.pageSetup && typeof content.pageSetup === "object") {
        setPageSetup(normalizePageSetupConfig(content.pageSetup));
      }
      const docJson = { ...content } as DocContent;
      delete docJson.pageSetup;
      delete docJson.footnotes;
      delete (docJson as Record<string, unknown>).styles;
      const nextDoc = mySchema.nodeFromJSON(docJson);
      if (docJsonForCompare(content) !== docJsonForCompare(view.state.doc.toJSON() as DocContent)) {
        documentHasFields = scanDocumentForFields(nextDoc);
        const tr = view.state.tr.replaceWith(0, view.state.doc.content.size, nextDoc.content);
        if (tr.docChanged) {
          view.dispatch(tr);
          refreshStatus(buildDocJson(view.state.doc, pageSetup(), comments(), customStyles(), footnotes()));
          refreshHeadings(view.state.doc);
        }
      }
    } catch {
      /* ignore invalid external content */
    }
  });

  onCleanup(() => {
    if (view) view.destroy();
  });

  const resizeSelectedImage = (width: number, height: number) => {
    const img = selectedImage();
    if (!view || !img) return;
    const node = view.state.doc.nodeAt(img.pos);
    if (!node || node.type.name !== "image") return;
    view.dispatch(
      view.state.tr.setNodeMarkup(img.pos, undefined, {
        ...node.attrs,
        width: Math.max(40, width),
        height: Math.max(40, height),
      }),
    );
    setSelectedImage({ pos: img.pos, width: Math.max(40, width), height: Math.max(40, height) });
  };

  const insertImageFromFile = (file: File, pos?: number) => {
    if (!view) return;
    const reader = new FileReader();
    reader.onload = () => {
      const src = String(reader.result || "");
      if (!src) return;
      const node = mySchema.nodes.image.create({ src, alt: file.name, width: 320, height: 240 });
      const tr = pos != null
        ? view!.state.tr.insert(pos, node)
        : view!.state.tr.replaceSelectionWith(node);
      view!.dispatch(tr);
    };
    reader.readAsDataURL(file);
  };

  const withView = (fn: (v: EditorView) => void) => {
    if (view) fn(view);
  };

  const trackedStats = () => {
    trackedChangeRevision();
    return view
      ? getTrackedChangeStats(view.state.doc.toJSON())
      : { insertions: 0, deletions: 0, insertedCharacters: 0, deletedCharacters: 0 };
  };

  const trackedChangeSummaries = () => {
    trackedChangeRevision();
    return view ? getTrackedChangeSummaries(view.state.doc.toJSON()) : [];
  };

  const markSelectionAsChange = (kind: TrackedChangeKind) => {
    withView((v) => {
      const { from, to } = v.state.selection;
      if (from === to) return;
      const markName = kind === "insert" ? TRACK_INSERT_MARK : TRACK_DELETE_MARK;
      const oppositeName = kind === "insert" ? TRACK_DELETE_MARK : TRACK_INSERT_MARK;
      const mark = mySchema.marks[markName].create({
        author: "You",
        changeId: `change-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
        createdAt: new Date().toISOString(),
      });
      const tr = v.state.tr
        .removeMark(from, to, mySchema.marks[oppositeName])
        .addMark(from, to, mark)
        .scrollIntoView();
      v.dispatch(tr);
    });
  };

  const resolveAllTrackedChanges = (decision: TrackedChangeDecision) => {
    withView((v) => {
      const resolved = resolveTrackedChanges(v.state.doc.toJSON(), decision);
      let nextDoc: Node;
      try {
        nextDoc = mySchema.nodeFromJSON(resolved);
      } catch {
        return;
      }
      const tr = v.state.tr.replaceWith(0, v.state.doc.content.size, nextDoc.content).scrollIntoView();
      if (tr.docChanged) v.dispatch(tr);
    });
  };

  const resolveOneTrackedChange = (changeId: string, decision: TrackedChangeDecision) => {
    withView((v) => {
      const resolved = resolveTrackedChange(v.state.doc.toJSON(), changeId, decision);
      let nextDoc: Node;
      try {
        nextDoc = mySchema.nodeFromJSON(resolved);
      } catch {
        return;
      }
      const tr = v.state.tr.replaceWith(0, v.state.doc.content.size, nextDoc.content).scrollIntoView();
      if (tr.docChanged) v.dispatch(tr);
    });
  };

  const execToggleBold = () => withView((v) => toggleMark(mySchema.marks.bold)(v.state, v.dispatch));
  const execToggleItalic = () => withView((v) => toggleMark(mySchema.marks.italic)(v.state, v.dispatch));
  const execToggleUnderline = () =>
    withView((v) => toggleMark(mySchema.marks.underline)(v.state, v.dispatch));
  const execToggleStrike = () => withView((v) => toggleMark(mySchema.marks.strike)(v.state, v.dispatch));
  const execToggleSuper = () =>
    withView((v) => toggleMark(mySchema.marks.superscript)(v.state, v.dispatch));
  const execToggleSub = () =>
    withView((v) => toggleMark(mySchema.marks.subscript)(v.state, v.dispatch));

  const execBlockType = (type: string) => {
    withView((v) => {
      if (type.startsWith("heading")) {
        const level = Number(type.replace("heading", "")) || 1;
        setBlockType(mySchema.nodes.heading, { level })(v.state, v.dispatch);
        return;
      }
      const nodeType = mySchema.nodes[type];
      if (nodeType) setBlockType(nodeType)(v.state, v.dispatch);
    });
  };

  const execList = (type: "bullet_list" | "ordered_list") => {
    withView((v) => wrapInList(mySchema.nodes[type])(v.state, v.dispatch));
  };

  const execColor = (markName: "color" | "highlight", color: string) => {
    withView((v) => toggleMark(mySchema.marks[markName], { color })(v.state, v.dispatch));
  };

  const applyFont = (family: string) => {
    setFontFamily(family);
    withView((v) => {
      const { from, to } = v.state.selection;
      if (from === to) {
        // stored mark for next typing
        v.dispatch(v.state.tr.addStoredMark(mySchema.marks.fontFamily.create({ family })));
        return;
      }
      toggleMark(mySchema.marks.fontFamily, { family })(v.state, v.dispatch);
    });
  };

  const applyFontSize = (size: string) => {
    setFontSize(size);
    withView((v) => {
      const { from, to } = v.state.selection;
      if (from === to) {
        v.dispatch(v.state.tr.addStoredMark(mySchema.marks.fontSize.create({ size })));
        return;
      }
      toggleMark(mySchema.marks.fontSize, { size })(v.state, v.dispatch);
    });
  };

  const FONT_SIZES = [8, 9, 10, 11, 12, 14, 16, 18, 20, 24, 28, 36, 48, 72];

  const execGrowFontSize = () => {
    withView((v) => {
      const mark = v.state.selection.$from.marks().find(m => m.type.name === "fontSize");
      const current = mark ? Number(mark.attrs.size) : 12;
      const next = FONT_SIZES.find(s => s > current) || FONT_SIZES[FONT_SIZES.length - 1];
      applyFontSize(String(next));
    });
  };

  const execShrinkFontSize = () => {
    withView((v) => {
      const mark = v.state.selection.$from.marks().find(m => m.type.name === "fontSize");
      const current = mark ? Number(mark.attrs.size) : 12;
      const next = [...FONT_SIZES].reverse().find(s => s < current) || FONT_SIZES[0];
      applyFontSize(String(next));
    });
  };

  const armFormatPainter = (sticky = false) => {
    withView((v) => {
      const { $from } = v.state.selection;
      setStoredMarks([...($from.marks() || [])]);
      let attrs = {};
      for (let d = $from.depth; d > 0; d--) {
        const node = $from.node(d);
        if (node.type.name === "paragraph" || node.type.name === "heading") {
          attrs = { ...node.attrs };
          break;
        }
      }
      setStoredAttrs(attrs);
      setPainterState(sticky ? "locked" : "armed");
    });
  };

  const insertPageBreak = () => {
    withView((v) => {
      v.dispatch(v.state.tr.replaceSelectionWith(mySchema.nodes.page_break.create()).scrollIntoView());
    });
  };

  const insertDocumentField = (kind: DocumentFieldKind) => {
    withView((v) => {
      const fieldType = mySchema.nodes.field;
      const normalized = normalizeDocumentFieldKind(kind);
      if (!fieldType || !normalized) return;
      documentHasFields = true;
      v.dispatch(v.state.tr.replaceSelectionWith(fieldType.create({ kind: normalized })).scrollIntoView());
    });
  };

  const insertSectionBreak = () => {
    withView((v) => {
      const setup = normalizePageSetupConfig(pageSetup());
      v.dispatch(
        v.state.tr
          .replaceSelectionWith(mySchema.nodes.section_break.create({ pageSetup: setup }))
          .scrollIntoView(),
      );
    });
  };

  const openLinkDialog = () => {
    setLinkError("");
    setLinkDialogOpen(true);
  };

  const applyLink = () => {
    const href = normalizeDocLink(linkHref());
    if (!href) {
      setLinkError("Use an https/http, mailto, tel, or internal bookmark link.");
      return;
    }
    withView((v) => toggleMark(mySchema.marks.link, { href, title: null })(v.state, v.dispatch));
    setLinkDialogOpen(false);
    setLinkError("");
  };

  const openBookmarkDialog = () => {
    setBookmarkName("");
    setBookmarkError("");
    withView((v) => {
      if (v.state.selection.empty) {
        setBookmarkError("Select the text that should be the bookmark target first.");
      }
      setBookmarkDialogOpen(true);
    });
  };

  const applyBookmark = () => {
    const name = normalizeBookmarkName(bookmarkName());
    if (!name) {
      setBookmarkError("Enter a bookmark name using letters, numbers, or underscores.");
      return;
    }
    withView((v) => {
      const { from, to } = v.state.selection;
      if (from === to) {
        setBookmarkError("Select the text that should be the bookmark target first.");
        return;
      }
      const existing = bookmarkNamesInDocument(v.state.doc);
      if (existing.has(name.toLowerCase())) {
        setBookmarkError(`A bookmark named “${name}” already exists in this document.`);
        return;
      }
      const mark = mySchema.marks.bookmark.create({ name, id: null });
      v.dispatch(v.state.tr.addMark(from, to, mark).scrollIntoView());
      setBookmarkName(name);
      setBookmarkError("");
      setBookmarkDialogOpen(false);
    });
  };

  const applyImage = () => {
    const src = imageSrc().trim();
    if (!src) return;
    withView((v) => {
      v.dispatch(v.state.tr.replaceSelectionWith(mySchema.nodes.image.create({ src, alt: "" })));
    });
    setImageDialogOpen(false);
    setImageSrc("");
  };

  const insertTable = (rows = 3, cols = 3) => {
    withView((v) => {
      const cellType = mySchema.nodes.table_cell;
      const headerType = mySchema.nodes.table_header;
      const rowType = mySchema.nodes.table_row;
      const tableType = mySchema.nodes.table;
      if (!cellType || !headerType || !rowType || !tableType) return;
      const built = Array.from({ length: rows }, (_, r) =>
        rowType.create(
          null,
          Array.from({ length: cols }, () =>
            (r === 0 ? headerType : cellType).createAndFill()!
          )
        )
      );
      v.dispatch(v.state.tr.replaceSelectionWith(tableType.create(null, built)));
    });
  };

  /**
   * Insert a table of contents built from the document's headings.
   * Each heading text gets a bookmark mark; the TOC entries are real
   * paragraphs with internal `internal:` links, so DOCX/PDF/print export
   * the TOC through the existing paragraph/link paths.
   */
  const insertTableOfContents = () => {
    withView((v) => {
      // Collect heading nodes with their text ranges first.
      const headingRanges: Array<{ from: number; to: number; level: number; text: string }> = [];
      v.state.doc.descendants((node, pos) => {
        if (node.type.name === "heading") {
          headingRanges.push({
            from: pos + 1,
            to: pos + node.nodeSize - 1,
            level: Number(node.attrs.level) || 1,
            text: node.textContent,
          });
        }
      });
      const usable = headingRanges.filter((range) => range.text.trim());
      if (!usable.length) return;

      const entries: TocEntry[] = [];
      const used = new Set<string>();
      let tr = v.state.tr;
      // Attach bookmark marks from the end so earlier positions stay valid.
      for (let i = usable.length - 1; i >= 0; i -= 1) {
        const range = usable[i];
        const anchor = sanitizeTocAnchor(range.text, used);
        const mark = mySchema.marks.bookmark.create({ name: anchor, id: null });
        tr = tr.addMark(range.from, range.to, mark);
        entries.unshift({ level: range.level, text: range.text.trim().slice(0, 120), anchor });
      }

      const paragraphType = mySchema.nodes.paragraph;
      const linkMarkType = mySchema.marks.link;
      if (!paragraphType || !linkMarkType) return;
      const tocNodes = buildTocContent(entries).map((entryJson) => {
        const linkMark = linkMarkType.create({ href: `internal:${entryJson.anchor}` });
        const textNode = mySchema.text(
          (entryJson.content![0] as { text: string }).text,
          [linkMark],
        );
        return paragraphType.create(entryJson.attrs as Record<string, never>, [textNode]);
      });
      tr = tr.replaceSelectionWith(tocNodes[0]);
      for (let i = 1; i < tocNodes.length; i += 1) {
        tr = tr.insert(tr.selection.from, tocNodes[i]);
      }
      v.dispatch(tr.scrollIntoView());
    });
  };

  const findTableContext = (v: EditorView): TableCommandState => {
    const $from = v.state.selection.$from;
    let cellPos: number | null = null;
    let rowPos: number | null = null;
    let tablePos: number | null = null;
    for (let d = $from.depth; d > 0; d--) {
      const node = $from.node(d);
      if (node.type.name === "table_cell" && cellPos == null) cellPos = $from.before(d);
      if (node.type.name === "table_row" && rowPos == null) rowPos = $from.before(d);
      if (node.type.name === "table" && tablePos == null) tablePos = $from.before(d);
    }
    return { cellPos, rowPos, tablePos };
  };

  const addTableRow = () => {
    withView((v) => addRowAfter(v.state, v.dispatch));
  };

  const deleteTableRow = () => {
    withView((v) => deleteRow(v.state, v.dispatch));
  };

  const addTableColumn = () => {
    withView((v) => addColumnAfter(v.state, v.dispatch));
  };

  const deleteTableColumn = () => {
    withView((v) => deleteColumn(v.state, v.dispatch));
  };

  const toggleHeaderRow = () => {
    withView((v) => pmToggleHeaderRow(v.state, v.dispatch));
  };

  const execDeleteTable = () => {
    withView((v) => deleteTable(v.state, v.dispatch));
  };

  const execMergeCells = () => withView((v) => mergeCells(v.state, v.dispatch));
  const execSplitCell = () => withView((v) => splitCell(v.state, v.dispatch));

  const execBlockAttrs = (attrs: Record<string, string | number>) => {
    withView((v) => {
      const { from, to } = v.state.selection;
      let transaction = v.state.tr;
      v.state.doc.nodesBetween(from, to, (node, pos) => {
        if (node.type.name !== "paragraph" && node.type.name !== "heading") return;
        transaction = transaction.setNodeMarkup(pos, undefined, { ...node.attrs, ...attrs });
      });
      if (transaction.docChanged) v.dispatch(transaction);
    });
  };

  const adjustIndent = (delta: number) => {
    withView((v) => {
      const { from, to } = v.state.selection;
      let transaction = v.state.tr;
      v.state.doc.nodesBetween(from, to, (node, pos) => {
        if (node.type.name !== "paragraph" && node.type.name !== "heading") return;
        const next = Math.max(0, Math.min(10, Number(node.attrs.indent || 0) + delta));
        transaction = transaction.setNodeMarkup(pos, undefined, { ...node.attrs, indent: next });
      });
      if (transaction.docChanged) v.dispatch(transaction);
    });
  };

  const clearFormatting = () => {
    withView((v) => {
      const { from, to } = v.state.selection;
      let tr = v.state.tr;
      if (from !== to) tr = tr.removeMark(from, to);
      v.state.doc.nodesBetween(from, to, (node, pos) => {
        if (node.type.name === "paragraph" || node.type.name === "heading") {
          tr = tr.setNodeMarkup(pos, undefined, {
            align: "left",
            indent: 0,
            lineHeight: 1.5,
            spacingBefore: 0,
            spacingAfter: 0,
            ...(node.type.name === "heading" ? { level: node.attrs.level } : {}),
          });
        }
      });
      if (tr.docChanged) v.dispatch(tr);
    });
  };

  const execUndo = () => withView((v) => undo(v.state, v.dispatch));
  const execRedo = () => withView((v) => redo(v.state, v.dispatch));

  onMount(() => {
    const onCommand = (event: Event) => {
      const detail = (event as CustomEvent<EditorCommandDetail>).detail;
      if (!detail?.id) return;
      switch (detail.id) {
        case "find":
        case "find-replace":
          setFindOpen(true);
          break;
        case "undo":
          execUndo();
          break;
        case "redo":
          execRedo();
          break;
        case "cut":
          document.execCommand("cut");
          break;
        case "copy":
          document.execCommand("copy");
          break;
        case "paste":
          document.execCommand("paste");
          break;
        case "bold":
          execToggleBold();
          break;
        case "italic":
          execToggleItalic();
          break;
        case "underline":
          execToggleUnderline();
          break;
        case "clear-formatting":
          clearFormatting();
          break;
        case "insert-table":
          insertTable();
          break;
        case "insert-image":
          setImageDialogOpen(true);
          break;
        case "insert-link":
          openLinkDialog();
          break;
        case "insert-bookmark":
          openBookmarkDialog();
          break;
        case "add-comment":
          openCommentDialog();
          break;
        case "mark-insertion":
          markSelectionAsChange("insert");
          break;
        case "mark-deletion":
          markSelectionAsChange("delete");
          break;
        case "accept-all-changes":
          resolveAllTrackedChanges("accept");
          break;
        case "reject-all-changes":
          resolveAllTrackedChanges("reject");
          break;
        case "insert-page-break":
          insertPageBreak();
          break;
        case "insert-page-field":
          insertDocumentField("page");
          break;
        case "insert-num-pages-field":
          insertDocumentField("numPages");
          break;
        case "insert-section-break":
          insertSectionBreak();
          break;
        case "insert-toc":
          insertTableOfContents();
          break;
        case "merge-cells":
          execMergeCells();
          break;
        case "split-cell":
          execSplitCell();
          break;
        case "add-table-row":
          addTableRow();
          break;
        case "delete-table-row":
          deleteTableRow();
          break;
        case "add-table-column":
          addTableColumn();
          break;
        case "delete-table-column":
          deleteTableColumn();
          break;
        case "delete-table":
          execDeleteTable();
          break;
        case "toggle-header-row":
          toggleHeaderRow();
          break;
        case "style-default":
          execBlockType("paragraph");
          break;
        case "style-h1":
          execBlockType("heading1");
          break;
        case "style-h2":
          execBlockType("heading2");
          break;
        case "style-h3":
          execBlockType("heading3");
          break;
        case "style-h4":
          execBlockType("heading4");
          break;
        case "style-h5":
          execBlockType("heading5");
          break;
        case "style-h6":
          execBlockType("heading6");
          break;
        case "print-preview":
          setPrintPreviewOpen(true);
          break;
        case "print":
          window.print();
          break;
        default:
          break;
      }
    };
    window.addEventListener(EDITOR_COMMAND, onCommand);
    onCleanup(() => window.removeEventListener(EDITOR_COMMAND, onCommand));
  });

  const refreshMatchDecorations = () => {
    withView((v) => {
      // Force plugin re-eval by empty transaction
      v.dispatch(v.state.tr.setMeta("find-refresh", true));
    });
  };

  const handleSearch = () => {
    if (!view || !findQuery()) {
      setMatchCount(0);
      setMatchIndex(0);
      refreshMatchDecorations();
      return;
    }
    const matches = collectMatches(view.state.doc, findQuery(), matchCase());
    setMatchCount(matches.length);
    if (matches.length === 0) setMatchIndex(0);
    else if (matchIndex() >= matches.length) setMatchIndex(0);
    refreshMatchDecorations();
  };

  const findAt = (dir: 1 | -1) => {
    if (!view || !findQuery()) return;
    const matches = collectMatches(view.state.doc, findQuery(), matchCase());
    setMatchCount(matches.length);
    if (!matches.length) {
      setMatchIndex(0);
      refreshMatchDecorations();
      return;
    }
    const { from } = view.state.selection;
    let idx =
      dir === 1
        ? matches.findIndex((m) => m.from > from)
        : (() => {
            for (let i = matches.length - 1; i >= 0; i--) if (matches[i].from < from) return i;
            return -1;
          })();
    if (idx < 0) idx = dir === 1 ? 0 : matches.length - 1;
    setMatchIndex(idx);
    const next = matches[idx];
    const sel = TextSelection.create(view.state.doc, next.from, next.to);
    view.dispatch(view.state.tr.setSelection(sel).scrollIntoView());
    refreshMatchDecorations();
  };

  const replaceCurrent = () => {
    if (!view || !findQuery()) return;
    const { from, to } = view.state.selection;
    const selected = view.state.doc.textBetween(from, to);
    const matches =
      matchCase() ? selected === findQuery() : selected.toLowerCase() === findQuery().toLowerCase();
    if (matches) {
      view.dispatch(view.state.tr.insertText(replaceWith(), from, to));
    }
    findAt(1);
    handleSearch();
  };

  const replaceAll = () => {
    if (!view || !findQuery()) return;
    const matches = collectMatches(view.state.doc, findQuery(), matchCase());
    let transaction = view.state.tr;
    for (const match of matches.reverse()) {
      transaction = transaction.insertText(replaceWith(), match.from, match.to);
    }
    if (matches.length) view.dispatch(transaction);
    setMatchIndex(0);
    handleSearch();
  };

  const clipboard = (action: "cut" | "copy" | "paste") => {
    document.execCommand(action);
  };

  const sidebarPanels = (): SidebarPanel[] => [
    {
      id: "properties",
      title: "Properties",
      icon: <IconProperties />,
      content: (
        <div style={{ display: "flex", "flex-direction": "column", gap: "8px" }}>
          <div><strong>Character</strong></div>
          <div>Font: {fontFamily()}</div>
          <div>Size: {fontSize()} pt</div>
          <div><strong>Paragraph</strong></div>
          <div>Alignment / indent via toolbar</div>
          <div style={{ "margin-top": "8px" }}>
            <label style={{ display: "block", "margin-bottom": "4px" }}>Layout</label>
            <select
              class="g-toolbar-select"
              value={layoutMode()}
              onChange={(e) => setLayoutMode(e.currentTarget.value as "paginated" | "focused")}
              style={{ width: "100%" }}
            >
              <option value="paginated">Print layout</option>
              <option value="focused">Web / pageless</option>
            </select>
          </div>
          <button type="button" class="g-toolbar-btn" style={{ width: "100%", "justify-content": "flex-start" }} onClick={insertPageBreak}>
            Insert page break
          </button>
        </div>
      ),
    },
    {
      id: "page",
      title: "Page",
      icon: <IconPage />,
      content: (
        <div style={{ display: "flex", "flex-direction": "column", gap: "6px" }}>
          <div>Page style: Default</div>
          <div>Format: <span style={{ "text-transform": "capitalize" }}>{pageSetup().paperSize}</span> ({paperDimensions().width} × {paperDimensions().height})</div>
          <div>Margins: {pageSetup().margins.top}″ / {pageSetup().margins.bottom}″ / {pageSetup().margins.left}″ / {pageSetup().margins.right}″</div>
          <div>Orientation: <span style={{ "text-transform": "capitalize" }}>{pageSetup().orientation}</span></div>
          <div>Text columns: {Math.max(1, Math.min(4, pageSetup().columns || 1))}</div>
          <div>Pages: {pageCount()}</div>
          <button type="button" class="g-toolbar-btn" onClick={() => setPageSetupOpen(true)}>
             Page Setup…
          </button>
        </div>
      ),
    },
    {
      id: "styles",
      title: "Styles",
      icon: <IconStyles />,
      content: (
        <div style={{ display: "flex", "flex-direction": "column", gap: "4px" }}>
          <For each={availableStyles()}>
            {(style) => (
              <button
                type="button"
                class="g-toolbar-btn"
                title={`Apply ${style.name}`}
                style={{
                  "justify-content": "flex-start",
                  width: "100%",
                  height: "auto",
                  padding: "4px 6px",
                  background: activeStyleId() === style.id ? "var(--bg-selected, #505050)" : "transparent",
                  "font-weight": style.bold ? "bold" : undefined,
                  "font-style": style.italic ? "italic" : undefined,
                  color: style.color || undefined,
                }}
                onClick={() => applyNamedStyle(style.id)}
              >
                {style.name}
              </button>
            )}
          </For>
          <div style={{ "border-top": "1px solid var(--border-color)", "margin-top": "4px", "padding-top": "6px" }}>
            <button
              type="button"
              class="g-toolbar-btn"
              style={{ "justify-content": "flex-start", width: "100%", height: "auto", padding: "4px 6px" }}
              onClick={() => createStyleFromSelection()}
            >
              + New style from selection
            </button>
            <div style={{ "font-size": "11px", color: "var(--text-muted)", "margin-top": "4px" }}>
              Named styles persist through DOCX round-trips as Word styles.
            </div>
          </div>
        </div>
      ),
    },
    {
      id: "footnotes",
      title: "Footnotes",
      icon: <IconList />,
      content: (
        <div style={{ display: "flex", "flex-direction": "column", gap: "6px" }}>
          <button
            type="button"
            class="g-toolbar-btn"
            style={{ "justify-content": "flex-start", width: "100%", height: "auto", padding: "4px 6px" }}
            onClick={() => insertFootnote()}
          >
            + Insert footnote at cursor
          </button>
          <For each={footnotes()}>
            {(note) => (
              <div style={{ display: "flex", "flex-direction": "column", gap: "3px", padding: "4px", background: "var(--bg-tertiary)", "border-radius": "4px" }}>
                <div style={{ display: "flex", "align-items": "center", gap: "6px" }}>
                  <span style={{ "font-size": "12px", "font-weight": "600", color: "var(--doc-accent)" }}>{note.label}.</span>
                  <span style={{ flex: 1 }} />
                  <button
                    type="button"
                    class="g-toolbar-btn"
                    aria-label={`Delete footnote ${note.label}`}
                    title="Delete footnote"
                    style={{ width: "22px", height: "20px", padding: 0, "font-size": "11px" }}
                    onClick={() => removeFootnote(note.id)}
                  >
                    ✕
                  </button>
                </div>
                <textarea
                  aria-label={`Footnote ${note.label} text`}
                  rows={2}
                  value={note.text}
                  onInput={(e) => updateFootnoteText(note.id, e.currentTarget.value)}
                  style={{ "font-size": "12px", width: "100%", resize: "vertical" }}
                />
              </div>
            )}
          </For>
          <Show when={footnotes().length === 0}>
            <div style={{ "font-size": "11px", color: "var(--text-muted)" }}>
              No footnotes yet. Insert one at the cursor; notes export as native Word footnotes.
            </div>
          </Show>
        </div>
      ),
    },
    {
      id: "gallery",
      title: "Gallery",
      icon: <IconGallery />,
      content: (
        <div>
          <button type="button" class="g-toolbar-btn" style={{ width: "100%" }} onClick={() => setImageDialogOpen(true)}>
            Insert image…
          </button>
          <div style={{ "margin-top": "8px", "font-size": "12px", color: "var(--text-muted)" }}>
            Paste an image from the clipboard into the document.
          </div>
        </div>
      ),
    },
    {
      id: "navigator",
      title: "Navigator",
      icon: <IconNavigator />,
      content: (
        <div style={{ "font-size": "13px", display: "flex", "flex-direction": "column", gap: "4px" }}>
          <Show 
            when={headings().length > 0} 
            fallback={<div style={{ color: "var(--text-muted)", "font-size": "12px", padding: "8px" }}>No headings found. Add Headings (H1-H3) to see outline tree.</div>}
          >
            {headings().map(h => (
              <button
                type="button"
                class="g-toolbar-btn"
                style={{ 
                  "justify-content": "flex-start", 
                  width: "100%", 
                  height: "auto", 
                  "padding-left": (h.level - 1) * 12 + "px",
                  "text-align": "left",
                  "white-space": "nowrap",
                  overflow: "hidden",
                  "text-overflow": "ellipsis"
                }}
                onClick={() => {
                  if (view) {
                    const sel = TextSelection.create(view.state.doc, h.pos);
                    view.dispatch(view.state.tr.setSelection(sel).scrollIntoView());
                  }
                }}
              >
                <span style={{ 
                  "font-size": "10px", 
                  "font-weight": "bold", 
                  background: "var(--bg-hover)", 
                  padding: "2px 4px", 
                  "border-radius": "4px", 
                  "margin-right": "6px",
                  color: "var(--text-muted)"
                }}>H{h.level}</span>
                <span style={{ overflow: "hidden", "text-overflow": "ellipsis" }}>{h.text || "Untitled"}</span>
              </button>
            ))}
          </Show>
        </div>
      ),
    },
    {
      id: "review",
      title: "Review",
      icon: <span aria-hidden="true" style={{ "font-size": "15px" }}>💬</span>,
      content: (
        <div style={{ display: "flex", "flex-direction": "column", gap: "8px" }}>
          <button type="button" class="g-toolbar-btn active" style={{ width: "100%" }} onClick={openCommentDialog}>
            New comment
          </button>
          <div style={{ "font-size": "11px", color: "var(--text-muted)" }}>
            Tracked changes: {trackedStats().insertions} insertions ({trackedStats().insertedCharacters} chars), {trackedStats().deletions} deletions ({trackedStats().deletedCharacters} chars)
          </div>
          <Show when={trackedChangeSummaries().length > 0}>
            <div style={{ "border-top": "1px solid var(--border-color)", "padding-top": "7px" }}>
              <div style={{ "font-size": "11px", color: "var(--text-muted)", "margin-bottom": "5px" }}>Individual changes</div>
              <For each={trackedChangeSummaries()}>
                {(change) => (
                  <div style={{ display: "flex", "align-items": "center", gap: "5px", "font-size": "11px", "margin-top": "4px" }}>
                    <span style={{ flex: 1, overflow: "hidden", "text-overflow": "ellipsis", "white-space": "nowrap" }} title={change.text}>
                      {change.kind === "insert" ? "Insertion" : "Deletion"} · {change.author} · {change.text || "(empty)"}
                    </span>
                    <button type="button" class="g-toolbar-btn" onClick={() => resolveOneTrackedChange(change.id, "accept")}>Accept</button>
                    <button type="button" class="g-toolbar-btn" onClick={() => resolveOneTrackedChange(change.id, "reject")}>Reject</button>
                  </div>
                )}
              </For>
            </div>
          </Show>
          <div style={{ "font-size": "11px", color: "var(--text-muted)", "border-top": "1px solid var(--border-color)", "padding-top": "7px" }}>
            {(() => {
              const comparison = compareSummary();
              return comparison.changed
                ? `${compareBaselineLabel()}: +${comparison.insertedWords} / −${comparison.deletedWords} words${comparison.truncated ? " (large-document summary)" : ""}`
                : `No text changes ${compareBaselineLabel().toLowerCase()}`;
            })()}
          </div>
          <Show when={(props.compareDocuments?.length ?? 0) > 0}>
            <label style={{ display: "flex", "flex-direction": "column", gap: "4px", "font-size": "11px" }}>
              Compare with open Writer document
              <select
                class="g-toolbar-select"
                aria-label="Compare with open Writer document"
                value={compareSourceId()}
                onChange={(event) => compareWithDocument(event.currentTarget.value)}
                style={{ height: "28px", padding: "0 6px" }}
              >
                <option value="">Select a document…</option>
                <For each={props.compareDocuments ?? []}>
                  {(document) => <option value={document.id}>{document.title || "Untitled document"}</option>}
                </For>
              </select>
            </label>
          </Show>
          <button type="button" class="g-toolbar-btn" onClick={resetCompareBaseline}>
            Reset compare baseline
          </button>
          <button
            type="button"
            class="g-toolbar-btn"
            aria-expanded={compareDetailsOpen()}
            onClick={() => setCompareDetailsOpen((open) => !open)}
          >
            {compareDetailsOpen() ? "Hide comparison" : "Show before/after comparison"}
          </button>
          <Show when={compareDetailsOpen()}>
            {(() => {
              const comparison = compareSummary();
              return (
                <div style={{ display: "flex", "flex-direction": "column", gap: "6px", "font-size": "11px" }}>
                  <div style={{ display: "grid", "grid-template-columns": "1fr 1fr", gap: "6px" }}>
                    <div>
                      <div style={{ "font-weight": "600", "margin-bottom": "3px" }}>Before</div>
                      <div style={{ "max-height": "180px", overflow: "auto", "white-space": "pre-wrap", padding: "6px", border: "1px solid var(--border-color)", "border-radius": "4px", background: "var(--bg-secondary, transparent)" }}>
                        {comparePreview(comparison.beforeText) || "(empty)"}
                      </div>
                    </div>
                    <div>
                      <div style={{ "font-weight": "600", "margin-bottom": "3px" }}>After</div>
                      <div style={{ "max-height": "180px", overflow: "auto", "white-space": "pre-wrap", padding: "6px", border: "1px solid var(--border-color)", "border-radius": "4px", background: "var(--bg-secondary, transparent)" }}>
                        {comparePreview(comparison.afterText) || "(empty)"}
                      </div>
                    </div>
                  </div>
                  <div style={{ "font-weight": "600" }}>Token diff</div>
                  <div style={{ "max-height": "120px", overflow: "auto", "white-space": "pre-wrap", padding: "6px", border: "1px solid var(--border-color)", "border-radius": "4px" }}>
                    <For each={comparison.segments}>
                      {(segment) => (
                        <span style={{
                          background: segment.kind === "insert" ? "#dcfce7" : segment.kind === "delete" ? "#fee2e2" : "transparent",
                          color: segment.kind === "delete" ? "#991b1b" : "inherit",
                          "text-decoration": segment.kind === "delete" ? "line-through" : "none",
                        }}>
                          {segment.text}
                        </span>
                      )}
                    </For>
                  </div>
                  <Show when={comparison.truncated}>
                    <div style={{ color: "var(--text-muted)" }}>Large-document preview is capped for responsiveness.</div>
                  </Show>
                </div>
              );
            })()}
          </Show>
          <div style={{ display: "flex", gap: "6px", "flex-wrap": "wrap" }}>
            <button
              type="button"
              class="g-toolbar-btn"
              aria-pressed={trackChangesOn()}
              onClick={() => setTrackChangesOn(!trackChangesOn())}
              style={trackChangesOn() ? { "font-weight": "700", outline: "2px solid var(--doc-accent, #7c3aed)", "outline-offset": "-2px" } : undefined}
              title="Automatically mark typed insertions as tracked changes"
            >
              {trackChangesOn() ? "✓ Track Changes: On" : "Track Changes"}
            </button>
            <button type="button" class="g-toolbar-btn" onClick={() => markSelectionAsChange("insert")}>
              Mark insertion
            </button>
            <button type="button" class="g-toolbar-btn" onClick={() => markSelectionAsChange("delete")}>
              Mark deletion
            </button>
          </div>
          <div style={{ display: "flex", gap: "6px", "flex-wrap": "wrap" }}>
            <button type="button" class="g-toolbar-btn" onClick={() => resolveAllTrackedChanges("accept")}>
              Accept all
            </button>
            <button type="button" class="g-toolbar-btn" onClick={() => resolveAllTrackedChanges("reject")}>
              Reject all
            </button>
          </div>
          <Show
            when={comments().length > 0}
            fallback={<div style={{ color: "var(--text-muted)", "font-size": "12px", padding: "8px 0" }}>Select text and add a comment to start a review thread.</div>}
          >
            <For each={comments()}>
              {(comment) => (
                <div
                  style={{
                    padding: "8px",
                    border: "1px solid var(--border-color)",
                    "border-radius": "6px",
                    background: selectedCommentId() === comment.id ? "var(--bg-hover)" : "transparent",
                    opacity: comment.resolved ? "0.65" : "1",
                  }}
                >
                  <button
                    type="button"
                    onClick={() => selectComment(comment)}
                    style={{ display: "block", width: "100%", padding: "0", background: "transparent", "text-align": "left", "font-size": "12px" }}
                    title="Select comment anchor"
                  >
                    <strong>{comment.author}</strong>
                    <span style={{ display: "block", "margin-top": "4px", "white-space": "pre-wrap", color: "var(--text-primary)" }}>{comment.text}</span>
                  </button>
                  {/* Threaded replies */}
                  <Show when={(comment.replies?.length ?? 0) > 0}>
                    <div style={{ "margin-top": "6px", "padding-left": "8px", "border-left": "2px solid var(--border-color)" }}>
                      <For each={comment.replies}>
                        {(reply) => (
                          <div style={{ "font-size": "12px", "margin-bottom": "4px" }}>
                            <strong>{reply.author}</strong>
                            <span style={{ display: "block", "white-space": "pre-wrap", color: "var(--text-primary)" }}>{reply.text}</span>
                          </div>
                        )}
                      </For>
                    </div>
                  </Show>
                  <Show when={selectedCommentId() === comment.id}>
                    <div style={{ display: "flex", gap: "4px", "margin-top": "6px" }}>
                      <input
                        type="text"
                        placeholder="Reply…"
                        aria-label={`Reply to ${comment.author}'s comment`}
                        value={replyDrafts()[comment.id] ?? ""}
                        onInput={(e) => { setReplyDrafts((prev) => ({ ...prev, [comment.id]: e.currentTarget.value })); }}
                        onKeyDown={(e) => {
                          if (e.key === "Enter") {
                            e.preventDefault();
                            submitReply(comment.id);
                          }
                        }}
                        style={{ flex: 1, "font-size": "12px", padding: "3px 8px", border: "1px solid var(--border-color)", "border-radius": "4px", background: "var(--bg-input, transparent)", color: "inherit" }}
                      />
                      <button type="button" class="g-toolbar-btn" onClick={() => submitReply(comment.id)}>Reply</button>
                    </div>
                  </Show>
                  <div style={{ display: "flex", gap: "6px", "margin-top": "7px" }}>
                    <button type="button" class="g-toolbar-btn" onClick={() => updateComment(comment.id, { resolved: !comment.resolved })}>
                      {comment.resolved ? "Reopen" : "Resolve"}
                    </button>
                    <button type="button" class="g-toolbar-btn" onClick={() => removeComment(comment.id)}>Delete</button>
                  </div>
                </div>
              )}
            </For>
          </Show>
        </div>
      ),
    },
  ];

  const zoom = () => props.zoomLevel ?? 100;

  return (
    <div style={{ display: "flex", "flex-direction": "column", height: "100%", background: "var(--bg-canvas)", overflow: "hidden" }}>
      <Show when={loadWarning()}>
        <div
          data-testid="doc-load-warning"
          style={{
            display: "flex", "align-items": "center", "justify-content": "space-between",
            gap: "12px", padding: "8px 16px",
            background: "var(--warn-bg, #3a2b0b)", color: "var(--warn-fg, #ffd479)",
            "font-size": "13px", "border-bottom": "1px solid rgba(255, 212, 121, 0.35)",
          }}
          role="status"
        >
          <span>{loadWarning()}</span>
          <button
            type="button"
            aria-label="Dismiss warning"
            style={{
              background: "transparent", border: "1px solid currentColor",
              color: "inherit", "border-radius": "6px", padding: "2px 10px", cursor: "pointer",
            }}
            onClick={() => setLoadWarning(null)}
          >
            Dismiss
          </button>
        </div>
      </Show>
      <DocToolbar
        onRequestNew={props.onRequestNew}
        onRequestOpen={props.onRequestOpen}
        onRequestSave={props.onRequestSave}
        onRequestExportPdf={props.onRequestExportPdf}
        onRequestExportDocx={props.onRequestExportDocx}
        onPageSetup={() => setPageSetupOpen(true)}
        onCut={() => clipboard("cut")}
        onCopy={() => clipboard("copy")}
        onPaste={() => clipboard("paste")}
        onUndo={execUndo}
        onRedo={execRedo}
        findOpen={findOpen()}
        onToggleFind={() => setFindOpen(!findOpen())}
        onInsertTable={() => insertTable()}
        onInsertImage={() => setImageDialogOpen(true)}
        onInsertLink={openLinkDialog}
        onInsertBookmark={openBookmarkDialog}
        onInsertPageBreak={insertPageBreak}
        onInsertPageField={() => insertDocumentField("page")}
        onInsertNumPagesField={() => insertDocumentField("numPages")}
        onInsertSectionBreak={insertSectionBreak}
        onInsertToc={insertTableOfContents}
        onAddTableRow={addTableRow}
        onDeleteTableRow={deleteTableRow}
        onAddTableColumn={addTableColumn}
        onDeleteTableColumn={deleteTableColumn}
        onDeleteTable={execDeleteTable}
        onMergeCells={execMergeCells}
        onSplitCell={execSplitCell}
        onToggleHeaderRow={toggleHeaderRow}
        currentBlockType={currentBlockType()}
        onChangeBlockType={execBlockType}
        fontFamily={fontFamily()}
        onChangeFontFamily={applyFont}
        fontSize={fontSize()}
        onChangeFontSize={applyFontSize}
        onShrinkFontSize={execShrinkFontSize}
        onGrowFontSize={execGrowFontSize}
        onToggleBold={execToggleBold}
        onToggleItalic={execToggleItalic}
        onToggleUnderline={execToggleUnderline}
        onToggleStrike={execToggleStrike}
        onToggleSuper={execToggleSuper}
        onToggleSub={execToggleSub}
        onClearFormatting={clearFormatting}
        painterActive={painterState() !== "idle"}
        onToggleFormatPainter={() => armFormatPainter(false)}
        textColor={textColor()}
        onChangeTextColor={(c) => { setTextColor(c); execColor("color", c); }}
        highlightColor={highlightColor()}
        onChangeHighlightColor={(c) => { setHighlightColor(c); execColor("highlight", c); }}
        onAlignLeft={() => execBlockAttrs({ align: "left" })}
        onAlignCenter={() => execBlockAttrs({ align: "center" })}
        onAlignRight={() => execBlockAttrs({ align: "right" })}
        onAlignJustify={() => execBlockAttrs({ align: "justify" })}
        onListBullet={() => execList("bullet_list")}
        onListOrdered={() => execList("ordered_list")}
        onDecreaseIndent={() => adjustIndent(-1)}
        onIncreaseIndent={() => adjustIndent(1)}
        currentLineSpacing={currentLineSpacing()}
        onChangeLineSpacing={(v) => execBlockAttrs({ lineHeight: v })}
        spacingBefore={spacingBefore()}
        spacingAfter={spacingAfter()}
        onChangeSpacingBefore={(v) => execBlockAttrs({ spacingBefore: v })}
        onChangeSpacingAfter={(v) => execBlockAttrs({ spacingAfter: v })}
      />

      <FindReplace
        open={findOpen()}
        query={findQuery()}
        replaceWith={replaceWith()}
        matchCount={matchCount()}
        matchIndex={matchIndex()}
        matchCase={matchCase()}
        onQueryChange={(q) => {
          setFindQuery(q);
          setMatchIndex(0);
          handleSearch();
        }}
        onReplaceChange={setReplaceWith}
        onFind={handleSearch}
        onFindNext={() => findAt(1)}
        onFindPrev={() => findAt(-1)}
        onReplace={replaceCurrent}
        onReplaceAll={replaceAll}
        onMatchCaseChange={(v) => {
          setMatchCase(v);
          handleSearch();
        }}
        onClose={() => setFindOpen(false)}
      />

      <Show when={pageSetup().header || pageSetup().footer}>
        {(() => {
          const getCssContent = (text: string | undefined) => {
            if (!text) return `""`;
            const withTotal = text
              .replace(/{pages}/g, String(pageCount()))
              .replace(/{total}/g, String(pageCount()));
            const parts = withTotal.split(/{page}/g);
            return parts.length === 1 ? JSON.stringify(parts[0]) : parts.map((p) => JSON.stringify(p)).join(" counter(page) ");
          };
          return (
            <style>{`
              .doc-paginated {
                counter-reset: page 1;
                position: relative;
              }
              .doc-paginated::before {
                content: ${getCssContent(pageSetup().header)};
                position: absolute;
                top: 24px;
                left: 0;
                right: 0;
                text-align: center;
                color: #9aa0a6;
                font-size: 12px;
                pointer-events: none;
              }
              .doc-paginated::after {
                content: ${getCssContent(pageSetup().footer)};
                position: absolute;
                bottom: 24px;
                left: 0;
                right: 0;
                text-align: center;
                color: #9aa0a6;
                font-size: 12px;
                pointer-events: none;
              }
              .doc-page-break::before {
                content: ${getCssContent(pageSetup().footer)};
                position: absolute;
                bottom: 12px;
                left: 0;
                right: 0;
                text-align: center;
                color: #9aa0a6;
                font-size: 12px;
                pointer-events: none;
              }
              .doc-page-break::after {
                counter-increment: page;
                content: ${getCssContent(pageSetup().header)};
                position: absolute;
                top: 12px;
                left: 0;
                right: 0;
                text-align: center;
                color: #9aa0a6;
                font-size: 12px;
                pointer-events: none;
              }
              .doc-print-header, .doc-print-footer {
                display: none;
              }
              @media print {
                .doc-print-header {
                  display: block;
                  position: fixed;
                  top: 0;
                  left: 0;
                  right: 0;
                  text-align: center;
                  font-size: 12px;
                  color: #000;
                }
                .doc-print-footer {
                  display: block;
                  position: fixed;
                  bottom: 0;
                  left: 0;
                  right: 0;
                  text-align: center;
                  font-size: 12px;
                  color: #000;
                }
                .doc-paginated::before, .doc-paginated::after, .doc-page-break::before, .doc-page-break::after {
                  display: none !important;
                }
              }
            `}</style>
          );
        })()}
      </Show>

      <Show when={pageSetup().header}>
        <div class="doc-print-header">
          {formatPrintHeaderFooter(pageSetup().header, pageCount(), currentCurPage)}
        </div>
      </Show>
      <Show when={pageSetup().footer}>
        <div class="doc-print-footer">
          {formatPrintHeaderFooter(pageSetup().footer, pageCount(), currentCurPage)}
        </div>
      </Show>

      <style>{`.doc-comment-highlight { background: color-mix(in srgb, var(--doc-accent) 24%, transparent); border-bottom: 2px solid var(--doc-accent); } .doc-track-insert { background: color-mix(in srgb, #22c55e 18%, transparent); border-bottom: 2px solid #22c55e; } .doc-track-delete { background: color-mix(in srgb, #ef4444 14%, transparent); color: #b91c1c; text-decoration: line-through; }`}</style>

      <style>{`.doc-spell-misspelled { text-decoration: underline wavy #e74c3c; text-underline-offset: 2px; }`}</style>

      <Ruler
        zoom={zoom()}
        leftMargin={pageSetup().margins.left * 96}
        rightMargin={pageSetup().margins.right * 96}
        pageWidth={paperDimensions().width}
        tabStops={tabStops()}
        onAddTabStop={(pos) => setTabStops((prev) => [...prev, pos].sort((a, b) => a - b))}
      />

      <div style={{ flex: 1, display: "flex", "min-height": "0", overflow: "hidden" }}>
        <main
          data-pane="canvas"
          aria-label="Document canvas"
          style={{
            flex: 1,
            overflow: "auto",
            display: "flex",
            "justify-content": "center",
            padding: "16px 16px 48px",
            "background-color": "var(--bg-canvas)",
          }}
        >
          <div
            class={layoutMode() === "focused" ? "doc-focused" : "doc-paginated"}
            style={{
              width: layoutMode() === "focused" ? "816px" : `${paperDimensions().width}px`,
              "min-height": layoutMode() === "focused" ? "calc(100vh - 220px)" : `${paperDimensions().height}px`,
              background: "var(--bg-paper)",
              "border-radius": "0",
              "box-shadow": "var(--shadow-paper)",
              padding: layoutMode() === "focused" 
                ? "48px 72px" 
                : `${pageSetup().margins.top * 96}px ${pageSetup().margins.right * 96}px ${pageSetup().margins.bottom * 96}px ${pageSetup().margins.left * 96}px`,
              color: "#000000",
              outline: "none",
              transform: `scale(${zoom() / 100})`,
              "transform-origin": "top center",
              "margin-bottom": zoom() !== 100 ? `${((zoom() / 100) - 1) * paperDimensions().height}px` : "0",
            }}
          >
            <div
              ref={editorRef}
              spellcheck={props.spellcheckEnabled !== false}
              style={{
                "min-height": "800px",
                outline: "none",
                "font-family": "Liberation Serif, serif",
                "font-size": "12pt",
                "column-count": Math.max(1, Math.min(4, pageSetup().columns || 1)),
                "column-gap": "36px",
              }}
              onDragOver={(e) => e.preventDefault()}
              onDrop={(e) => {
                e.preventDefault();
                if (!view) return;
                const file = e.dataTransfer?.files?.[0];
                if (!file || !file.type.startsWith("image/")) return;
                const pos = view.posAtCoords({ left: e.clientX, top: e.clientY })?.pos;
                insertImageFromFile(file, pos);
              }}
              onContextMenu={(event) => {
                event.preventDefault();
                const items: ContextMenuItem[] = [
                  { id: "cut", label: "Cut", action: () => clipboard("cut") },
                  { id: "copy", label: "Copy", action: () => clipboard("copy") },
                  { id: "paste", label: "Paste", action: () => clipboard("paste") },
                  { id: "sep1", label: "", separator: true },
                  { id: "bold", label: "Bold", action: () => emitEditorCommand("bold") },
                  { id: "italic", label: "Italic", action: () => emitEditorCommand("italic") },
                  { id: "sep2", label: "", separator: true },
                  { id: "insert-link", label: "Insert Link…", action: () => emitEditorCommand("insert-link") },
                  { id: "insert-bookmark", label: "Insert Bookmark…", action: () => emitEditorCommand("insert-bookmark") },
                  { id: "insert-table", label: "Insert Table", action: () => emitEditorCommand("insert-table") },
                  { id: "insert-section-break", label: "Insert Section Break", action: () => emitEditorCommand("insert-section-break") },
                  { id: "insert-toc", label: "Insert Table of Contents", action: () => emitEditorCommand("insert-toc") },
                  { id: "sep-table", label: "", separator: true },
                  { id: "add-table-row", label: "Add Row", action: () => emitEditorCommand("add-table-row") },
                  { id: "delete-table-row", label: "Delete Row", action: () => emitEditorCommand("delete-table-row") },
                  { id: "add-table-column", label: "Add Column", action: () => emitEditorCommand("add-table-column") },
                  { id: "delete-table-column", label: "Delete Column", action: () => emitEditorCommand("delete-table-column") },
                  { id: "delete-table", label: "Delete Table", action: () => emitEditorCommand("delete-table") },
                  { id: "merge-cells", label: "Merge cells", action: () => emitEditorCommand("merge-cells") },
                  { id: "split-cell", label: "Split cell", action: () => emitEditorCommand("split-cell") },
                  { id: "toggle-header-row", label: "Toggle Header", action: () => emitEditorCommand("toggle-header-row") },
                  { id: "sep3", label: "", separator: true },
                  { id: "find", label: "Find…", action: () => emitEditorCommand("find") },
                ];
                setContextMenu({ x: event.clientX, y: event.clientY, items });
              }}
            />
            <Show when={selectedImage() && view}>
              <div
                role="button"
                tabindex="0"
                aria-label="Resize image"
                title="Drag to resize image"
                style={{
                  position: "fixed",
                  left: `${view!.coordsAtPos(selectedImage()!.pos).right - 6}px`,
                  top: `${view!.coordsAtPos(selectedImage()!.pos).bottom - 6}px`,
                  width: "12px",
                  height: "12px",
                  background: "var(--doc-accent)",
                  border: "2px solid white",
                  cursor: "nwse-resize",
                  "z-index": 50,
                }}
                onPointerDown={(e) => {
                  const img = selectedImage()!;
                  const node = view!.state.doc.nodeAt(img.pos);
                  const startW = Number(node?.attrs.width) || img.width;
                  const startH = Number(node?.attrs.height) || img.height;
                  e.preventDefault();
                  const startX = e.clientX;
                  const startY = e.clientY;
                  const onMove = (ev: PointerEvent) => {
                    resizeSelectedImage(startW + (ev.clientX - startX), startH + (ev.clientY - startY));
                  };
                  const onUp = () => {
                    window.removeEventListener("pointermove", onMove);
                    window.removeEventListener("pointerup", onUp);
                  };
                  window.addEventListener("pointermove", onMove);
                  window.addEventListener("pointerup", onUp);
                }}
              />
            </Show>
          </div>
        </main>
        <IconSidebar panels={sidebarPanels()} defaultPanel="properties" />
      </div>

      <Show when={contextMenu()}>
        {(menu) => (
          <ContextMenu
            x={menu().x}
            y={menu().y}
            items={menu().items}
            onClose={() => setContextMenu(null)}
          />
        )}
      </Show>

      <Dialog open={linkDialogOpen()} title="Insert hyperlink" onClose={() => setLinkDialogOpen(false)}>
        <div style={{ display: "flex", "flex-direction": "column", gap: "12px", "min-width": "360px" }}>
          <label style={{ display: "flex", "flex-direction": "column", gap: "4px", "font-size": "13px" }}>
            URL
            <input
              class="g-toolbar-input"
              value={linkHref()}
              onInput={(e) => setLinkHref(e.currentTarget.value)}
              style={{ width: "100%", height: "28px", padding: "0 8px" }}
            />
          </label>
          <Show when={linkError()}>
            <div role="alert" style={{ "font-size": "12px", color: "var(--danger, #b91c1c)" }}>{linkError()}</div>
          </Show>
          <div style={{ display: "flex", gap: "8px", "justify-content": "flex-end" }}>
            <button type="button" class="g-toolbar-btn" onClick={() => setLinkDialogOpen(false)}>Cancel</button>
            <button type="button" class="g-toolbar-btn active" onClick={applyLink}>Apply</button>
          </div>
          <div style={{ "font-size": "11px", color: "var(--text-muted)" }}>
            For a same-document target, use <code>internal:BookmarkName</code> or <code>#BookmarkName</code>.
          </div>
        </div>
      </Dialog>

      <Dialog open={bookmarkDialogOpen()} title="Insert bookmark" onClose={() => setBookmarkDialogOpen(false)}>
        <div style={{ display: "flex", "flex-direction": "column", gap: "12px", "min-width": "360px" }}>
          <div style={{ "font-size": "12px", color: "var(--text-muted)" }}>
            Select text before opening this dialog. Word-compatible names are limited to 40 characters.
          </div>
          <label style={{ display: "flex", "flex-direction": "column", gap: "4px", "font-size": "13px" }}>
            Bookmark name
            <input
              class="g-toolbar-input"
              value={bookmarkName()}
              onInput={(event) => {
                setBookmarkName(event.currentTarget.value);
                setBookmarkError("");
              }}
              maxlength={40}
              autofocus
              aria-label="Bookmark name"
              placeholder="ProjectPlan"
              style={{ width: "100%", height: "28px", padding: "0 8px" }}
            />
          </label>
          <Show when={bookmarkError()}>
            <div role="alert" style={{ "font-size": "12px", color: "var(--danger, #b91c1c)" }}>{bookmarkError()}</div>
          </Show>
          <div style={{ display: "flex", gap: "8px", "justify-content": "flex-end" }}>
            <button type="button" class="g-toolbar-btn" onClick={() => setBookmarkDialogOpen(false)}>Cancel</button>
            <button type="button" class="g-toolbar-btn active" onClick={applyBookmark}>Insert</button>
          </div>
        </div>
      </Dialog>

      <Dialog open={commentDialogOpen()} title="New comment" onClose={() => setCommentDialogOpen(false)}>
        <div style={{ display: "flex", "flex-direction": "column", gap: "10px", "min-width": "360px" }}>
          <div style={{ "font-size": "12px", color: "var(--text-muted)" }}>Your comment will be attached to the selected text.</div>
          <textarea
            class="g-toolbar-input"
            value={commentDraft()}
            onInput={(event) => setCommentDraft(event.currentTarget.value)}
            rows={5}
            autofocus
            aria-label="Comment text"
            placeholder="Write a comment…"
            style={{ width: "100%", resize: "vertical", padding: "8px" }}
          />
          <div style={{ display: "flex", gap: "8px", "justify-content": "flex-end" }}>
            <button type="button" class="g-toolbar-btn" onClick={() => setCommentDialogOpen(false)}>Cancel</button>
            <button type="button" class="g-toolbar-btn active" disabled={!commentDraft().trim()} onClick={saveComment}>Add comment</button>
          </div>
        </div>
      </Dialog>

      <Dialog open={imageDialogOpen()} title="Insert image" onClose={() => setImageDialogOpen(false)}>
        <div style={{ display: "flex", "flex-direction": "column", gap: "12px", "min-width": "360px" }}>
          <label style={{ display: "flex", "flex-direction": "column", gap: "4px", "font-size": "13px" }}>
            Image URL or data URI
            <input
              class="g-toolbar-input"
              value={imageSrc()}
              onInput={(e) => setImageSrc(e.currentTarget.value)}
              style={{ width: "100%", height: "28px", padding: "0 8px" }}
              placeholder="https://… or paste data:image/…"
            />
          </label>
          <label style={{ "font-size": "13px" }}>
            Or choose a local file
            <input
              type="file"
              accept="image/*"
              style={{ display: "block", "margin-top": "6px" }}
              onChange={(e) => {
                const file = e.currentTarget.files?.[0];
                if (!file) return;
                const reader = new FileReader();
                reader.onload = () => setImageSrc(String(reader.result || ""));
                reader.readAsDataURL(file);
              }}
            />
          </label>
          <div style={{ display: "flex", gap: "8px", "justify-content": "flex-end" }}>
            <button type="button" class="g-toolbar-btn" onClick={() => setImageDialogOpen(false)}>Cancel</button>
            <button type="button" class="g-toolbar-btn active" onClick={applyImage}>Insert</button>
          </div>
        </div>
      </Dialog>

      <BubbleToolbar
        visible={bubbleVisible()}
        top={bubbleTop()}
        left={bubbleLeft()}
        onBold={execToggleBold}
        onItalic={execToggleItalic}
        onUnderline={execToggleUnderline}
        onStrikethrough={execToggleStrike}
        onLink={openLinkDialog}
      />

      <PageSetupDialog
        open={pageSetupOpen()}
        onClose={() => setPageSetupOpen(false)}
        config={pageSetup()}
        onApply={(config) => {
          const normalized = normalizePageSetupConfig(config);
          setPageSetup(normalized);
          if (view) {
            refreshStatus(buildDocJson(view.state.doc, normalized, comments(), customStyles(), footnotes()), false);
            emitDocChange(view.state.doc, normalized);
          }
        }}
      />

      <Show when={printPreviewOpen()}>
        <PrintPreview
          content={
            view
              ? (() => {
                  const div = document.createElement("div");
                  const v = view as any as EditorView;
                  div.appendChild(DOMSerializer.fromSchema(mySchema).serializeFragment(v.state.doc.content));
                  return div.innerHTML;
                })()
              : ""
          }
          pageSetup={pageSetup()}
          pageCount={pageCount()}
          onClose={() => setPrintPreviewOpen(false)}
        />
      </Show>
    </div>
  );
}
