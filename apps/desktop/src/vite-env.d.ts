/// <reference types="vite/client" />

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
