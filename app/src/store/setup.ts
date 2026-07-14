import { create } from "zustand";
import { ffmpegReady, downloadFfmpeg, onFfmpegProgress } from "@/lib/ipc";
import type { FfmpegProgressPayload } from "@/lib/types";

interface SetupState {
  ready: boolean | null;
  progress: FfmpegProgressPayload | null;
  error: string | null;
  downloading: boolean;
  check: () => Promise<void>;
  download: () => Promise<void>;
}

export const useSetup = create<SetupState>((set, get) => ({
  ready: null,
  progress: null,
  error: null,
  downloading: false,
  check: async () => {
    try {
      set({ ready: await ffmpegReady() });
    } catch {
      set({ ready: false });
    }
  },
  download: async () => {
    if (get().downloading) return;
    set({ downloading: true, error: null, progress: null });
    let unlisten = () => {};
    try {
      unlisten = await onFfmpegProgress((p) => set({ progress: p }));
      await downloadFfmpeg();
      set({ ready: true });
    } catch (e) {
      set({ error: String(e) });
    } finally {
      set({ downloading: false });
      unlisten();
    }
  },
}));
