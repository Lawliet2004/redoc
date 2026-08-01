import { createSignal, For, Show } from "solid-js";
import {
  IconDoc,
  IconSheet,
  IconSlide,
  IconFolderOpen,
  IconPin,
  IconSearch,
} from "@redoc/icons";
import { RecentEntry } from "@redoc/api-client";
import { formatDate } from "@redoc/utils";

interface HomeScreenProps {
  recents: RecentEntry[];
  onNewDoc: (mode: "doc" | "sheet" | "slide", templateId?: string) => void;
  onOpenFile: () => void;
  onOpenRecent: (entry: RecentEntry) => void;
  onTogglePin: (id: string) => void;
  onDropFile?: (file: File) => void;
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
    <button
      type="button"
      onClick={props.onClick}
      style={{
        display: "flex",
        "flex-direction": "column",
        "align-items": "stretch",
        width: "148px",
        background: "transparent",
        border: "none",
        cursor: "pointer",
        "text-align": "left",
        gap: "10px",
        padding: "0",
      }}
    >
      <div
        style={{
          height: "186px",
          background: "#fff",
          border: "1px solid var(--border-color)",
          "border-radius": "4px",
          display: "flex",
          "align-items": "center",
          "justify-content": "center",
          transition: "border-color 0.15s ease, box-shadow 0.15s ease",
          position: "relative",
          overflow: "hidden",
        }}
        onMouseEnter={(e) => {
          e.currentTarget.style.borderColor = props.accent;
          e.currentTarget.style.boxShadow = "var(--shadow-sm)";
        }}
        onMouseLeave={(e) => {
          e.currentTarget.style.borderColor = "var(--border-color)";
          e.currentTarget.style.boxShadow = "none";
        }}
      >
        <div
          style={{
            width: "56px",
            height: "56px",
            "border-radius": "8px",
            background: props.iconBg,
            display: "flex",
            "align-items": "center",
            "justify-content": "center",
            color: props.accent,
          }}
        >
          <Icon width="32" height="32" />
        </div>
      </div>
      <div>
        <div style={{ "font-size": "13px", color: "var(--text-primary)", "font-weight": "500" }}>
          {props.label}
        </div>
        <div style={{ "font-size": "12px", color: "var(--text-muted)", "margin-top": "2px" }}>
          {props.sublabel}
        </div>
      </div>
    </button>
  );
}

export function HomeScreen(props: HomeScreenProps) {
  const [searchQuery, setSearchQuery] = createSignal("");
  const [filterCategory, setFilterCategory] = createSignal("All");
  const [sortBy, setSortBy] = createSignal("Last opened");

  const filteredRecents = () => {
    let list = props.recents;
    const cat = filterCategory();
    if (cat === "Documents") list = list.filter((r) => r.mode === "doc");
    else if (cat === "Spreadsheets") list = list.filter((r) => r.mode === "sheet");
    else if (cat === "Presentations") list = list.filter((r) => r.mode === "slide");
    else if (cat === "Pinned") list = list.filter((r) => r.pinned);

    const q = searchQuery().toLowerCase().trim();
    if (q) {
      list = list.filter(
        (r) => r.title.toLowerCase().includes(q) || r.path.toLowerCase().includes(q)
      );
    }

    const sorted = [...list];
    const sort = sortBy();
    if (sort === "Title") {
      sorted.sort((a, b) => a.title.localeCompare(b.title));
    } else if (sort === "Mode") {
      sorted.sort((a, b) => a.mode.localeCompare(b.mode));
    } else {
      sorted.sort((a, b) => new Date(b.lastOpenedAt).getTime() - new Date(a.lastOpenedAt).getTime());
    }

    return sorted;
  };

  return (
    <div
      onDragOver={(e) => e.preventDefault()}
      onDrop={(e) => {
        e.preventDefault();
        if (e.dataTransfer?.files?.length && props.onDropFile) {
          props.onDropFile(e.dataTransfer.files[0]);
        }
      }}
      style={{
        flex: 1,
        display: "flex",
        "flex-direction": "column",
        background: "var(--bg-home)",
        overflow: "auto",
        height: "100%",
      }}
    >
      {/* Google Docs–style top search bar */}
      <div
        style={{
          display: "flex",
          "align-items": "center",
          "justify-content": "center",
          padding: "12px 24px",
          background: "var(--bg-surface)",
          "border-bottom": "1px solid var(--border-color)",
          gap: "16px",
        }}
      >
        <div style={{ display: "flex", "align-items": "center", gap: "10px", "min-width": "160px" }}>
          <div
            style={{
              width: "40px",
              height: "40px",
              "border-radius": "8px",
              background: "var(--g-blue-light)",
              display: "flex",
              "align-items": "center",
              "justify-content": "center",
              color: "var(--doc-accent)",
            }}
          >
            <IconDoc width="24" height="24" />
          </div>
          <span style={{ "font-size": "22px", color: "var(--text-secondary)", "font-weight": "400" }}>
            Redoc
          </span>
        </div>
        <div
          style={{
            flex: 1,
            "max-width": "720px",
            display: "flex",
            "align-items": "center",
            gap: "12px",
            background: "var(--bg-tertiary)",
            "border-radius": "8px",
            padding: "10px 16px",
            height: "48px",
          }}
        >
          <IconSearch color="var(--text-secondary)" />
          <input
            type="search"
            placeholder="Search"
            aria-label="Search documents"
            value={searchQuery()}
            onInput={(e) => setSearchQuery(e.currentTarget.value)}
            style={{
              flex: 1,
              border: "none",
              background: "transparent",
              "font-size": "16px",
              color: "var(--text-primary)",
            }}
          />
        </div>
        <button
          type="button"
          onClick={props.onOpenFile}
          style={{
            display: "inline-flex",
            "align-items": "center",
            gap: "8px",
            padding: "8px 16px",
            "border-radius": "24px",
            border: "1px solid var(--border-color)",
            background: "var(--bg-surface)",
            color: "var(--text-primary)",
            "font-size": "14px",
            "font-weight": "500",
          }}
        >
          <IconFolderOpen /> Open
        </button>
      </div>

      {/* Start a new document */}
      <section style={{ background: "var(--bg-secondary)", padding: "28px 0 32px" }}>
        <div style={{ "max-width": "1080px", margin: "0 auto", padding: "0 24px" }}>
          <div
            style={{
              display: "flex",
              "align-items": "center",
              "justify-content": "space-between",
              "margin-bottom": "16px",
            }}
          >
            <h2 style={{ "font-size": "16px", "font-weight": "500", color: "var(--text-primary)" }}>
              Start a new document
            </h2>
          </div>
          <div style={{ display: "flex", gap: "20px", "flex-wrap": "wrap" }}>
            <TemplateTile
              label="Blank document"
              sublabel="Docs"
              accent="var(--doc-accent)"
              iconBg="var(--g-blue-light)"
              icon={IconDoc}
              onClick={() => props.onNewDoc("doc")}
            />
            <TemplateTile
              label="Resume / CV"
              sublabel="Docs"
              accent="var(--doc-accent)"
              iconBg="var(--g-blue-light)"
              icon={IconDoc}
              onClick={() => props.onNewDoc("doc", "resume")}
            />
            <TemplateTile
              label="Project Report"
              sublabel="Docs"
              accent="var(--doc-accent)"
              iconBg="var(--g-blue-light)"
              icon={IconDoc}
              onClick={() => props.onNewDoc("doc", "report")}
            />
            <TemplateTile
              label="Blank spreadsheet"
              sublabel="Sheets"
              accent="var(--sheet-accent)"
              iconBg="var(--g-green-light)"
              icon={IconSheet}
              onClick={() => props.onNewDoc("sheet")}
            />
            <TemplateTile
              label="Monthly Budget"
              sublabel="Sheets"
              accent="var(--sheet-accent)"
              iconBg="var(--g-green-light)"
              icon={IconSheet}
              onClick={() => props.onNewDoc("sheet", "budget")}
            />
            <TemplateTile
              label="Project Plan"
              sublabel="Sheets"
              accent="var(--sheet-accent)"
              iconBg="var(--g-green-light)"
              icon={IconSheet}
              onClick={() => props.onNewDoc("sheet", "project_plan")}
            />
            <TemplateTile
              label="Blank presentation"
              sublabel="Slides"
              accent="var(--slide-accent)"
              iconBg="var(--g-yellow-light)"
              icon={IconSlide}
              onClick={() => props.onNewDoc("slide")}
            />
            <TemplateTile
              label="Pitch Deck"
              sublabel="Slides"
              accent="var(--slide-accent)"
              iconBg="var(--g-yellow-light)"
              icon={IconSlide}
              onClick={() => props.onNewDoc("slide", "pitch_deck")}
            />
          </div>
        </div>
      </section>

      {/* Recent documents */}
      <section style={{ "max-width": "1080px", width: "100%", margin: "0 auto", padding: "28px 24px 48px" }}>
        <div style={{ display: "flex", "align-items": "center", "justify-content": "space-between", "margin-bottom": "16px" }}>
          <h2
            style={{
              "font-size": "16px",
              "font-weight": "500",
              color: "var(--text-primary)",
              margin: 0,
            }}
          >
            Recent documents
          </h2>
          
          <div style={{ display: "flex", gap: "12px", "align-items": "center" }}>
            <select 
              aria-label="Sort recent documents"
              value={sortBy()} 
              onChange={(e) => setSortBy(e.currentTarget.value)}
              style={{
                padding: "6px 12px",
                "border-radius": "4px",
                border: "1px solid var(--border-color)",
                background: "var(--bg-surface)",
                color: "var(--text-primary)",
                "font-size": "14px",
              }}
            >
              <option value="Last opened">Last opened</option>
              <option value="Title">Title</option>
              <option value="Mode">Mode</option>
            </select>
          </div>
        </div>

        <div style={{ display: "flex", gap: "16px", "margin-bottom": "16px", "border-bottom": "1px solid var(--border-color)", "padding-bottom": "8px" }}>
          <For each={["All", "Documents", "Spreadsheets", "Presentations", "Pinned"]}>
            {(cat) => (
              <button
                type="button"
                onClick={() => setFilterCategory(cat)}
                style={{
                  background: "transparent",
                  border: "none",
                  "border-bottom": filterCategory() === cat ? "2px solid var(--accent-color)" : "2px solid transparent",
                  color: filterCategory() === cat ? "var(--accent-color)" : "var(--text-muted)",
                  "font-size": "14px",
                  "font-weight": filterCategory() === cat ? "500" : "400",
                  cursor: "pointer",
                  padding: "0 4px 8px",
                  "margin-bottom": "-10px",
                }}
              >
                {cat}
              </button>
            )}
          </For>
        </div>

        <Show
          when={filteredRecents().length > 0}
          fallback={
            <div
              style={{
                padding: "48px 24px",
                "text-align": "center",
                color: "var(--text-muted)",
                "font-size": "14px",
                border: "1px dashed var(--border-color)",
                "border-radius": "8px",
                background: "var(--bg-surface)",
              }}
            >
              No recent files. Create a blank document to get started.
            </div>
          }
        >
          <div
            style={{
              background: "var(--bg-surface)",
              "border-radius": "8px",
              overflow: "hidden",
              border: "1px solid var(--border-color)",
            }}
          >
            <div
              style={{
                display: "grid",
                "grid-template-columns": "1fr 180px 160px 40px",
                padding: "10px 16px",
                "font-size": "12px",
                color: "var(--text-muted)",
                "border-bottom": "1px solid var(--border-color)",
                "font-weight": "500",
              }}
            >
              <span>Name</span>
              <span>Owner</span>
              <span>Last opened</span>
              <span />
            </div>
            <For each={filteredRecents()}>
              {(entry) => (
                <div
                  role="button"
                  tabindex="0"
                  onClick={() => props.onOpenRecent(entry)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") props.onOpenRecent(entry);
                  }}
                  style={{
                    display: "grid",
                    "grid-template-columns": "1fr 180px 160px 40px",
                    "align-items": "center",
                    padding: "10px 16px",
                    "border-bottom": "1px solid var(--border-color)",
                    cursor: "pointer",
                    "font-size": "14px",
                  }}
                  onMouseEnter={(e) => {
                    e.currentTarget.style.background = "var(--bg-tertiary)";
                  }}
                  onMouseLeave={(e) => {
                    e.currentTarget.style.background = "transparent";
                  }}
                >
                  <div style={{ display: "flex", "align-items": "center", gap: "12px", "min-width": "0" }}>
                    <Show when={entry.mode === "doc"}>
                      <IconDoc color="var(--doc-accent)" />
                    </Show>
                    <Show when={entry.mode === "sheet"}>
                      <IconSheet color="var(--sheet-accent)" />
                    </Show>
                    <Show when={entry.mode === "slide"}>
                      <IconSlide color="var(--slide-accent)" />
                    </Show>
                    <div style={{ overflow: "hidden", "text-overflow": "ellipsis", "white-space": "nowrap" }}>
                      <div style={{ "font-weight": "500", color: "var(--text-primary)" }}>{entry.title}</div>
                      <div
                        style={{
                          "font-size": "12px",
                          color: "var(--text-muted)",
                          overflow: "hidden",
                          "text-overflow": "ellipsis",
                        }}
                      >
                        {entry.path}
                      </div>
                    </div>
                  </div>
                  <span style={{ color: "var(--text-secondary)", "font-size": "13px" }}>me</span>
                  <span style={{ color: "var(--text-secondary)", "font-size": "13px" }}>
                    {formatDate(entry.lastOpenedAt)}
                  </span>
                  <button
                    type="button"
                    title={entry.pinned ? "Unpin" : "Pin"}
                    onClick={(e) => {
                      e.stopPropagation();
                      props.onTogglePin(entry.id);
                    }}
                    class="g-icon-btn"
                  >
                    <IconPin color={entry.pinned ? "var(--accent-color)" : "var(--text-muted)"} />
                  </button>
                </div>
              )}
            </For>
          </div>
        </Show>
      </section>
    </div>
  );
}
