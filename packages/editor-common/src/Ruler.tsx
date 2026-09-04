interface RulerProps {
  /** Page content width in px (default A4-ish 816) */
  pageWidth?: number;
  /** Left margin in px */
  leftMargin?: number;
  /** Right margin in px */
  rightMargin?: number;
  /** Zoom percent */
  zoom?: number;
  /** Tab stop positions in px from content left */
  tabStops?: number[];
  /** Called when user clicks ruler to add a tab stop */
  onAddTabStop?: (positionPx: number) => void;
}

/** Visual horizontal ruler with cm tick marks, margin indicators, and tab stops. */
export function Ruler(props: RulerProps) {
  const pageWidth = () => props.pageWidth ?? 816;
  const left = () => props.leftMargin ?? 96;
  const right = () => props.rightMargin ?? 96;
  const zoom = () => (props.zoom ?? 100) / 100;
  const scaledWidth = () => pageWidth() * zoom();
  const tabStops = () => props.tabStops ?? [];

  // ~37.8 px per cm at 96dpi
  const ticks = () => {
    const cm = 37.795;
    const count = Math.ceil(pageWidth() / cm) + 1;
    return Array.from({ length: count }, (_, i) => i);
  };

  const handleRulerClick = (event: MouseEvent) => {
    if (!props.onAddTabStop) return;
    const target = event.currentTarget as HTMLElement;
    const rect = target.getBoundingClientRect();
    const x = (event.clientX - rect.left) / zoom();
    if (x < left() || x > pageWidth() - right()) return;
    props.onAddTabStop(Math.round(x));
  };

  return (
    <div class="g-ruler g-no-print" aria-hidden="true">
      <div
        style={{
          position: "relative",
          height: "100%",
          margin: "0 auto",
          width: `${scaledWidth()}px`,
        }}
        onClick={handleRulerClick}
      >
        {/* margin bands */}
        <div
          style={{
            position: "absolute",
            left: 0,
            top: 0,
            bottom: 0,
            width: `${left() * zoom()}px`,
            background: "rgba(0,0,0,0.18)",
          }}
        />
        <div
          style={{
            position: "absolute",
            right: 0,
            top: 0,
            bottom: 0,
            width: `${right() * zoom()}px`,
            background: "rgba(0,0,0,0.18)",
          }}
        />
        {/* margin markers */}
        <div
          style={{
            position: "absolute",
            left: `${left() * zoom() - 4}px`,
            top: "2px",
            width: "0",
            height: "0",
            "border-left": "4px solid transparent",
            "border-right": "4px solid transparent",
            "border-top": "6px solid var(--text-muted)",
          }}
        />
        <div
          style={{
            position: "absolute",
            left: `${(pageWidth() - right()) * zoom() - 4}px`,
            top: "2px",
            width: "0",
            height: "0",
            "border-left": "4px solid transparent",
            "border-right": "4px solid transparent",
            "border-top": "6px solid var(--text-muted)",
          }}
        />
        {tabStops().map((stop) => (
          <div
            style={{
              position: "absolute",
              left: `${stop * zoom()}px`,
              bottom: "0",
              width: "0",
              height: "0",
              "border-left": "4px solid transparent",
              "border-right": "4px solid transparent",
              "border-bottom": "6px solid var(--doc-accent)",
              transform: "translateX(-4px)",
            }}
          />
        ))}
        {ticks().map((i) => {
          const x = i * 37.795 * zoom();
          const major = i % 1 === 0;
          return (
            <div
              style={{
                position: "absolute",
                left: `${x}px`,
                bottom: 0,
                width: "1px",
                height: major ? "10px" : "5px",
                background: "var(--text-muted)",
              }}
            >
              {i > 0 && i % 2 === 0 ? (
                <span
                  style={{
                    position: "absolute",
                    top: "-2px",
                    left: "2px",
                    "font-size": "8px",
                    color: "var(--text-muted)",
                    "white-space": "nowrap",
                  }}
                >
                  {i}
                </span>
              ) : null}
            </div>
          );
        })}
      </div>
    </div>
  );
}
