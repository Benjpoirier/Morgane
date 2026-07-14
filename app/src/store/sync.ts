import { create } from "zustand";
import { cancelSync, startSync, type SyncLaunch } from "@/lib/ipc";
import type { SyncProgressPhase } from "@/lib/types";
import { remainingSeconds } from "@/lib/eta";

export type SyncPhase =
  | { kind: "idle" }
  | { kind: "preparing"; done: number; total: number }
  | { kind: "connecting" }
  | { kind: "sending"; bytesDone: number; bytesTotal: number }
  | { kind: "finished"; count: number }
  | { kind: "cancelled" }
  | { kind: "failed"; message: string };

function fromProgress(p: SyncProgressPhase): SyncPhase {
  switch (p.type) {
    case "preparing":
      return { kind: "preparing", ...p.data };
    case "connecting":
      return { kind: "connecting" };
    case "sending":
      return { kind: "sending", ...p.data };
    case "finished":
      return { kind: "finished", count: p.data.count };
    case "failed":
      return { kind: "failed", message: p.data };
  }
}

export function formatBytes(bytes: number): string {
  const units = ["octets", "Ko", "Mo", "Go"];
  let value = bytes;
  let unit = 0;
  while (value >= 1000 && unit < units.length - 1) {
    value /= 1000;
    unit += 1;
  }
  return unit === 0 ? `${bytes} octets` : `${value.toFixed(1)} ${units[unit]}`;
}

interface Sample {
  t: number;
  bytes: number;
}

interface SyncState {
  phase: SyncPhase;
  log: string[];
  stepLabel: string | null;
  stepFraction: number;
  uploadedUuids: Set<string>;
  speedBytesPerSec: number | null;
  running: boolean;
  modalOpen: boolean;
  samples: Sample[];

  begin: () => void;
  applyPhase: (p: SyncProgressPhase) => void;
  pushLog: (line: string) => void;
  setStep: (label: string | null, fraction: number) => void;
  markUploaded: (uuid: string) => void;
  ended: () => void;
  setModalOpen: (open: boolean) => void;

  start: (launch: SyncLaunch) => Promise<void>;
  cancel: () => Promise<void>;

  currentActivity: () => string;
  sendingDetail: () => string | null;

  etaSeconds: () => number | null;
}

export const useSync = create<SyncState>((set, get) => ({
  phase: { kind: "idle" },
  log: [],
  stepLabel: null,
  stepFraction: 0,
  uploadedUuids: new Set(),
  speedBytesPerSec: null,
  running: false,
  modalOpen: false,
  samples: [],

  begin: () =>
    set({
      log: [],
      stepLabel: null,
      stepFraction: 0,
      uploadedUuids: new Set(),
      speedBytesPerSec: null,
      running: true,
      modalOpen: true,
      samples: [],
      phase: { kind: "connecting" },
    }),

  applyPhase: (p) =>
    set((s) => {

      let speed = s.speedBytesPerSec;
      let samples = s.samples;
      if (p.type === "sending") {
        const now = performance.now();
        samples = [...s.samples, { t: now, bytes: p.data.bytesDone }].filter(
          (x) => now - x.t < 3000,
        );
        if (samples.length >= 2) {
          const oldest = samples[0];
          const elapsed = (now - oldest.t) / 1000;
          if (elapsed > 0.2) {
            speed = Math.max(0, p.data.bytesDone - oldest.bytes) / elapsed;
          }
        }
      }

      if (s.phase.kind === "cancelled") return { samples, speedBytesPerSec: speed };
      return { phase: fromProgress(p), samples, speedBytesPerSec: speed };
    }),

  pushLog: (line) => set((s) => ({ log: [...s.log, line] })),
  setStep: (stepLabel, stepFraction) => set({ stepLabel, stepFraction }),
  markUploaded: (uuid) =>
    set((s) => ({ uploadedUuids: new Set(s.uploadedUuids).add(uuid) })),
  ended: () => set({ running: false }),
  setModalOpen: (modalOpen) => set({ modalOpen }),

  start: async (launch) => {
    get().begin();
    await startSync(launch);
  },
  cancel: async () => {
    set({ phase: { kind: "cancelled" } });
    await cancelSync();
  },

  currentActivity: () => {
    const s = get();
    if (s.stepLabel) return s.stepLabel;
    return s.log[s.log.length - 1] ?? "…";
  },
  sendingDetail: () => {
    const s = get();
    if (s.phase.kind !== "sending" || s.phase.bytesTotal === 0) return null;
    let text = `${formatBytes(s.phase.bytesDone)} / ${formatBytes(s.phase.bytesTotal)}`;
    if (s.speedBytesPerSec && s.speedBytesPerSec > 0) {
      text += ` · ${formatBytes(Math.round(s.speedBytesPerSec))}/s`;
    }
    return text;
  },
  etaSeconds: () => {
    const s = get();
    if (s.phase.kind !== "sending") return null;
    return remainingSeconds(s.speedBytesPerSec, s.phase.bytesDone, s.phase.bytesTotal);
  },
}));
