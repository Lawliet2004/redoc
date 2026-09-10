import { describe, expect, it } from "vitest";

// Theme color values synced with packages/ui/src/theme.css
// WCAG AAA requires 7:1 for normal text, 4.5:1 for large text (18pt+ bold or 24pt+)
export const CONTRAST_PAIRS = {
  dark: {
    // Backgrounds
    bgPrimary: "#2b2b2b",
    bgTertiary: "#4a4a4a",
    accentText: "#0b1420",
    toastText: "#0b1420",
    // Text colors
    textPrimary: "#f5f5f5",
    textSecondary: "#d4d4d4",
    textMuted: "#c0c0c0",
    textLink: "#8ecbff",
    // Accent colors
    accent: "#66b3ff",
    error: "#f0705f",
    warning: "#f5b04d",
    success: "#4cc38a",
  },
  light: {
    // Backgrounds
    bgPrimary: "#f0f0f0",
    bgTertiary: "#d0d0d0",
    accentText: "#ffffff",
    // Text colors
    textPrimary: "#1a1a1a",
    textSecondary: "#3a3a3a",
    textMuted: "#505050",
    textLink: "#054a98",
    // Accent colors
    accent: "#054a98",
  },
} as const;

function channel(v: number): number {
  const s = v / 255;
  return s <= 0.03928 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
}

function luminance(hex: string): number {
  const h = hex.replace("#", "");
  const r = channel(parseInt(h.slice(0, 2), 16));
  const g = channel(parseInt(h.slice(2, 4), 16));
  const b = channel(parseInt(h.slice(4, 6), 16));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

export function contrastRatio(a: string, b: string): number {
  const la = luminance(a);
  const lb = luminance(b);
  const hi = Math.max(la, lb);
  const lo = Math.min(la, lb);
  return (hi + 0.05) / (lo + 0.05);
}

describe("theme contrast tokens - WCAG AAA compliance", () => {
  // Dark theme tests
  it("dark: text-primary passes 7:1 on bg-primary (AAA)", () => {
    const { dark } = CONTRAST_PAIRS;
    expect(contrastRatio(dark.textPrimary, dark.bgPrimary)).toBeGreaterThanOrEqual(7);
  });

  it("dark: text-secondary passes 7:1 on bg-primary (AAA)", () => {
    const { dark } = CONTRAST_PAIRS;
    expect(contrastRatio(dark.textSecondary, dark.bgPrimary)).toBeGreaterThanOrEqual(7);
  });

  it("dark: text-muted passes 4.5:1 on bg-tertiary (AA)", () => {
    const { dark } = CONTRAST_PAIRS;
    expect(contrastRatio(dark.textMuted, dark.bgTertiary)).toBeGreaterThanOrEqual(4.5);
  });

  it("dark: text-link passes 7:1 on bg-primary (AAA)", () => {
    const { dark } = CONTRAST_PAIRS;
    expect(contrastRatio(dark.textLink, dark.bgPrimary)).toBeGreaterThanOrEqual(7);
  });

  it("dark: accent fills pass 4.5:1 with their text", () => {
    const { dark } = CONTRAST_PAIRS;
    expect(contrastRatio(dark.accent, dark.accentText)).toBeGreaterThanOrEqual(4.5);
  });

  it("dark: toast color fills pass 4.5:1 with toast text", () => {
    const { dark } = CONTRAST_PAIRS;
    expect(contrastRatio(dark.error, dark.toastText)).toBeGreaterThanOrEqual(4.5);
    expect(contrastRatio(dark.warning, dark.toastText)).toBeGreaterThanOrEqual(4.5);
    expect(contrastRatio(dark.success, dark.toastText)).toBeGreaterThanOrEqual(4.5);
  });

  // Light theme tests
  it("light: text-primary passes 7:1 on bg-primary (AAA)", () => {
    const { light } = CONTRAST_PAIRS;
    expect(contrastRatio(light.textPrimary, light.bgPrimary)).toBeGreaterThanOrEqual(7);
  });

  it("light: text-secondary passes 7:1 on bg-primary (AAA)", () => {
    const { light } = CONTRAST_PAIRS;
    expect(contrastRatio(light.textSecondary, light.bgPrimary)).toBeGreaterThanOrEqual(7);
  });

  it("light: text-muted passes 4.5:1 on bg-tertiary (AA)", () => {
    const { light } = CONTRAST_PAIRS;
    expect(contrastRatio(light.textMuted, light.bgTertiary)).toBeGreaterThanOrEqual(4.5);
  });

  it("light: text-link passes 7:1 on bg-primary (AAA)", () => {
    const { light } = CONTRAST_PAIRS;
    expect(contrastRatio(light.textLink, light.bgPrimary)).toBeGreaterThanOrEqual(7);
  });

  it("light: accent fills pass 4.5:1 with their text", () => {
    const { light } = CONTRAST_PAIRS;
    expect(contrastRatio(light.accent, light.accentText)).toBeGreaterThanOrEqual(4.5);
  });
});
