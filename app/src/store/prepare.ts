import { create } from "zustand";
import type { SelectedPair } from "@/lib/types";
import { prepareSelection, preparedGuids } from "@/lib/ipc";

interface PrepareState {

  prepared: Set<string>;

  failed: Record<string, string>;

  preparing: boolean;

  dirty: boolean;

  currentGuid: string | null;

  progress: number;
  lastPairs: SelectedPair[];
  lastAssetsKey: string;

  preparedAssetsKey: string;
  reconcile: (pairs: SelectedPair[], assetsKey: string) => Promise<void>;
  retry: () => Promise<void>;
  markReady: (guid: string) => void;
  markFailed: (guid: string, error: string) => void;
  markProgress: (guid: string, fraction: number) => void;
  onEnded: () => void;
}

function pruneFailed(failed: Record<string, string>, guids: string[]): Record<string, string> {
  const kept = new Set(guids);
  return Object.fromEntries(Object.entries(failed).filter(([guid]) => kept.has(guid)));
}

export const usePrepare = create<PrepareState>((set, get) => ({
  prepared: new Set(),
  failed: {},
  preparing: false,
  dirty: false,
  currentGuid: null,
  progress: 0,
  lastPairs: [],
  lastAssetsKey: "",
  preparedAssetsKey: "\u0000never",

  reconcile: async (pairs, assetsKey) => {
    set({ lastPairs: pairs, lastAssetsKey: assetsKey });
    const guids = pairs.map((p) => p.episode.guid);
    if (guids.length === 0 && !assetsKey) {
      set({ prepared: new Set(), failed: {}, preparedAssetsKey: assetsKey });
      return;
    }
    let prepared: Set<string>;
    try {
      prepared = new Set(await preparedGuids(guids));
    } catch {
      return;
    }
    set((s) => ({ prepared, failed: pruneFailed(s.failed, guids) }));
    const st = get();

    const episodesResolved = guids.every((g) => prepared.has(g) || g in st.failed);
    const assetsChanged = assetsKey !== st.preparedAssetsKey;
    if (episodesResolved && !assetsChanged) return;
    if (st.preparing) {
      set({ dirty: true });
      return;
    }
    set({ preparing: true, dirty: false, preparedAssetsKey: assetsKey });
    try {
      await prepareSelection(pairs);
    } catch {

      set({ preparing: false, dirty: true, preparedAssetsKey: st.preparedAssetsKey });
    }
  },

  retry: async () => {
    set({ failed: {}, preparedAssetsKey: "\u0000never" });
    await get().reconcile(get().lastPairs, get().lastAssetsKey);
  },

  markReady: (guid) =>
    set((s) => {
      const prepared = new Set(s.prepared);
      prepared.add(guid);
      const failed = { ...s.failed };
      delete failed[guid];
      const clearCurrent = s.currentGuid === guid;
      return {
        prepared,
        failed,
        currentGuid: clearCurrent ? null : s.currentGuid,
        progress: clearCurrent ? 0 : s.progress,
      };
    }),

  markFailed: (guid, error) =>
    set((s) => ({
      failed: { ...s.failed, [guid]: error },
      currentGuid: s.currentGuid === guid ? null : s.currentGuid,
      progress: s.currentGuid === guid ? 0 : s.progress,
    })),

  markProgress: (guid, fraction) => set({ currentGuid: guid, progress: fraction }),

  onEnded: () => {
    set({ preparing: false, currentGuid: null, progress: 0 });
    void get().reconcile(get().lastPairs, get().lastAssetsKey);
  },
}));
