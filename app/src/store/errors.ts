import { create } from "zustand";

export interface CapturedError {
  id: number;
  timestamp: number;
  context: string;
  message: string;
}

const MAX_ENTRIES = 200;
let nextId = 1;

interface ErrorState {
  entries: CapturedError[];
  record: (context: string, message: string) => void;
  clear: () => void;
}

export const useErrors = create<ErrorState>((set) => ({
  entries: [],
  record: (context, message) =>
    set((s) => ({
      entries: [
        { id: nextId++, timestamp: Date.now(), context, message },
        ...s.entries,
      ].slice(0, MAX_ENTRIES),
    })),
  clear: () => set({ entries: [] }),
}));
