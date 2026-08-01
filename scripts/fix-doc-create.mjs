import fs from "fs";
const path = new URL("../packages/doc-editor/src/DocEditor.tsx", import.meta.url);
let c = fs.readFileSync(path, "utf8");
const before = c.includes("return Decoration.create");
c = c.replaceAll("return Decrations.create(state.doc, decos);", "return DecorationSet.create(state.doc, decos);");
// Also handle if it's Decoration.create without s (singular class misuse)
c = c.replaceAll("return Decoration.create(state.doc, decos);", "return DecorationSet.create(state.doc, decos);");
fs.writeFileSync(path, c);
console.log({ before, after: c.includes("return DecorationSet.create") });
