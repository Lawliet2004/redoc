/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_TELEMETRY?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}

interface GridPerformanceProbe {
  measureGrid: (frames?: number) => {
    frames: number;
    durationMs: number;
    averageFrameMs: number;
    fps: number;
  };
}

interface Window {
  __perf?: GridPerformanceProbe;
}
