import { Schema, MarkSpec, NodeSpec } from "prosemirror-model";
import { schema as basicSchema } from "prosemirror-schema-basic";
import { addListNodes } from "prosemirror-schema-list";
import { tableNodes } from "prosemirror-tables";

/** Persist as bold/italic so DOCX/PDF exporters match. */
export const customMarks: Record<string, MarkSpec> = {
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
  trackInsert: {
    attrs: {
      author: { default: "You" },
      changeId: { default: "" },
      createdAt: { default: "" },
    },
    excludes: "trackDelete",
    parseDOM: [{ tag: 'span[data-track-change="insert"]' }],
    toDOM: (mark) => ["span", {
      "data-track-change": "insert",
      "data-change-id": mark.attrs.changeId || undefined,
      style: "background:color-mix(in srgb, #22c55e 18%, transparent);border-bottom:2px solid #22c55e;",
    }, 0],
  },
  trackDelete: {
    attrs: {
      author: { default: "You" },
      changeId: { default: "" },
      createdAt: { default: "" },
    },
    excludes: "trackInsert",
    parseDOM: [{ tag: 'span[data-track-change="delete"]' }],
    toDOM: (mark) => ["span", {
      "data-track-change": "delete",
      "data-change-id": mark.attrs.changeId || undefined,
      style: "background:color-mix(in srgb, #ef4444 14%, transparent);color:#b91c1c;text-decoration:line-through;",
    }, 0],
  },
};

export const pageBreakSpec: NodeSpec = {
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

export const styledNodes = addListNodes(basicSchema.spec.nodes, "paragraph block*", "block")
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
  });

export const baseNodes = styledNodes.addToEnd("page_break", pageBreakSpec);
export const nodesWithTables = baseNodes.append(tableNodes({ tableGroup: "block", cellContent: "block+", cellAttributes: {} }));


const imageBase = basicSchema.spec.nodes.get("image")!;
const imageAttrs = imageBase.attrs ?? {};
export const nodesWithImage = nodesWithTables.update("image", {
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

export const mySchema = new Schema({
  nodes: nodesWithImage,
  marks: customMarks,
});
