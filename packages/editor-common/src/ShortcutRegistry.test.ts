import { describe, expect, it } from "vitest";
import { ShortcutRegistry } from "./ShortcutRegistry";

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
});
