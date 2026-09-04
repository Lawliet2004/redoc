/** F6 pane-cycle helper: moves focus across [data-pane] landmarks. */
export const PANE_ORDER = ["toolbar", "canvas", "inspector", "status"] as const;

export type PaneId = (typeof PANE_ORDER)[number];

function focusablesIn(root: HTMLElement): HTMLElement[] {
  return Array.from(
    root.querySelectorAll<HTMLElement>(
      'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
    ),
  ).filter((el) => el.offsetParent !== null || el === document.activeElement);
}

export function cyclePane(direction: 1 | -1 = 1): PaneId | null {
  const panes = Array.from(document.querySelectorAll<HTMLElement>("[data-pane]"));
  if (!panes.length) return null;
  const active = document.activeElement as HTMLElement | null;
  const current = active?.closest?.("[data-pane]") as HTMLElement | null;
  const currentId = current?.dataset.pane as PaneId | undefined;
  let idx = currentId ? PANE_ORDER.indexOf(currentId) : -1;
  // Fall back to DOM order when panes are not in PANE_ORDER.
  const ordered = PANE_ORDER.map((id) => panes.find((p) => p.dataset.pane === id)).filter(
    Boolean,
  ) as HTMLElement[];
  const list = ordered.length ? ordered : panes;
  let pos = current ? list.indexOf(current) : idx;
  if (pos < 0) pos = direction === 1 ? -1 : 0;
  const next = list[(pos + direction + list.length) % list.length];
  if (!next) return null;
  const targets = focusablesIn(next);
  const target = targets[0] ?? next;
  if (!target.hasAttribute("tabindex") && target === next) next.tabIndex = -1;
  target.focus();
  return (next.dataset.pane as PaneId) ?? null;
}

export function handleF6(event: KeyboardEvent, direction: 1 | -1 = 1): boolean {
  if (event.key !== "F6") return false;
  event.preventDefault();
  cyclePane(event.shiftKey ? -1 : direction);
  return true;
}
