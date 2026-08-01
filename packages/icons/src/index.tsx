import { ComponentProps, JSX } from "solid-js";

type IconProps = ComponentProps<"svg">;

function base(props: IconProps, children: JSX.Element, size = 16) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
      {...props}
    >
      {children}
    </svg>
  );
}

export function IconDoc(props: IconProps) {
  return base(props, <>
    <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
    <polyline points="14 2 14 8 20 8" />
    <line x1="16" y1="13" x2="8" y2="13" />
    <line x1="16" y1="17" x2="8" y2="17" />
  </>, 20);
}

export function IconSheet(props: IconProps) {
  return base(props, <>
    <rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
    <line x1="3" y1="9" x2="21" y2="9" />
    <line x1="3" y1="15" x2="21" y2="15" />
    <line x1="9" y1="3" x2="9" y2="21" />
    <line x1="15" y1="3" x2="15" y2="21" />
  </>, 20);
}

export function IconSlide(props: IconProps) {
  return base(props, <>
    <rect x="2" y="3" width="20" height="14" rx="2" ry="2" />
    <line x1="8" y1="21" x2="16" y2="21" />
    <line x1="12" y1="17" x2="12" y2="21" />
  </>, 20);
}

export function IconBold(props: IconProps) {
  return base(props, <>
    <path d="M6 4h8a4 4 0 0 1 4 4 4 4 0 0 1-4 4H6z" />
    <path d="M6 12h9a4 4 0 0 1 4 4 4 4 0 0 1-4 4H6z" />
  </>);
}

export function IconItalic(props: IconProps) {
  return base(props, <>
    <line x1="19" y1="4" x2="10" y2="4" />
    <line x1="14" y1="20" x2="5" y2="20" />
    <line x1="15" y1="4" x2="9" y2="20" />
  </>);
}

export function IconUnderline(props: IconProps) {
  return base(props, <>
    <path d="M6 3v7a6 6 0 0 0 6 6 6 6 0 0 0 6-6V3" />
    <line x1="4" y1="21" x2="20" y2="21" />
  </>);
}

export function IconStrikethrough(props: IconProps) {
  return base(props, <>
    <path d="M16 4H9a3 3 0 0 0-3 3v0a3 3 0 0 0 3 3h6" />
    <line x1="4" y1="12" x2="20" y2="12" />
    <path d="M15 12h1a3 3 0 0 1 3 3v0a3 3 0 0 1-3 3H8" />
  </>);
}

export function IconAlignLeft(props: IconProps) {
  return base(props, <>
    <line x1="17" y1="10" x2="3" y2="10" />
    <line x1="21" y1="6" x2="3" y2="6" />
    <line x1="21" y1="14" x2="3" y2="14" />
    <line x1="15" y1="18" x2="3" y2="18" />
  </>);
}

export function IconAlignCenter(props: IconProps) {
  return base(props, <>
    <line x1="18" y1="10" x2="6" y2="10" />
    <line x1="21" y1="6" x2="3" y2="6" />
    <line x1="21" y1="14" x2="3" y2="14" />
    <line x1="16" y1="18" x2="8" y2="18" />
  </>);
}

export function IconAlignRight(props: IconProps) {
  return base(props, <>
    <line x1="21" y1="10" x2="7" y2="10" />
    <line x1="21" y1="6" x2="3" y2="6" />
    <line x1="21" y1="14" x2="3" y2="14" />
    <line x1="21" y1="18" x2="9" y2="18" />
  </>);
}

export function IconAlignJustify(props: IconProps) {
  return base(props, <>
    <line x1="21" y1="6" x2="3" y2="6" />
    <line x1="21" y1="10" x2="3" y2="10" />
    <line x1="21" y1="14" x2="3" y2="14" />
    <line x1="21" y1="18" x2="3" y2="18" />
  </>);
}

export function IconList(props: IconProps) {
  return base(props, <>
    <line x1="8" y1="6" x2="21" y2="6" />
    <line x1="8" y1="12" x2="21" y2="12" />
    <line x1="8" y1="18" x2="21" y2="18" />
    <line x1="3" y1="6" x2="3.01" y2="6" />
    <line x1="3" y1="12" x2="3.01" y2="12" />
    <line x1="3" y1="18" x2="3.01" y2="18" />
  </>);
}

export function IconOrderedList(props: IconProps) {
  return base(props, <>
    <line x1="10" y1="6" x2="21" y2="6" />
    <line x1="10" y1="12" x2="21" y2="12" />
    <line x1="10" y1="18" x2="21" y2="18" />
    <path d="M4 6h1v4" />
    <path d="M4 10h2" />
    <path d="M6 18H4c0-1 2-2 2-3s-1-1.5-2-1" />
  </>);
}

export function IconTable(props: IconProps) {
  return base(props, <>
    <path d="M9 3H5a2 2 0 0 0-2 2v4m6-6h10a2 2 0 0 1 2 2v4M9 3v18m0 0h10a2 2 0 0 0 2-2V9M9 21H5a2 2 0 0 1-2-2V9m0 0h18" />
  </>);
}

export function IconImage(props: IconProps) {
  return base(props, <>
    <rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
    <circle cx="8.5" cy="8.5" r="1.5" />
    <polyline points="21 15 16 10 5 21" />
  </>);
}

export function IconPlus(props: IconProps) {
  return base(props, <>
    <line x1="12" y1="5" x2="12" y2="19" />
    <line x1="5" y1="12" x2="19" y2="12" />
  </>);
}

export function IconFolderOpen(props: IconProps) {
  return base(props, <>
    <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
  </>);
}

export function IconSave(props: IconProps) {
  return base(props, <>
    <path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z" />
    <polyline points="17 21 17 13 7 13 7 21" />
    <polyline points="7 3 7 8 15 8" />
  </>);
}

export function IconSettings(props: IconProps) {
  return base(props, <>
    <circle cx="12" cy="12" r="3" />
    <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z" />
  </>);
}

export function IconSearch(props: IconProps) {
  return base(props, <>
    <circle cx="11" cy="11" r="8" />
    <line x1="21" y1="21" x2="16.65" y2="16.65" />
  </>);
}

export function IconPin(props: IconProps) {
  return base(props, <>
    <line x1="12" y1="17" x2="12" y2="22" />
    <path d="M5 17h14l-1.5-6H19.5L18 7H6L4.5 11H5.5L4 17z" />
  </>);
}

export function IconSun(props: IconProps) {
  return base(props, <>
    <circle cx="12" cy="12" r="5" />
    <line x1="12" y1="1" x2="12" y2="3" />
    <line x1="12" y1="21" x2="12" y2="23" />
    <line x1="4.22" y1="4.22" x2="5.64" y2="5.64" />
    <line x1="18.36" y1="18.36" x2="19.78" y2="19.78" />
    <line x1="1" y1="12" x2="3" y2="12" />
    <line x1="21" y1="12" x2="23" y2="12" />
    <line x1="4.22" y1="19.78" x2="5.64" y2="18.36" />
    <line x1="18.36" y1="5.64" x2="19.78" y2="4.22" />
  </>);
}

export function IconMoon(props: IconProps) {
  return base(props, <>
    <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
  </>);
}

export function IconUndo(props: IconProps) {
  return base(props, <>
    <polyline points="1 4 1 10 7 10" />
    <path d="M3.51 15a9 9 0 1 0 2.13-9.36L1 10" />
  </>);
}

export function IconRedo(props: IconProps) {
  return base(props, <>
    <polyline points="23 4 23 10 17 10" />
    <path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10" />
  </>);
}

export function IconCut(props: IconProps) {
  return base(props, <>
    <circle cx="6" cy="6" r="3" />
    <circle cx="6" cy="18" r="3" />
    <line x1="20" y1="4" x2="8.12" y2="15.88" />
    <line x1="14.47" y1="14.48" x2="20" y2="20" />
    <line x1="8.12" y1="8.12" x2="12" y2="12" />
  </>);
}

export function IconCopy(props: IconProps) {
  return base(props, <>
    <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
    <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
  </>);
}

export function IconPaste(props: IconProps) {
  return base(props, <>
    <path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2" />
    <rect x="8" y="2" width="8" height="4" rx="1" ry="1" />
  </>);
}

export function IconPrint(props: IconProps) {
  return base(props, <>
    <polyline points="6 9 6 2 18 2 18 9" />
    <path d="M6 18H4a2 2 0 0 1-2-2v-5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2h-2" />
    <rect x="6" y="14" width="12" height="8" />
  </>);
}

export function IconPdf(props: IconProps) {
  return base(props, <>
    <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
    <polyline points="14 2 14 8 20 8" />
    <text x="7" y="17" fill="currentColor" stroke="none" font-size="7" font-weight="700">PDF</text>
  </>);
}

export function IconNew(props: IconProps) {
  return base(props, <>
    <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
    <polyline points="14 2 14 8 20 8" />
    <line x1="12" y1="18" x2="12" y2="12" />
    <line x1="9" y1="15" x2="15" y2="15" />
  </>);
}

export function IconLink(props: IconProps) {
  return base(props, <>
    <path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71" />
    <path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71" />
  </>);
}

export function IconIndent(props: IconProps) {
  return base(props, <>
    <line x1="3" y1="8" x2="21" y2="8" />
    <line x1="3" y1="16" x2="21" y2="16" />
    <polyline points="8 12 12 16 8 20" />
    <line x1="3" y1="12" x2="12" y2="12" />
  </>);
}

export function IconOutdent(props: IconProps) {
  return base(props, <>
    <line x1="21" y1="8" x2="3" y2="8" />
    <line x1="21" y1="16" x2="3" y2="16" />
    <polyline points="16 12 12 16 16 20" />
    <line x1="21" y1="12" x2="12" y2="12" />
  </>);
}

export function IconClearFormat(props: IconProps) {
  return base(props, <>
    <path d="M4 7V4h16v3" />
    <path d="M9 20h6" />
    <path d="M12 4v9" />
    <line x1="4" y1="20" x2="20" y2="4" />
  </>);
}

export function IconSuperscript(props: IconProps) {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor" {...props}>
      <text x="2" y="18" font-size="14" font-family="serif">x</text>
      <text x="12" y="10" font-size="10" font-family="serif">2</text>
    </svg>
  );
}

export function IconSubscript(props: IconProps) {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor" {...props}>
      <text x="2" y="14" font-size="14" font-family="serif">x</text>
      <text x="12" y="20" font-size="10" font-family="serif">2</text>
    </svg>
  );
}

export function IconTextColor(props: IconProps) {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" {...props}>
      <path d="M4 20h16" stroke-width="3" />
      <path d="M9 4l-4 12h2.5l.8-2.4h5.4L14.5 16H17L13 4H9z" fill="currentColor" stroke="none" />
    </svg>
  );
}

export function IconHighlight(props: IconProps) {
  return base(props, <>
    <path d="M12 20h9" />
    <path d="M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z" />
  </>);
}

export function IconLineSpacing(props: IconProps) {
  return base(props, <>
    <line x1="6" y1="4" x2="6" y2="20" />
    <polyline points="3 7 6 4 9 7" />
    <polyline points="3 17 6 20 9 17" />
    <line x1="12" y1="8" x2="21" y2="8" />
    <line x1="12" y1="12" x2="21" y2="12" />
    <line x1="12" y1="16" x2="21" y2="16" />
  </>);
}

export function IconProperties(props: IconProps) {
  return base(props, <>
    <path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z" />
  </>);
}

export function IconPage(props: IconProps) {
  return base(props, <>
    <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
    <polyline points="14 2 14 8 20 8" />
  </>);
}

export function IconStyles(props: IconProps) {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor" {...props}>
      <text x="4" y="18" font-size="16" font-family="serif" font-weight="700">A</text>
    </svg>
  );
}

export function IconGallery(props: IconProps) {
  return base(props, <>
    <rect x="3" y="3" width="18" height="18" rx="2" />
    <circle cx="8.5" cy="8.5" r="1.5" />
    <polyline points="21 15 16 10 5 21" />
  </>);
}

export function IconNavigator(props: IconProps) {
  return base(props, <>
    <circle cx="12" cy="12" r="10" />
    <polygon points="16.24 7.76 14.12 14.12 7.76 16.24 9.88 9.88 16.24 7.76" />
  </>);
}

export function IconFunctions(props: IconProps) {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor" {...props}>
      <text x="3" y="17" font-size="14" font-style="italic" font-family="serif">ƒx</text>
    </svg>
  );
}

export function IconTransition(props: IconProps) {
  return base(props, <>
    <rect x="2" y="6" width="8" height="12" rx="1" />
    <rect x="14" y="6" width="8" height="12" rx="1" />
    <polyline points="10 12 12 12 14 12" />
    <polyline points="12 9 14 12 12 15" />
  </>);
}

export function IconAnimation(props: IconProps) {
  return base(props, <>
    <polygon points="5 3 19 12 5 21 5 3" />
  </>);
}

export function IconMaster(props: IconProps) {
  return base(props, <>
    <rect x="3" y="3" width="18" height="18" rx="2" />
    <line x1="3" y1="9" x2="21" y2="9" />
    <line x1="9" y1="21" x2="9" y2="9" />
  </>);
}

export function IconSelect(props: IconProps) {
  return base(props, <>
    <path d="M3 3l7.07 16.97 2.51-7.39 7.39-2.51L3 3z" />
  </>);
}

export function IconLine(props: IconProps) {
  return base(props, <>
    <line x1="5" y1="19" x2="19" y2="5" />
  </>);
}

export function IconRect(props: IconProps) {
  return base(props, <>
    <rect x="3" y="3" width="18" height="18" rx="1" />
  </>);
}

export function IconEllipse(props: IconProps) {
  return base(props, <>
    <ellipse cx="12" cy="12" rx="9" ry="6" />
  </>);
}

export function IconArrow(props: IconProps) {
  return base(props, <>
    <line x1="5" y1="12" x2="19" y2="12" />
    <polyline points="12 5 19 12 12 19" />
  </>);
}

export function IconTextBox(props: IconProps) {
  return base(props, <>
    <rect x="3" y="5" width="18" height="14" rx="1" stroke-dasharray="3 2" />
    <path d="M8 9h8M10 15h4M12 9v6" />
  </>);
}

export function IconChart(props: IconProps) {
  return base(props, <>
    <line x1="18" y1="20" x2="18" y2="10" />
    <line x1="12" y1="20" x2="12" y2="4" />
    <line x1="6" y1="20" x2="6" y2="14" />
  </>);
}

export function IconSortAsc(props: IconProps) {
  return base(props, <>
    <path d="M11 5h10" />
    <path d="M11 9h7" />
    <path d="M11 13h4" />
    <path d="M3 17l3 3 3-3" />
    <path d="M6 18V4" />
  </>);
}

export function IconSortDesc(props: IconProps) {
  return base(props, <>
    <path d="M11 5h4" />
    <path d="M11 9h7" />
    <path d="M11 13h10" />
    <path d="M3 7l3-3 3 3" />
    <path d="M6 6v14" />
  </>);
}

export function IconFilter(props: IconProps) {
  return base(props, <>
    <polygon points="22 3 2 3 10 12.46 10 19 14 21 14 12.46 22 3" />
  </>);
}

export function IconCurrency(props: IconProps) {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor" {...props}>
      <text x="5" y="17" font-size="16" font-weight="700">$</text>
    </svg>
  );
}

export function IconPercent(props: IconProps) {
  return base(props, <>
    <line x1="19" y1="5" x2="5" y2="19" />
    <circle cx="6.5" cy="6.5" r="2.5" />
    <circle cx="17.5" cy="17.5" r="2.5" />
  </>);
}

export function IconMerge(props: IconProps) {
  return base(props, <>
    <rect x="3" y="6" width="18" height="12" rx="1" />
    <line x1="12" y1="6" x2="12" y2="18" stroke-dasharray="2 2" />
  </>);
}

export function IconWrap(props: IconProps) {
  return base(props, <>
    <path d="M3 6h18" />
    <path d="M3 12h12a3 3 0 1 1 0 6h-3" />
    <polyline points="12 15 9 18 12 21" />
    <path d="M3 18h4" />
  </>);
}

export function IconAlignTop(props: IconProps) {
  return base(props, <>
    <line x1="3" y1="4" x2="21" y2="4" />
    <rect x="7" y="8" width="4" height="10" />
    <rect x="13" y="8" width="4" height="6" />
  </>);
}

export function IconAlignMiddle(props: IconProps) {
  return base(props, <>
    <line x1="3" y1="12" x2="21" y2="12" />
    <rect x="7" y="5" width="4" height="14" />
    <rect x="13" y="8" width="4" height="8" />
  </>);
}

export function IconAlignBottom(props: IconProps) {
  return base(props, <>
    <line x1="3" y1="20" x2="21" y2="20" />
    <rect x="7" y="6" width="4" height="10" />
    <rect x="13" y="10" width="4" height="6" />
  </>);
}

export function IconSum(props: IconProps) {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor" {...props}>
      <text x="4" y="18" font-size="16" font-family="serif">Σ</text>
    </svg>
  );
}

export function IconEquals(props: IconProps) {
  return base(props, <>
    <line x1="5" y1="9" x2="19" y2="9" />
    <line x1="5" y1="15" x2="19" y2="15" />
  </>);
}

export function IconFreeze(props: IconProps) {
  return base(props, <>
    <rect x="3" y="3" width="18" height="18" rx="1" />
    <line x1="3" y1="9" x2="21" y2="9" />
    <line x1="9" y1="3" x2="9" y2="21" />
  </>);
}

export function IconDuplicate(props: IconProps) {
  return base(props, <>
    <rect x="8" y="8" width="12" height="12" rx="1" />
    <path d="M4 16V5a1 1 0 0 1 1-1h11" />
  </>);
}

export function IconTrash(props: IconProps) {
  return base(props, <>
    <polyline points="3 6 5 6 21 6" />
    <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
  </>);
}

export function IconChevronUp(props: IconProps) {
  return base(props, <polyline points="18 15 12 9 6 15" />);
}

export function IconChevronDown(props: IconProps) {
  return base(props, <polyline points="6 9 12 15 18 9" />);
}

export function IconPresent(props: IconProps) {
  return base(props, <>
    <rect x="2" y="3" width="20" height="14" rx="2" />
    <polygon points="10 8 16 12 10 16 10 8" fill="currentColor" stroke="none" />
    <line x1="8" y1="21" x2="16" y2="21" />
  </>);
}
