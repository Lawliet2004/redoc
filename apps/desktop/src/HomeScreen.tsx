import { createSignal, createMemo, createEffect, For, Show } from "solid-js";
import {
  IconDoc,
  IconSheet,
  IconSlide,
  IconFolderOpen,
  IconPin,
  IconSearch,
} from "@redoc/icons";
import { t } from "@redoc/ui";
import { ContextMenu, ContextMenuItem } from "@redoc/editor-common";
import { commands, RecentEntry } from "@redoc/api-client";
import { formatDate } from "@redoc/utils";

interface HomeScreenProps {
  recents: RecentEntry[];
  onNewDoc: (mode: "doc" | "sheet" | "slide", templateId?: string) => void;
  onOpenFile: () => void;
  onOpenRecent: (entry: RecentEntry) => void;
  onTogglePin: (id: string) => void;
  onDropFile?: (file: File) => void;
}

const ONBOARDED_KEY = "redoc-onboarded";

function shouldShowOnboarding(recentsCount: number): boolean {
  if (recentsCount > 0) return false;
  try {
    return window.localStorage.getItem(ONBOARDED_KEY) !== "1";
  } catch {
    return false;
  }
}

function markOnboarded() {
  try {
    window.localStorage.setItem(ONBOARDED_KEY, "1");
  } catch {
    /* storage unavailable */
  }
}

type RailId = "new" | "recent" | "shared" | "templates";

function modeAccentVar(mode: string): string {
  return mode === "sheet"
    ? "var(--sheet-accent)"
    : mode === "slide"
      ? "var(--slide-accent)"
      : "var(--doc-accent)";
}

function TemplateTile(props: {
  label: string;
  sublabel: string;
  accent: string;
  iconBg: string;
  icon: any;
  onClick: () => void;
}) {
  const Icon = props.icon;
  return (
    <button type="button" class="home-tile" onClick={props.onClick}>
      <div class="home-tile-canvas" style={{ "--tile-accent": props.accent }}>
        <div class="home-tile-icon" style={{ background: props.iconBg, color: props.accent }}>
          <Icon width="32" height="32" />
        </div>
      </div>
      <div>
        <div class="home-tile-label">{props.label}</div>
        <div class="home-tile-sublabel">{props.sublabel}</div>
      </div>
    </button>
  );
}

function HeroCard(props: {
  label: string;
  sublabel: string;
  accent: string;
  icon: any;
  onClick: () => void;
}) {
  const Icon = props.icon;
  return (
    <button
      type="button"
      class="app-hero-card"
      style={{ "--card-accent": props.accent }}
      onClick={props.onClick}
    >
      <span class="app-hero-icon" aria-hidden="true">
        <Icon width="26" height="26" />
      </span>
      <span class="app-hero-text">
        <span class="app-hero-label">{props.label}</span>
        <span class="app-hero-sub">{props.sublabel}</span>
      </span>
    </button>
  );
}

function QuickTile(props: {
  label: string;
  accent: string;
  icon: any;
  onClick: () => void;
}) {
  const Icon = props.icon;
  return (
    <button
      type="button"
      class="app-quick-tile"
      style={{ "--tile-accent": props.accent }}
      onClick={props.onClick}
    >
      <Icon width="14" height="14" />
      <span>{props.label}</span>
    </button>
  );
}

const BLANKS = (onNewDoc: HomeScreenProps["onNewDoc"]) => [
  { label: t("home.tiles.blankDoc"), sub: t("home.cardSub.doc"), accent: "var(--doc-accent)", icon: IconDoc, run: () => onNewDoc("doc") },
  { label: t("home.tiles.blankSheet"), sub: t("home.cardSub.sheet"), accent: "var(--sheet-accent)", icon: IconSheet, run: () => onNewDoc("sheet") },
  { label: t("home.tiles.blankDeck"), sub: t("home.cardSub.slide"), accent: "var(--slide-accent)", icon: IconSlide, run: () => onNewDoc("slide") },
];

const TEMPLATES = (onNewDoc: HomeScreenProps["onNewDoc"]) => [
  { label: t("home.tiles.resume"), sub: t("home.tileKind.docs"), accent: "var(--doc-accent)", bg: "var(--g-blue-light)", icon: IconDoc, run: () => onNewDoc("doc", "resume") },
  { label: t("home.tiles.report"), sub: t("home.tileKind.docs"), accent: "var(--doc-accent)", bg: "var(--g-blue-light)", icon: IconDoc, run: () => onNewDoc("doc", "report") },
  { label: t("home.tiles.budget"), sub: t("home.tileKind.sheets"), accent: "var(--sheet-accent)", bg: "var(--g-green-light)", icon: IconSheet, run: () => onNewDoc("sheet", "budget") },
  { label: t("home.tiles.plan"), sub: t("home.tileKind.sheets"), accent: "var(--sheet-accent)", bg: "var(--g-green-light)", icon: IconSheet, run: () => onNewDoc("sheet", "project_plan") },
  { label: t("home.tiles.pitch"), sub: t("home.tileKind.slides"), accent: "var(--slide-accent)", bg: "var(--g-yellow-light)", icon: IconSlide, run: () => onNewDoc("slide", "pitch_deck") },
];

export function HomeScreen(props: HomeScreenProps) {
  const [searchQuery, setSearchQuery] = createSignal("");
  const [rail, setRail] = createSignal<RailId>("recent");
  const [filterCategory, setFilterCategory] = createSignal("All");
  const [sortBy, setSortBy] = createSignal("Last opened");
  const [selectedId, setSelectedId] = createSignal<string | null>(null);
  const [fileExists, setFileExists] = createSignal<Record<string, boolean>>({});
  const [rowMenu, setRowMenu] = createSignal<{ x: number; y: number; entry: RecentEntry } | null>(null);
  const [showWelcome, setShowWelcome] = createSignal(shouldShowOnboarding(props.recents.length));

  const checkMissing = async () => {
    const recents = props.recents;
    const known = fileExists();
    const pending = recents.filter((r) => r.path && known[r.id] === undefined);
    if (!pending.length) return;
    try {
      const flags = await commands.pathsExist(pending.map((r) => r.path!));
      const updates = { ...known };
      for (let i = 0; i < pending.length; i++) updates[pending[i].id] = flags[i] ?? false;
      setFileExists(updates);
    } catch {
      const updates = { ...known };
      for (const r of pending) updates[r.id] = false;
      setFileExists(updates);
    }
  };
  // Reactive: recents arrive asynchronously after mount, so re-run whenever
  // the list (or the known-existence map) changes. checkMissing() early-returns
  // once every entry has a known flag, so this settles without looping.
  createEffect(() => {
    void checkMissing();
  });

  const filteredRecents = createMemo(() => {
    let list = props.recents;
    const cat = filterCategory();
    if (cat === "Documents") list = list.filter((r) => r.mode === "doc");
    else if (cat === "Spreadsheets") list = list.filter((r) => r.mode === "sheet");
    else if (cat === "Presentations") list = list.filter((r) => r.mode === "slide");
    else if (cat === "Pinned") list = list.filter((r) => r.pinned);
    if (rail() === "shared") return [];
    const q = searchQuery().toLowerCase().trim();
    if (q) list = list.filter((r) => r.title?.toLowerCase().includes(q) || r.path?.toLowerCase().includes(q));
    const sorted = [...list];
    if (sortBy() === "Title") sorted.sort((a, b) => a.title.localeCompare(b.title));
    else if (sortBy() === "Mode") sorted.sort((a, b) => a.mode.localeCompare(b.mode));
    else sorted.sort((a, b) => new Date(b.lastOpenedAt).getTime() - new Date(a.lastOpenedAt).getTime());
    return sorted;
  });

  const selected = createMemo(() => {
    const list = filteredRecents();
    if (!list.length) return null;
    return list.find((r) => r.id === selectedId()) ?? list[0];
  });

  const modeIcon = (mode: string) =>
    mode === "sheet" ? <IconSheet color="var(--sheet-accent)" /> : mode === "slide" ? <IconSlide color="var(--slide-accent)" /> : <IconDoc color="var(--doc-accent)" />;

  const blanks = BLANKS(props.onNewDoc);
  const templates = TEMPLATES(props.onNewDoc);

  const rowMenuItems = (entry: RecentEntry): ContextMenuItem[] => [
    {
      id: "open",
      label: t("home.previewOpen"),
      disabled: fileExists()[entry.id] === false,
      action: () => props.onOpenRecent(entry),
    },
    {
      id: "copy-path",
      label: t("home.rowMenu.copyPath"),
      disabled: !entry.path,
      action: () => {
        if (entry.path) void navigator.clipboard?.writeText(entry.path);
      },
    },
    {
      id: "unpin",
      label: entry.pinned ? t("home.unpin") : t("home.pin"),
      action: () => props.onTogglePin(entry.id),
    },
  ];

  const rails: Array<{ id: RailId; label: string }> = [
    { id: "recent", label: t("home.rail.recent") },
    { id: "new", label: t("home.rail.new") },
    { id: "templates", label: t("home.rail.templates") },
    { id: "shared", label: t("home.rail.shared") },
  ];

  return (
    <div
      class="home-root"
      onDragOver={(e) => e.preventDefault()}
      onDrop={(e) => {
        e.preventDefault();
        if (e.dataTransfer?.files?.length && props.onDropFile) props.onDropFile(e.dataTransfer.files[0]);
      }}
    >
      {/* Slim home topbar: brand left, search centered, actions right */}
      <div class="home-topbar app-topbar">
        <div class="home-brand">
          <div class="home-brand-badge app-brand-badge" aria-hidden="true">
            <IconDoc width="22" height="22" />
          </div>
          <div style={{ "min-width": "0" }}>
            <div class="home-brand-name">{t("home.brand")}</div>
            <div class="app-brand-tag">{t("home.tagline")}</div>
          </div>
        </div>
        <div class="home-search">
          <IconSearch color="var(--text-secondary)" />
          <input
            type="search"
            placeholder={t("home.searchPlaceholder")}
            aria-label={t("home.search")}
            value={searchQuery()}
            onInput={(e) => setSearchQuery(e.currentTarget.value)}
          />
        </div>
        <div class="app-topbar-actions">
          <div class="g-avatar-stack" role="group" aria-label={t("shell.titlebar.avatarStack")} title={t("shell.titlebar.avatarStack")}>
            <span class="g-avatar" style={{ background: "#5b9bd5" }} title="You (offline stub)">Y</span>
            <span class="g-avatar" style={{ background: "#70ad47" }} title="Teammate (offline stub)">T</span>
          </div>
          <button type="button" class="home-open-btn" onClick={props.onOpenFile}>
            <IconFolderOpen /> {t("home.open")}
          </button>
        </div>
      </div>

      {/* Centered max-width content column */}
      <div class="app-home-scroll">
        <div class="app-home-inner">
          <Show when={showWelcome()}>
            <div class="home-welcome app-welcome" role="region" aria-label={t("home.welcome.title")}>
              <div class="app-welcome-text">
                <h2 class="app-welcome-title">{t("home.welcome.title")}</h2>
                <p class="app-welcome-body">{t("home.welcome.body")}</p>
              </div>
              <div class="app-welcome-actions">
                <button type="button" class="home-open-btn" onClick={() => { markOnboarded(); setShowWelcome(false); props.onNewDoc("doc"); }}>
                  {t("home.tiles.blankDoc")}
                </button>
                <button type="button" class="home-open-btn" onClick={() => { markOnboarded(); setShowWelcome(false); props.onNewDoc("sheet"); }}>
                  {t("home.tiles.blankSheet")}
                </button>
                <button type="button" class="home-open-btn" onClick={() => { markOnboarded(); setShowWelcome(false); props.onNewDoc("slide"); }}>
                  {t("home.tiles.blankDeck")}
                </button>
                <button type="button" class="home-open-btn" onClick={() => { markOnboarded(); setShowWelcome(false); props.onNewDoc("doc", "resume"); }}>
                  {t("home.welcome.sample")}
                </button>
                <button type="button" class="g-icon-btn" aria-label={t("home.welcome.dismiss")} title={t("home.welcome.dismiss")} onClick={() => { markOnboarded(); setShowWelcome(false); }}>
                  ✕
                </button>
              </div>
            </div>
          </Show>

          {/* Section switcher */}
          <nav class="app-home-nav" aria-label="Home sections">
            <For each={rails}>
              {(item) => (
                <button
                  type="button"
                  class="app-nav-tab"
                  aria-pressed={rail() === item.id}
                  onClick={() => setRail(item.id)}
                >
                  {item.label}
                </button>
              )}
            </For>
          </nav>

          <div class="app-home-content">
            <main class="home-center app-main" aria-label="Documents">
              {/* New: three hero cards + compact template shortcuts */}
              <Show when={rail() === "new"}>
                <h2 class="home-section-title">{t("home.newHero")}</h2>
                <div class="app-hero-grid">
                  <For each={blanks}>
                    {(card) => (
                      <HeroCard label={card.label} sublabel={card.sub} accent={card.accent} icon={card.icon} onClick={card.run} />
                    )}
                  </For>
                </div>
                <h2 class="home-section-title">{t("home.templatesSection")}</h2>
                <div class="app-quick-row">
                  <For each={templates}>
                    {(tile) => <QuickTile label={tile.label} accent={tile.accent} icon={tile.icon} onClick={tile.run} />}
                  </For>
                </div>
              </Show>

              {/* Templates: full gallery grid (distinct from the New rail) */}
              <Show when={rail() === "templates"}>
                <h2 class="home-section-title">{t("home.templatesSection")}</h2>
                <p class="app-section-sub">{t("home.templatesSub")}</p>
                <div class="home-tile-grid">
                  <For each={templates}>
                    {(tile) => <TemplateTile label={tile.label} sublabel={tile.sub} accent={tile.accent} iconBg={tile.bg} icon={tile.icon} onClick={tile.run} />}
                  </For>
                </div>
              </Show>

              {/* Recent: quick-new strip + full recents list */}
              <Show when={rail() === "recent"}>
                <h2 class="home-section-title">{t("home.quickNew")}</h2>
                <div class="app-quick-row">
                  <For each={blanks}>
                    {(card) => <QuickTile label={card.label} accent={card.accent} icon={card.icon} onClick={card.run} />}
                  </For>
                </div>
                <div class="home-recent-header">
                  <h2 class="home-section-title">{t("home.recentSection")}</h2>
                  <select class="home-recent-sort" aria-label={t("home.sort.label")} value={sortBy()} onChange={(e) => setSortBy(e.currentTarget.value)}>
                    <option value="Last opened">{t("home.sort.lastOpened")}</option>
                    <option value="Title">{t("home.sort.title")}</option>
                    <option value="Mode">{t("home.sort.mode")}</option>
                  </select>
                </div>
                <div role="tablist" aria-label="File filters" class="home-filter-tabs">
                  <For each={["All", "Documents", "Spreadsheets", "Presentations", "Pinned"]}>
                    {(cat) => (
                      <button type="button" role="tab" class="home-filter-tab" aria-selected={filterCategory() === cat} onClick={() => setFilterCategory(cat)}>
                        {cat}
                      </button>
                    )}
                  </For>
                </div>
                <Show
                  when={filteredRecents().length > 0}
                  fallback={<div class="home-empty">{searchQuery() ? t("home.emptySearch", { query: searchQuery() }) : t("home.empty")}</div>}
                >
                  <div class="home-file-card" role="listbox" aria-label={t("home.recentSection")}>
                    <div class="home-file-row-head" aria-hidden="true">
                      <span>{t("home.table.name")}</span>
                      <span>{t("home.table.owner")}</span>
                      <span>{t("home.table.lastOpened")}</span>
                      <span />
                    </div>
                    <For each={filteredRecents()}>
                      {(entry) => {
                        // Reactive getter — fileExists() fills in asynchronously
                        // after pathsExist resolves, so reads must stay live.
                        const missing = () => entry.path && fileExists()[entry.id] === false;
                        return (
                          <div
                            role="option"
                            aria-selected={selected()?.id === entry.id}
                            tabindex="0"
                            class="home-file-row"
                            data-missing={missing() ? "true" : undefined}
                            title={missing() ? t("home.missingFile") : entry.path}
                            onClick={() => {
                              if (missing()) return;
                              setSelectedId(entry.id);
                              props.onOpenRecent(entry);
                            }}
                            onKeyDown={(e) => {
                              if (missing()) return;
                              if (e.key === "Enter" || e.key === " ") {
                                e.preventDefault();
                                setSelectedId(entry.id);
                                props.onOpenRecent(entry);
                              }
                              if (e.key === "ArrowDown" || e.key === "ArrowUp") {
                                e.preventDefault();
                                const list = filteredRecents();
                                const i = list.findIndex((r) => r.id === entry.id);
                                const n = e.key === "ArrowDown" ? list[i + 1] : list[i - 1];
                                if (n) {
                                  setSelectedId(n.id);
                                  (e.currentTarget.parentElement?.querySelector(`[data-entry="${n.id}"]`) as HTMLElement)?.focus();
                                }
                              }
                            }}
                            data-entry={entry.id}
                            onFocus={() => setSelectedId(entry.id)}
                            onContextMenu={(e) => {
                              e.preventDefault();
                              setSelectedId(entry.id);
                              setRowMenu({ x: e.clientX, y: e.clientY, entry });
                            }}
                          >
                            <div class="home-file-name">
                              <span class="app-file-chip" style={{ "--chip-accent": modeAccentVar(entry.mode) }} aria-hidden="true">
                                {modeIcon(entry.mode)}
                              </span>
                              <div style={{ overflow: "hidden", "text-overflow": "ellipsis", "white-space": "nowrap", "min-width": "0", flex: "1" }}>
                                <div class="home-file-title">{entry.title}</div>
                                <div class="home-file-path">{entry.path}</div>
                              </div>
                              <Show when={missing()}>
                                <span class="app-missing-chip">{t("home.missingBadge")}</span>
                              </Show>
                            </div>
                            <span class="home-file-meta">{t("home.ownerMe")}</span>
                            <span class="home-file-meta">{formatDate(entry.lastOpenedAt)}</span>
                            <button
                              type="button"
                              title={entry.pinned ? t("home.unpin") : t("home.pin")}
                              aria-label={`${entry.pinned ? t("home.unpin") : t("home.pin")}: ${entry.title}`}
                              aria-pressed={entry.pinned}
                              onClick={(e) => {
                                e.stopPropagation();
                                props.onTogglePin(entry.id);
                              }}
                              class="g-icon-btn"
                            >
                              <IconPin color={entry.pinned ? "var(--accent-color)" : "var(--text-muted)"} />
                            </button>
                          </div>
                        );
                      }}
                    </For>
                  </div>
                </Show>
              </Show>

              <Show when={rail() === "shared"}>
                <h2 class="home-section-title">{t("home.sharedSection")}</h2>
                <div class="home-empty">{t("home.sharedEmpty")}</div>
              </Show>

              <Show when={props.onDropFile}>
                <div class="app-drop-hint">{t("home.dropHint")}</div>
              </Show>
            </main>

            <aside class="home-right app-aside" aria-label="Preview and activity">
              <section aria-label={t("home.previewSection")}>
                <h2 class="home-section-title" style={{ "margin-bottom": "8px" }}>{t("home.previewSection")}</h2>
                <Show
                  when={selected()}
                  fallback={<div class="home-empty">{t("home.previewEmpty")}</div>}
                >
                  {(entry) => (
                    <div class="app-preview-card">
                      <div class="app-preview-head">
                        <span class="app-file-chip" style={{ "--chip-accent": modeAccentVar(entry().mode) }} aria-hidden="true">
                          {modeIcon(entry().mode)}
                        </span>
                        <div style={{ "min-width": "0" }}>
                          <div class="app-preview-title">{entry().title}</div>
                          <div class="app-preview-path">{entry().path}</div>
                        </div>
                      </div>
                      <button type="button" class="home-open-btn" onClick={() => props.onOpenRecent(entry())}>
                        {t("home.previewOpen")}
                      </button>
                    </div>
                  )}
                </Show>
              </section>
              <section aria-label={t("home.activitySection")}>
                <h2 class="home-section-title" style={{ "margin-bottom": "8px" }}>{t("home.activitySection")}</h2>
                <Show when={props.recents.length > 0} fallback={<div class="home-empty">{t("home.activityEmpty")}</div>}>
                  <div style={{ display: "flex", "flex-direction": "column", gap: "8px" }}>
                    <For each={props.recents.slice(0, 5)}>
                      {(r) => (
                        <div class="app-activity-row">
                          <span class="app-activity-dot" style={{ background: modeAccentVar(r.mode) }} aria-hidden="true" />
                          <span class="app-activity-text">{r.title} · {formatDate(r.lastOpenedAt)}</span>
                        </div>
                      )}
                    </For>
                  </div>
                </Show>
              </section>
            </aside>
          </div>
        </div>
      </div>
      <Show when={rowMenu()}>
        {(menu) => (
          <ContextMenu x={menu().x} y={menu().y} items={rowMenuItems(menu().entry)} onClose={() => setRowMenu(null)} />
        )}
      </Show>
    </div>
  );
}
