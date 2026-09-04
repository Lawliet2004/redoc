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
  const [telemetryEnabled, setTelemetryEnabled] = createSignal(props.settings.telemetryEnabled ?? false);
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
      setTelemetryEnabled(props.settings.telemetryEnabled ?? false);
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
      telemetryEnabled: telemetryEnabled(),
      checkForUpdates: checkForUpdates(),
      author: {
        displayName: authorName().trim() || "You",
        email: authorEmail().trim() ? authorEmail().trim() : null,
        color: authorColor(),
      },
    });
    props.onClose();
  };

  const inputStyle = {
    width: "100%",
    "margin-top": "4px",
    padding: "8px",
    "border-radius": "var(--radius-md)",
    border: "1px solid var(--border-color)",
    background: "var(--bg-surface)",
    color: "var(--text-primary)",
  };

  const labelStyle = { "font-size": "13px", "font-weight": "500", color: "var(--text-secondary)" };
  const checkboxLabelStyle = { display: "flex", gap: "8px", "align-items": "center", "font-size": "13px", color: "var(--text-primary)" };

  return (
    <Dialog open={props.open} title="Redoc Settings" onClose={props.onClose}>
      <div style={{ display: "flex", "flex-direction": "column", gap: "16px" }}>
        <div>
          <label style={labelStyle}>Theme</label>
          <select
            value={theme()}
            onChange={(e) => setTheme(e.currentTarget.value as any)}
            style={inputStyle}
          >
            <option value="system">System Default</option>
            <option value="light">Light Mode</option>
            <option value="dark">Dark Mode</option>
          </select>
        </div>

        <div style={{ display: "flex", gap: "16px" }}>
          <div style={{ flex: 1 }}>
            <label style={labelStyle}>Default Font Size (8-72)</label>
            <input
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
              style={inputStyle}
            />
          </div>
          <div style={{ flex: 1 }}>
            <label style={labelStyle}>Default Zoom Level (50-200)</label>
            <input
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
              style={inputStyle}
            />
          </div>
        </div>

        <div>
          <label style={labelStyle}>Autosave Interval (ms)</label>
          <input
            type="number"
            min="500"
            value={autosaveInterval()}
            onInput={(e) => setAutosaveInterval(Number(e.currentTarget.value))}
            style={inputStyle}
          />
        </div>

        <div>
          <label style={labelStyle}>Display Name (used for comments and tracked changes)</label>
          <input
            type="text"
            maxlength="80"
            value={authorName()}
            onInput={(e) => setAuthorName(e.currentTarget.value)}
            style={inputStyle}
          />
        </div>

        <div style={{ display: "flex", gap: "16px" }}>
          <div style={{ flex: 1 }}>
            <label style={labelStyle}>Email (optional)</label>
            <input
              type="email"
              maxlength="254"
              value={authorEmail()}
              onInput={(e) => setAuthorEmail(e.currentTarget.value)}
              style={inputStyle}
            />
          </div>
          <div>
            <label style={labelStyle}>Author Color</label>
            <input
              type="color"
              value={authorColor()}
              onInput={(e) => setAuthorColor(e.currentTarget.value)}
              aria-label="Author color"
              style={{ ...inputStyle, width: "64px", height: "36px", padding: "2px", "margin-top": "4px", cursor: "pointer" }}
            />
          </div>
        </div>

        <div style={{ display: "flex", "flex-direction": "column", gap: "8px", "margin-top": "4px" }}>
          <label style={checkboxLabelStyle}>
            <input
              type="checkbox"
              checked={spellcheckEnabled()}
              onChange={(e) => setSpellcheckEnabled(e.currentTarget.checked)}
            />
            Enable Spellcheck
          </label>
          
          <label style={checkboxLabelStyle}>
            <input
              type="checkbox"
              checked={telemetryEnabled()}
              onChange={(e) => setTelemetryEnabled(e.currentTarget.checked)}
            />
            Enable Anonymous Telemetry
          </label>
          
          <label style={checkboxLabelStyle}>
            <input
              type="checkbox"
              checked={checkForUpdates()}
              onChange={(e) => setCheckForUpdates(e.currentTarget.checked)}
            />
            Check for updates
          </label>
        </div>

        <div style={{ "margin-top": "4px" }}>
          <Button variant="secondary" onClick={() => void commands.openLogsFolder()}>
            Open logs folder
          </Button>
        </div>

        <div style={{ display: "flex", "justify-content": "flex-end", gap: "8px", "margin-top": "8px" }}>
          <Button variant="secondary" onClick={props.onClose}>Cancel</Button>
          <Button variant="primary" onClick={handleSave}>Save Settings</Button>
        </div>
      </div>
    </Dialog>
  );
}
