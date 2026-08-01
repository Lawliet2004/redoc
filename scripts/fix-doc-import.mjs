import fs from "fs";
const path = new URL("../packages/doc-editor/src/DocEditor.tsx", import.meta.url);
let c = fs.readFileSync(path, "utf8");
const lines = c.split(/\r?\n/);
console.log("BEFORE:", JSON.stringify(lines[2]));
// Decoration (class) + DecorationSet (collection)
lines[2] = "import { " + "Decoration" + ", " + "DecorationSet" + ", EditorView } from \"prosemirror-view\";";
c = lines.join("\n");
c = c.split("return Decorations.create").join("return DecorationSet.create");
c = c.split("Decorations.inline").join("Decoration.inline");
fs.writeFileSync(path, c);
console.log("AFTER:", JSON.stringify(fs.readFileSync(path, "utf8").split(/\r?\n/)[2]));
