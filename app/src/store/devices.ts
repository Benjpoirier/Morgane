import { create } from "zustand";
import type { RegisteredDevice } from "@/lib/types";
import {
  listRegisteredDevices,
  removeRegisteredDevice,
  renameRegisteredDevice,
  setActiveDevice,
} from "@/lib/ipc";
import { usePodcasts } from "./podcasts";
import { useTree } from "./tree";

async function reloadPodcastsForActiveDevice() {
  const podcasts = usePodcasts.getState();
  useTree.getState().reset();
  await podcasts.reloadSubscriptions();
  void podcasts.loadAllFeeds();
  void podcasts.reloadSyncState();
}

interface DevicesState {
  devices: RegisteredDevice[];
  reload: () => Promise<void>;
  setActive: (mac: string) => Promise<void>;
  remove: (mac: string) => Promise<void>;
  rename: (mac: string, name: string) => Promise<void>;
}

export const useDevices = create<DevicesState>((set) => ({
  devices: [],
  reload: async () => {
    try {
      set({ devices: await listRegisteredDevices() });
    } catch {

    }
  },
  setActive: async (mac) => {
    await setActiveDevice(mac);
    await useDevices.getState().reload();
    await reloadPodcastsForActiveDevice();
  },
  remove: async (mac) => {
    await removeRegisteredDevice(mac);
    await useDevices.getState().reload();
    await reloadPodcastsForActiveDevice();
  },
  rename: async (mac, name) => {
    await renameRegisteredDevice(mac, name);
    await useDevices.getState().reload();
  },
}));
