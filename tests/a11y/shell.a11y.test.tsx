import { render } from "solid-js/web";
import { afterEach, describe, expect, it } from "vitest";
import { Dialog } from "@redoc/ui";
import { CommandPalette, FindBar, shortcutRegistry } from "@redoc/editor-common";
import { HomeScreen } from "../../apps/desktop/src/HomeScreen";
import { expectNoSeriousA11yViolations } from "./runAxe";

const disposers: Array<() => void> = [];

function mount(ui: () => unknown) {
  const container = document.createElement("div");
  document.body.appendChild(container);
  const dispose = render(() => ui() as never, container);
  disposers.push(() => {
    dispose();
    container.remove();
  });
  return container;
}

afterEach(() => {
  while (disposers.length) disposers.pop()?.();
});

describe("a11y shell scans", () => {
  it("HomeScreen shell markup has no serious/critical axe violations", async () => {
    mount(() => (
      <HomeScreen
        recents={[]}
        onNewDoc={() => undefined}
        onOpenFile={() => undefined}
        onOpenRecent={() => undefined}
        onTogglePin={() => undefined}
      />
    ));
    await expectNoSeriousA11yViolations();
  });

  it("Dialog open has no serious/critical axe violations", async () => {
    mount(() => (
      <Dialog open title="Settings" onClose={() => undefined}>
        <p>Adjust preferences for Redoc.</p>
        <button type="button">Save</button>
      </Dialog>
    ));
    await expectNoSeriousA11yViolations();
  });

  it("CommandPalette open has no serious/critical axe violations", async () => {
    shortcutRegistry.register({
      id: "a11y-test-command",
      title: "Accessibility test command",
      action: () => undefined,
    });
    try {
      mount(() => <CommandPalette open onClose={() => undefined} />);
      // Placeholder can satisfy axe name computation; require the explicit aria-label contract.
      const search = document.querySelector<HTMLInputElement>(
        'input[placeholder="Type a command or search..."]',
      );
      expect(search?.getAttribute("aria-label")).toBe("Command search");
      await expectNoSeriousA11yViolations();
    } finally {
      shortcutRegistry.unregister("a11y-test-command");
    }
  });

  it("FindBar has no serious/critical axe violations", async () => {
    mount(() => (
      <FindBar
        query="hello"
        replaceWith=""
        matchCount={2}
        matchIndex={0}
        matchCase={false}
        showReplace
        onQueryChange={() => undefined}
        onReplaceChange={() => undefined}
        onFindNext={() => undefined}
        onFindPrev={() => undefined}
        onReplace={() => undefined}
        onReplaceAll={() => undefined}
        onMatchCaseChange={() => undefined}
        onClose={() => undefined}
      />
    ));
    await expectNoSeriousA11yViolations();
  });
});
