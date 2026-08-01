import { createEffect, createSignal, onCleanup, onMount, Show } from "solid-js";
import { EditorState, Plugin, TextSelection, NodeSelection } from "prosemirror-state";
import { Decoration, DecorationSet, EditorView } from "prosemirror-view";
import { Schema, MarkSpec, NodeSpec, Mark, Node, Fragment, Slice } from "prosemirror-model";
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
} from "prosemirror-tables";
import { Dialog } from "@redoc/ui";
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
import { DocToolbar } from "./DocToolbar";
import { FindReplace } from "./FindReplace";
import { transformPastedHTML } from "./pasteSanitizer";
import type { DocContent, DocEditorProps, TableCommandState } from "./types";

/** Persist as bold/italic so DOCX/PDF exporters match. */
const customMarks: Record<string, MarkSpec> = {
  bold: {
    parseDOM: [
      { tag: "strong" },
      { tag: "b" },
      { style: "font-weight=bold" },
      { style: "font-weight=700" },
    ],
    toDOM: () => ["strong", 0],
  },
  italic: {
    parseDOM: [{ tag: "em" }, { tag: "i" }, { style: "font-style=italic" }],
    toDOM: () => ["em", 0],
  },
  link: basicSchema.spec.marks.get("link")!,
  underline: {
    parseDOM: [{ tag: "u" }, { style: "text-decoration=underline" }],
    toDOM: () => ["u", 0],
  },
  strike: {
    parseDOM: [{ tag: "s" }, { tag: "del" }, { style: "text-decoration=line-through" }],
    toDOM: () => ["s", 0],
  },
  color: {
    attrs: { color: { default: "#111827" } },
    parseDOM: [{ style: "color", getAttrs: (value) => ({ color: value }) }],
    toDOM: (mark) => ["span", { style: `color: ${mark.attrs.color}` }, 0],
  },
  highlight: {
    attrs: { color: { default: "#fef08a" } },
    parseDOM: [{ style: "background-color", getAttrs: (value) => ({ color: value }) }],
    toDOM: (mark) => ["span", { style: `background-color: ${mark.attrs.color}` }, 0],
  },
  fontFamily: {
    attrs: { family: { default: "Liberation Serif" } },
    parseDOM: [
      {
        style: "font-family",
        getAttrs: (value) => ({ family: String(value).split(",")[0].replace(/['"]/g, "").trim() }),
      },
    ],
    toDOM: (mark) => ["span", { style: `font-family: ${mark.attrs.family}` }, 0],
  },
  fontSize: {
    attrs: { size: { default: "12" } },
    parseDOM: [
      {
        style: "font-size",
        getAttrs: (value) => {
          const m = String(value).match(/([\d.]+)/);
          return m ? { size: m[1] } : null;
        },
      },
    ],
    toDOM: (mark) => ["span", { style: `font-size: ${mark.attrs.size}pt` }, 0],
  },
  superscript: {
    excludes: "subscript",
    parseDOM: [{ tag: "sup" }, { style: "vertical-align=super" }],
    toDOM: () => ["sup", 0],
  },
  subscript: {
    excludes: "superscript",
    parseDOM: [{ tag: "sub" }, { style: "vertical-align=sub" }],
    toDOM: () => ["sub", 0],
  },
};

const pageBreakSpec: NodeSpec = {
  group: "block",
  selectable: true,
  atom: true,
  parseDOM: [{ tag: "div[data-page-break]" }],
  toDOM: () => [
    "div",
    {
      "data-page-break": "true",
      class: "doc-page-break",
      style:
        "page-break-after:always;border-top:1px dashed #9aa0a6;margin:24px 0;height:0;position:relative;",
      contenteditable: "false",
    },
  ],
};

const styledNodes = addListNodes(basicSchema.spec.nodes, "paragraph block*", "block")
  .update("paragraph", {
    ...basicSchema.spec.nodes.get("paragraph"),
    attrs: {
      align: { default: "left" },
      indent: { default: 0 },
      lineHeight: { default: 1.5 },
      spacingBefore: { default: 0 },
      spacingAfter: { default: 0 },
    },
    parseDOM: [
      {
        tag: "p",
        getAttrs: (dom) => {
          const el = dom as HTMLElement;
          const mt = el.style.marginTop?.replace("pt", "");
          const mb = el.style.marginBottom?.replace("pt", "");
          return {
            spacingBefore: mt ? Number(mt) || 0 : 0,
            spacingAfter: mb ? Number(mb) || 0 : 0,
          };
        },
      },
    ],
    toDOM: (node) => [
      "p",
      {
        style: `text-align:${node.attrs.align};margin-left:${node.attrs.indent}em;line-height:${node.attrs.lineHeight};margin-top:${node.attrs.spacingBefore}pt;margin-bottom:${node.attrs.spacingAfter}pt`,
      },
      0,
    ],
  })
  .update("heading", {
    ...basicSchema.spec.nodes.get("heading"),
    attrs: {
      level: { default: 1 },
      align: { default: "left" },
      indent: { default: 0 },
      lineHeight: { default: 1.5 },
      spacingBefore: { default: 0 },
      spacingAfter: { default: 0 },
    },
    parseDOM: [
      {
        tag: "h1",
        attrs: { level: 1 },
        getAttrs: (dom) => {
          const el = dom as HTMLElement;
          const mt = el.style.marginTop?.replace("pt", "");
          const mb = el.style.marginBottom?.replace("pt", "");
          return {
            level: 1,
            spacingBefore: mt ? Number(mt) || 0 : 0,
            spacingAfter: mb ? Number(mb) || 0 : 0,
          };
        },
      },
      {
        tag: "h2",
        attrs: { level: 2 },
        getAttrs: (dom) => {
          const el = dom as HTMLElement;
          const mt = el.style.marginTop?.replace("pt", "");
          const mb = el.style.marginBottom?.replace("pt", "");
          return { level: 2, spacingBefore: mt ? Number(mt) || 0 : 0, spacingAfter: mb ? Number(mb) || 0 : 0 };
        },
      },
      {
        tag: "h3",
        attrs: { level: 3 },
        getAttrs: (dom) => {
          const el = dom as HTMLElement;
          const mt = el.style.marginTop?.replace("pt", "");
          const mb = el.style.marginBottom?.replace("pt", "");
          return { level: 3, spacingBefore: mt ? Number(mt) || 0 : 0, spacingAfter: mb ? Number(mb) || 0 : 0 };
        },
      },
      {
        tag: "h4",
        attrs: { level: 4 },
        getAttrs: (dom) => {
          const el = dom as HTMLElement;
          const mt = el.style.marginTop?.replace("pt", "");
          const mb = el.style.marginBottom?.replace("pt", "");
          return { level: 4, spacingBefore: mt ? Number(mt) || 0 : 0, spacingAfter: mb ? Number(mb) || 0 : 0 };
        },
      },
      {
        tag: "h5",
        attrs: { level: 5 },
        getAttrs: (dom) => {
          const el = dom as HTMLElement;
          const mt = el.style.marginTop?.replace("pt", "");
          const mb = el.style.marginBottom?.replace("pt", "");
          return { level: 5, spacingBefore: mt ? Number(mt) || 0 : 0, spacingAfter: mb ? Number(mb) || 0 : 0 };
        },
      },
      {
        tag: "h6",
        attrs: { level: 6 },
        getAttrs: (dom) => {
          const el = dom as HTMLElement;
          const mt = el.style.marginTop?.replace("pt", "");
          const mb = el.style.marginBottom?.replace("pt", "");
          return { level: 6, spacingBefore: mt ? Number(mt) || 0 : 0, spacingAfter: mb ? Number(mb) || 0 : 0 };
        },
      },
    ],
    toDOM: (node) => [
      `h${node.attrs.level}`,
      {
        style: `text-align:${node.attrs.align};margin-left:${node.attrs.indent}em;line-height:${node.attrs.lineHeight};margin-top:${node.attrs.spacingBefore}pt;margin-bottom:${node.attrs.spacingAfter}pt`,
      },
      0,
    ],
  })
  .addToEnd("table", {
    group: "block",
    content: "table_row+",
    tableRole: "table",
    isolating: true,
    toDOM: () => [
      "table",
      { style: "border-collapse:collapse;width:100%;margin:8px 0" },
      ["tbody", 0],
    ],
  })
  .addToEnd("table_row", {
    content: "table_cell+",
    tableRole: "row",
    toDOM: () => ["tr", 0],
  })
  .addToEnd("table_cell", {
    content: "block+",
    attrs: {
      header: { default: false },
      colspan: { default: 1 },
      rowspan: { default: 1 },
      colwidth: { default: null },
    },
    tableRole: "cell",
    isolating: true,
    parseDOM: [
      {
        tag: "td",
        getAttrs: (dom) => {
          const el = dom as HTMLElement;
          const widthAttr = el.getAttribute("data-colwidth");
          const widths = widthAttr
            ? widthAttr.split(",").map((s) => Number(s) || 0)
            : el.style.width
              ? [parseInt(el.style.width, 10)]
              : null;
          return {
            header: false,
            colspan: Number(el.getAttribute("colspan") || 1),
            rowspan: Number(el.getAttribute("rowspan") || 1),
            colwidth: widths && widths.some((w) => w > 0) ? widths : null,
          };
        },
      },
      {
        tag: "th",
        getAttrs: (dom) => {
          const el = dom as HTMLElement;
          const widthAttr = el.getAttribute("data-colwidth");
          const widths = widthAttr
            ? widthAttr.split(",").map((s) => Number(s) || 0)
            : el.style.width
              ? [parseInt(el.style.width, 10)]
              : null;
          return {
            header: true,
            colspan: Number(el.getAttribute("colspan") || 1),
            rowspan: Number(el.getAttribute("rowspan") || 1),
            colwidth: widths && widths.some((w) => w > 0) ? widths : null,
          };
        },
      },
    ],
    toDOM: (node) => {
      const widths = node.attrs.colwidth as number[] | null;
      const styleParts = [
        "border:1px solid #dadce0",
        "padding:6px 8px",
        "min-width:48px",
        "vertical-align:top",
      ];
      if (widths?.[0]) styleParts.push(`width:${widths[0]}px`);
      return [
        node.attrs.header ? "th" : "td",
        {
          style: styleParts.join(";"),
          colspan: node.attrs.colspan > 1 ? node.attrs.colspan : undefined,
          rowspan: node.attrs.rowspan > 1 ? node.attrs.rowspan : undefined,
          "data-colwidth": widths?.length ? widths.join(",") : undefined,
        },
        0,
      ];
    },
  })
  .addToEnd("page_break", pageBreakSpec);

const imageBase = basicSchema.spec.nodes.get("image")!;
const imageAttrs = imageBase.attrs ?? {};
const nodesWithImage = styledNodes.update("image", {
  ...imageBase,
  attrs: {
    src: imageAttrs.src ?? {},
    alt: { default: null },
    title: { default: null },
    width: { default: null },
    height: { default: null },
  },
  parseDOM: [
    {
      tag: "img[src]",
      getAttrs(dom) {
        const el = dom as HTMLElement;
        const w = el.getAttribute("width") || el.style.width;
        const h = el.getAttribute("height") || el.style.height;
        return {
          src: el.getAttribute("src"),
          alt: el.getAttribute("alt"),
          title: el.getAttribute("title"),
          width: w ? Number.parseInt(String(w), 10) || null : null,
          height: h ? Number.parseInt(String(h), 10) || null : null,
        };
      },
    },
  ],
  toDOM(node) {
    const style: string[] = [];
    if (node.attrs.width) style.push(`width:${node.attrs.width}px`);
    if (node.attrs.height) style.push(`height:${node.attrs.height}px`);
    return [
      "img",
      {
        src: node.attrs.src,
        alt: node.attrs.alt || "",
        style: style.length ? style.join(";") : undefined,
      },
    ];
  },
});

const mySchema = new Schema({
  nodes: nodesWithImage,
  marks: customMarks,
});

function buildDocJson(doc: Node, setup: PageSetupConfig): DocContent {
  return {
    ...(doc.toJSON() as DocContent),
    pageSetup: setup,
  };
}

function docJsonForCompare(json: DocContent): string {
  const copy = { ...json };
  delete copy.pageSetup;
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

function findPlugin(getQuery: () => string, getMatchCase: () => boolean, getActive: () => number) {
  return new Plugin({
    props: {
      decorations(state) {
        const query = getQuery();
        if (!query) return null;
        const matches = collectMatches(state.doc, query, getMatchCase());
        const active = getActive();
        const decos = matches.map((m, i) =>
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
  const [headings, setHeadings] = createSignal<{ level: number; text: string; pos: number }[]>([]);
  const [linkDialogOpen, setLinkDialogOpen] = createSignal(false);
  const [linkHref, setLinkHref] = createSignal("https://");
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

  const [pageSetupOpen, setPageSetupOpen] = createSignal(false);
  const [pageSetup, setPageSetup] = createSignal<PageSetupConfig>({
    margins: { top: 1, bottom: 1, left: 1, right: 1 },
    orientation: "portrait",
    paperSize: "letter"
  });

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

  const refreshStatus = (json: DocContent) => {
    const paper = paperDimensions();
    const config = pageSetup();
    const topMarginPx = (config.margins.top || 1) * 96;
    const bottomMarginPx = (config.margins.bottom || 1) * 96;
    const contentPageHeight = Math.max(300, paper.height - topMarginPx - bottomMarginPx);

    const domHeight = editorRef ? editorRef.scrollHeight : 0;
    const estimatedPages = Math.max(1, Math.ceil(domHeight / contentPageHeight));
    const explicitPageBreaks = (JSON.stringify(json).match(/"type"\s*:\s*"page_break"/g) || []).length;
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

    commands.computeDocWordCount(json).then((wc) => {
      props.onWordCountChange?.(
        `Page ${curPage} of ${pages} · ${wc.words} words, ${wc.characters} characters`
      );
    });
  };

  const emitDocChange = (doc: Node, setup: PageSetupConfig) => {
    if (!props.onChange) return;
    const payload = buildDocJson(doc, setup);
    const serialized = JSON.stringify(payload);
    if (serialized === lastEmittedJson) return;
    lastEmittedJson = serialized;
    props.onChange(payload);
  };

  onMount(() => {
    const initial = props.initialContent;
    if (initial?.pageSetup && typeof initial.pageSetup === "object") {
      setPageSetup(initial.pageSetup as PageSetupConfig);
    }

    let docNode;
    try {
      const docJson = { ...(initial || {}) } as DocContent;
      delete docJson.pageSetup;
      docNode = mySchema.nodeFromJSON(
        docJson.type ? docJson : { type: "doc", content: [{ type: "paragraph", content: [] }] }
      );
    } catch {
      docNode = mySchema.node("doc", null, [mySchema.node("paragraph")]);
    }

    const state = EditorState.create({
      doc: docNode,
      plugins: [
        history(),
        columnResizing({ handleWidth: 5, cellMinWidth: 48 }),
        tableEditing(),
        findPlugin(findQuery, matchCase, matchIndex),
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

        const json = buildDocJson(newState.doc, pageSetup());
        refreshStatus(json);

        if (transaction.docChanged) {
          refreshHeadings(newState.doc);
          emitDocChange(newState.doc, pageSetup());
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
    });

    refreshStatus(buildDocJson(view.state.doc, pageSetup()));
    refreshHeadings(view.state.doc);
    syncToolbarFromSelection(view.state, setFontFamily, setFontSize, setTextColor, setHighlightColor);
  });

  createEffect(() => {
    const content = props.initialContent;
    if (!view || !content) return;
    if (JSON.stringify(content) === lastEmittedJson) return;
    try {
      if (content.pageSetup && typeof content.pageSetup === "object") {
        setPageSetup(content.pageSetup as PageSetupConfig);
      }
      const docJson = { ...content } as DocContent;
      delete docJson.pageSetup;
      const nextDoc = mySchema.nodeFromJSON(docJson);
      if (docJsonForCompare(content) !== docJsonForCompare(view.state.doc.toJSON() as DocContent)) {
        const tr = view.state.tr.replaceWith(0, view.state.doc.content.size, nextDoc.content);
        if (tr.docChanged) {
          view.dispatch(tr);
          refreshStatus(buildDocJson(view.state.doc, pageSetup()));
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

  const applyLink = () => {
    const href = linkHref().trim();
    if (!href) return;
    withView((v) => toggleMark(mySchema.marks.link, { href, title: null })(v.state, v.dispatch));
    setLinkDialogOpen(false);
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
      const rowType = mySchema.nodes.table_row;
      const tableType = mySchema.nodes.table;
      if (!cellType || !rowType || !tableType) return;
      const built = Array.from({ length: rows }, (_, r) =>
        rowType.create(
          null,
          Array.from({ length: cols }, () =>
            cellType.createAndFill({ header: r === 0 })!
          )
        )
      );
      v.dispatch(v.state.tr.replaceSelectionWith(tableType.create(null, built)));
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
    withView((v) => {
      const { tablePos } = findTableContext(v);
      if (tablePos == null) return;
      const table = v.state.doc.nodeAt(tablePos);
      if (!table || table.childCount === 0) return;
      const firstRow = table.child(0);
      let tr = v.state.tr;
      let cellPos = tablePos + 1 + 1;
      for (let c = 0; c < firstRow.childCount; c++) {
        const cell = firstRow.child(c);
        tr = tr.setNodeMarkup(cellPos, undefined, {
          ...cell.attrs,
          header: !cell.attrs.header,
        });
        cellPos += cell.nodeSize;
      }
      if (tr.docChanged) v.dispatch(tr);
    });
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
          setLinkDialogOpen(true);
          break;
        case "insert-page-break":
          insertPageBreak();
          break;
        case "merge-cells":
          execMergeCells();
          break;
        case "split-cell":
          execSplitCell();
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
    commands.searchDocText(view.state.doc.toJSON(), findQuery(), matchCase()).then((m) => {
      setMatchCount(m.length);
    });
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
          {[
            ["paragraph", "Default Paragraph Style"],
            ["heading1", "Heading 1"],
            ["heading2", "Heading 2"],
            ["heading3", "Heading 3"],
            ["blockquote", "Block Quote"],
            ["code_block", "Preformatted Text"],
          ].map(([value, label]) => (
            <button
              type="button"
              class="g-toolbar-btn"
              style={{ "justify-content": "flex-start", width: "100%", height: "auto", padding: "4px 6px" }}
              onClick={() => execBlockType(value)}
            >
              {label}
            </button>
          ))}
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
  ];

  const zoom = () => props.zoomLevel ?? 100;

  return (
    <div style={{ display: "flex", "flex-direction": "column", height: "100%", background: "var(--bg-canvas)", overflow: "hidden" }}>
      <DocToolbar
        onRequestNew={props.onRequestNew}
        onRequestOpen={props.onRequestOpen}
        onRequestSave={props.onRequestSave}
        onRequestExportPdf={props.onRequestExportPdf}
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
        onInsertLink={() => setLinkDialogOpen(true)}
        onInsertPageBreak={insertPageBreak}
        onAddTableRow={addTableRow}
        onDeleteTableRow={deleteTableRow}
        onAddTableColumn={addTableColumn}
        onDeleteTableColumn={deleteTableColumn}
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
          {pageSetup().header
            ?.replace(/{pages}/g, String(pageCount()))
            ?.replace(/{total}/g, String(pageCount()))
            ?.replace(/{page}/g, "1")}
        </div>
      </Show>
      <Show when={pageSetup().footer}>
        <div class="doc-print-footer">
          {pageSetup().footer
            ?.replace(/{pages}/g, String(pageCount()))
            ?.replace(/{total}/g, String(pageCount()))
            ?.replace(/{page}/g, "1")}
        </div>
      </Show>

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
              style={{ "min-height": "800px", outline: "none", "font-family": "Liberation Serif, serif", "font-size": "12pt" }}
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
                  { id: "insert-table", label: "Insert Table", action: () => emitEditorCommand("insert-table") },
                  { id: "merge-cells", label: "Merge cells", action: () => emitEditorCommand("merge-cells") },
                  { id: "split-cell", label: "Split cell", action: () => emitEditorCommand("split-cell") },
                  { id: "sep3", label: "", separator: true },
                  { id: "find", label: "Find…", action: () => emitEditorCommand("find") },
                ];
                setContextMenu({ x: event.clientX, y: event.clientY, items });
              }}
            />
            <Show when={selectedImage() && view}>
              <div
                role="button"
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
          <div style={{ display: "flex", gap: "8px", "justify-content": "flex-end" }}>
            <button type="button" class="g-toolbar-btn" onClick={() => setLinkDialogOpen(false)}>Cancel</button>
            <button type="button" class="g-toolbar-btn active" onClick={applyLink}>Apply</button>
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
        onLink={() => setLinkDialogOpen(true)}
      />

      <PageSetupDialog
        open={pageSetupOpen()}
        onClose={() => setPageSetupOpen(false)}
        config={pageSetup()}
        onApply={(config) => {
          setPageSetup(config);
          if (view) emitDocChange(view.state.doc, config);
        }}
      />

      <Show when={printPreviewOpen()}>
        <PrintPreview
          content={editorRef?.innerHTML || ""}
          onClose={() => setPrintPreviewOpen(false)}
        />
      </Show>
    </div>
  );
}
