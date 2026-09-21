import { describe, expect, it } from "vitest";
import { ShortcutRegistry, matchShortcut } from "./ShortcutRegistry";

function keyEvent(init: Partial<KeyboardEventInit> & { key: string; code?: string }): KeyboardEvent {
  return new KeyboardEvent("keydown", {
    ctrlKey: false,
    shiftKey: false,
    altKey: false,
    metaKey: false,
    ...init,
    ...(init.code ? {} : {}),
  });
}

describe("ShortcutRegistry", () => {
  it("includes registered commands with menuPath in buildMenus", () => {
    const registry = new ShortcutRegistry();
    let ran = false;
    registry.register({
      id: "test-open",
      title: "Test Open",
      shortcut: "Ctrl+O",
      mode: "global",
      menuPath: ["File", "Test Open…"],
      action: () => {
        ran = true;
      },
    });

    const menus = registry.buildMenus("home");
    const fileMenu = menus.find((menu) => menu.label === "File");
    expect(fileMenu).toBeDefined();
    expect(fileMenu?.items.some((item) => item.id === "test-open" && item.label === "Test Open…")).toBe(true);

    registry.run("test-open");
    expect(ran).toBe(true);
  });

  it("filters sheet-only menus when not in sheet mode", () => {
    const registry = new ShortcutRegistry();
    registry.register({
      id: "sheet-only",
      title: "Sheet Action",
      mode: "sheet",
      menuPath: ["Sheet", "Do Thing"],
      action: () => {},
    });

    expect(registry.buildMenus("doc").some((menu) => menu.label === "Sheet")).toBe(false);
    expect(registry.buildMenus("sheet").some((menu) => menu.label === "Sheet")).toBe(true);
  });

  it("matches zoom shortcuts via physical key codes", () => {
    expect(matchShortcut(keyEvent({ key: "=", code: "Equal", ctrlKey: true }), "Ctrl+=")).toBe(true);
    expect(matchShortcut(keyEvent({ key: "-", code: "Minus", ctrlKey: true }), "Ctrl+-")).toBe(true);
    expect(matchShortcut(keyEvent({ key: "0", code: "Digit0", ctrlKey: true }), "Ctrl+0")).toBe(true);
    // No accidental match without modifiers.
    expect(matchShortcut(keyEvent({ key: "=", code: "Equal" }), "Ctrl+=")).toBe(false);
    // Ctrl+Shift+= produces e.key "+"; the shifted glyph satisfies the declared
    // base key so "Ctrl+=" still fires (browser zoom-in convention), as do
    // equivalent "Ctrl+Shift+=" / "Ctrl++" declarations.
    expect(matchShortcut(keyEvent({ key: "+", code: "Equal", ctrlKey: true, shiftKey: true }), "Ctrl+=")).toBe(true);
    expect(matchShortcut(keyEvent({ key: "+", code: "Equal", ctrlKey: true, shiftKey: true }), "Ctrl+Shift+=")).toBe(true);
    expect(matchShortcut(keyEvent({ key: "+", code: "Equal", ctrlKey: true, shiftKey: true }), "Ctrl++")).toBe(true);
    // A declared unshifted key never fires without its required modifiers.
    expect(matchShortcut(keyEvent({ key: "=", code: "Equal", shiftKey: true }), "Ctrl+=")).toBe(false);
    // Shift on letters stays strict: Ctrl+Shift+A is not Ctrl+A.
    expect(matchShortcut(keyEvent({ key: "A", code: "KeyA", ctrlKey: true, shiftKey: true }), "Ctrl+A")).toBe(false);
    expect(matchShortcut(keyEvent({ key: "A", code: "KeyA", ctrlKey: true, shiftKey: true }), "Ctrl+Shift+A")).toBe(true);
  });
});
