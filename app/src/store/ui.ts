import { create } from "zustand";

export type Section = "connect" | "podcasts" | "sync" | "errors";

interface UiState {
  section: Section;
  setSection: (section: Section) => void;
}

export const useUi = create<UiState>((set) => ({
  section: "connect",
  setSection: (section) => set({ section }),
}));
