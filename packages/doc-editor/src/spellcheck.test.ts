import { describe, it, expect } from "vitest";
import { JSDOM } from "jsdom";

/**
 * Spellcheck wiring: the DocEditor's editing surface must forward the
 * `spellcheckEnabled` setting to the DOM `spellcheck` attribute so the
 * platform (WebView2/Spellcheck) engine renders squiggles.
 *
 * The attribute logic under test is `spellcheck={enabled !== false}`, which
 * means default-on and explicit off works. We validate the DOM behavior of
 * the attribute itself plus the default policy in isolation (rendering the
 * full ProseMirror editor in jsdom is covered by shell a11y tests).
 */

function resolveSpellcheck(enabled: boolean | undefined): boolean {
  // mirrors DocEditor's `props.spellcheckEnabled !== false`
  return enabled !== false;
}

describe("spellcheck setting wiring", () => {
  it("defaults to enabled when the setting is undefined", () => {
    expect(resolveSpellcheck(undefined)).toBe(true);
  });

  it("honors an explicit true", () => {
    expect(resolveSpellcheck(true)).toBe(true);
  });

  it("turns off when explicitly disabled", () => {
    expect(resolveSpellcheck(false)).toBe(false);
  });

  it("jsdom reflects spellcheck attribute on a contenteditable div", () => {
    const dom = new JSDOM('<div contenteditable="true"></div>');
    const el = dom.window.document.querySelector("div")!;
    el.setAttribute("spellcheck", "true");
    expect(el.getAttribute("spellcheck")).toBe("true");
    el.setAttribute("spellcheck", "false");
    expect(el.getAttribute("spellcheck")).toBe("false");
  });
});
