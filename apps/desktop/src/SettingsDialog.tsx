import { createSignal, createEffect } from "solid-js";
import { Dialog, Button } from "@redoc/ui";
import { AppSettings, commands } from "@redoc/api-client";

interface SettingsDialogProps {
  open: boolean;
  settings: AppSettings;
  onClose: () => void;
  onSave: (newSettings: AppSettings) => void;
}

export function SettingsDialog(props: SettingsDialogProps) {
  const [theme, setTheme] = createSignal<"light" | "dark" | "system">(props.settings.theme || "system");
  const [fontSizeDefault, setFontSizeDefault] = createSignal(props.settings.fontSizeDefault ?? 12);
  const [zoomLevel, setZoomLevel] = createSignal(props.settings.zoomLevel ?? 100);
  const [autosaveInterval, setAutosaveInterval] = createSignal(props.settings.autosaveIntervalMs || 2000);
  const [spellcheckEnabled, setSpellcheckEnabled] = createSignal(props.settings.spellcheckEnabled ?? true);
  const [checkForUpdates, setCheckForUpdates] = createSignal(props.settings.checkForUpdates !== false);
  const [authorName, setAuthorName] = createSignal(props.settings.author?.displayName || "You");
  const [authorEmail, setAuthorEmail] = createSignal(props.settings.author?.email || "");
  const [authorColor, setAuthorColor] = createSignal(props.settings.author?.color || "#5b9bd5");

  createEffect(() => {
    if (props.open) {
      setTheme(props.settings.theme || "system");
      setFontSizeDefault(props.settings.fontSizeDefault ?? 12);
      setZoomLevel(props.settings.zoomLevel ?? 100);
      setAutosaveInterval(props.settings.autosaveIntervalMs || 2000);
      setSpellcheckEnabled(props.settings.spellcheckEnabled ?? true);
      setCheckForUpdates(props.settings.checkForUpdates !== false);
      setAuthorName(props.settings.author?.displayName || "You");
      setAuthorEmail(props.settings.author?.email || "");
      setAuthorColor(props.settings.author?.color || "#5b9bd5");
    }
  });

  const handleSave = () => {
    props.onSave({
      ...props.settings,
      theme: theme(),
      fontSizeDefault: fontSizeDefault(),
      zoomLevel: zoomLevel(),
      autosaveIntervalMs: autosaveInterval(),
      spellcheckEnabled: spellcheckEnabled(),
      checkForUpdates: checkForUpdates(),
      author: {
        displayName: authorName().trim() || "You",
        email: authorEmail().trim() ? authorEmail().trim() : null,
        color: authorColor(),
      },
    });
    props.onClose();
  };

  return (
    <Dialog open={props.open} title="Redoc Settings" onClose={props.onClose}>
      <div style={{ display: "flex", "flex-direction": "column", gap: "18px" }}>
        <div>
          <label class="app-field-label" for="settings-theme">Theme</label>
          <select
            id="settings-theme"
            class="app-field-input"
            value={theme()}
            onChange={(e) => setTheme(e.currentTarget.value as any)}
          >
            <option value="system">System Default</option>
            <option value="light">Light Mode</option>
            <option value="dark">Dark Mode</option>
          </select>
        </div>

        <div style={{ display: "flex", gap: "16px" }}>
          <div style={{ flex: 1 }}>
            <label class="app-field-label" for="settings-font-size">Default Font Size (8-72)</label>
            <input
              id="settings-font-size"
              class="app-field-input"
              type="number"
              min="8"
              max="72"
              value={fontSizeDefault()}
              onInput={(e) => {
                let val = Number(e.currentTarget.value);
                if (val < 8) val = 8;
                if (val > 72) val = 72;
                setFontSizeDefault(val);
              }}
            />
          </div>
          <div style={{ flex: 1 }}>
            <label class="app-field-label" for="settings-zoom">Default Zoom Level (50-200)</label>
            <input
              id="settings-zoom"
              class="app-field-input"
              type="number"
              min="50"
              max="200"
              value={zoomLevel()}
              onInput={(e) => {
                let val = Number(e.currentTarget.value);
                if (val < 50) val = 50;
                if (val > 200) val = 200;
                setZoomLevel(val);
              }}
            />
          </div>
        </div>

        <div>
          <label class="app-field-label" for="settings-autosave">Autosave Interval (ms)</label>
          <input
            id="settings-autosave"
            class="app-field-input"
            type="number"
            min="500"
            value={autosaveInterval()}
            onInput={(e) => setAutosaveInterval(Number(e.currentTarget.value))}
          />
        </div>

        <div>
          <label class="app-field-label" for="settings-author-name">Display Name (used for comments and tracked changes)</label>
          <input
            id="settings-author-name"
            class="app-field-input"
            type="text"
            maxlength="80"
            value={authorName()}
            onInput={(e) => setAuthorName(e.currentTarget.value)}
          />
        </div>

        <div style={{ display: "flex", gap: "16px" }}>
          <div style={{ flex: 1 }}>
            <label class="app-field-label" for="settings-author-email">Email (optional)</label>
            <input
              id="settings-author-email"
              class="app-field-input"
              type="email"
              maxlength="254"
              value={authorEmail()}
              onInput={(e) => setAuthorEmail(e.currentTarget.value)}
            />
          </div>
          <div>
            <label class="app-field-label" for="settings-author-color">Author Color</label>
            <input
              id="settings-author-color"
              class="app-field-input"
              type="color"
              value={authorColor()}
              onInput={(e) => setAuthorColor(e.currentTarget.value)}
              aria-label="Author color"
              style={{ width: "64px", height: "38px", padding: "2px", cursor: "pointer" }}
            />
          </div>
        </div>

        <div style={{ display: "flex", "flex-direction": "column", gap: "6px" }}>
          <label class="app-field-check">
            <input
              type="checkbox"
              checked={spellcheckEnabled()}
              onChange={(e) => setSpellcheckEnabled(e.currentTarget.checked)}
            />
            Enable Spellcheck
          </label>

          <label class="app-field-check">
            <input
              type="checkbox"
              checked={checkForUpdates()}
              onChange={(e) => setCheckForUpdates(e.currentTarget.checked)}
            />
            Check for updates
          </label>
        </div>

        <div>
          <Button variant="secondary" onClick={() => void commands.openLogsFolder()}>
            Open logs folder
          </Button>
        </div>

        <div class="app-dialog-actions">
          <Button variant="secondary" onClick={props.onClose}>Cancel</Button>
          <Button variant="primary" onClick={handleSave}>Save Settings</Button>
        </div>
      </div>
    </Dialog>
  );
}
