import { create } from "zustand";
import { convertFileSrc } from "@tauri-apps/api/core";

export interface PlayerTrack {
  guid: string;
  title: string;
  audioUrl: string;
  imageUrl: string | null;
}

interface PlayerState {
  queue: PlayerTrack[];
  index: number;
  current: PlayerTrack | null;
  playing: boolean;
  loading: boolean;
  currentTime: number;
  duration: number;

  play: (queue: PlayerTrack[], index: number) => void;

  toggleTrack: (queue: PlayerTrack[], index: number) => void;
  togglePlay: () => void;
  next: () => void;
  previous: () => void;
  seek: (time: number) => void;
  skip: (delta: number) => void;
  stop: () => void;
}

let element: HTMLAudioElement | null = null;

function playableSrc(audioUrl: string): string {
  const path = audioUrl.startsWith("file://") ? audioUrl.slice("file://".length) : null;
  return path ? convertFileSrc(path) : audioUrl;
}

export const usePlayer = create<PlayerState>((set, get) => {
  function ensureElement(): HTMLAudioElement {
    if (element) return element;
    const audio = new Audio();
    audio.addEventListener("timeupdate", () => set({ currentTime: audio.currentTime }));
    const syncDuration = () =>
      set({ duration: Number.isFinite(audio.duration) ? audio.duration : 0 });
    audio.addEventListener("durationchange", syncDuration);
    audio.addEventListener("loadedmetadata", syncDuration);
    audio.addEventListener("playing", () => set({ playing: true, loading: false }));
    audio.addEventListener("play", () => set({ playing: true }));
    audio.addEventListener("pause", () => set({ playing: false }));
    audio.addEventListener("waiting", () => set({ loading: true }));
    audio.addEventListener("ended", () => get().next());
    audio.addEventListener("error", () => set({ playing: false, loading: false }));
    element = audio;
    return audio;
  }

  return {
    queue: [],
    index: -1,
    current: null,
    playing: false,
    loading: false,
    currentTime: 0,
    duration: 0,

    play: (queue, index) => {
      const track = queue[index];
      if (!track) return;
      const audio = ensureElement();
      audio.src = playableSrc(track.audioUrl);
      audio.currentTime = 0;
      set({ queue, index, current: track, playing: false, loading: true, currentTime: 0, duration: 0 });
      void audio.play().catch(() => set({ playing: false, loading: false }));
    },

    toggleTrack: (queue, index) => {
      const track = queue[index];
      if (track && get().current?.guid === track.guid) get().togglePlay();
      else get().play(queue, index);
    },

    togglePlay: () => {
      if (!element || !get().current) return;
      if (get().playing) element.pause();
      else void element.play().catch(() => {});
    },

    next: () => {
      const { queue, index } = get();
      if (index + 1 < queue.length) get().play(queue, index + 1);
      else get().stop();
    },

    previous: () => {
      if (element && element.currentTime > 3) {
        element.currentTime = 0;
        return;
      }
      const { queue, index } = get();
      if (index > 0) get().play(queue, index - 1);
      else if (element) element.currentTime = 0;
    },

    seek: (time) => {
      if (!element) return;
      element.currentTime = time;
      set({ currentTime: time });
    },

    skip: (delta) => {
      if (!element) return;
      const max = Number.isFinite(element.duration) ? element.duration : element.currentTime + delta;
      const time = Math.max(0, Math.min(max, element.currentTime + delta));
      element.currentTime = time;
      set({ currentTime: time });
    },

    stop: () => {
      if (element) {
        element.pause();
        element.removeAttribute("src");
        element.load();
      }
      set({
        queue: [],
        index: -1,
        current: null,
        playing: false,
        loading: false,
        currentTime: 0,
        duration: 0,
      });
    },
  };
});
