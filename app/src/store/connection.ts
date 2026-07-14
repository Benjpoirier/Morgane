import { create } from "zustand";
import type { ConnectionStatus } from "@/lib/types";
import { usePodcasts } from "./podcasts";
import { useDevices } from "./devices";
import { useTree } from "./tree";

export type NetworkContext = "merlin" | "internet" | "offline" | "unknown";

interface ConnectionState {
  host: string;
  port: number;
  isConnected: boolean;
  isTesting: boolean;
  statusMessage: string;
  lastPingMs: number | null;
  networkContext: NetworkContext;
  setNetworkContext: (context: NetworkContext) => void;

  deviceMac: string | null;

  deviceName: string | null;

  pendingNameMac: string | null;
  clearPendingName: () => void;
  setHost: (host: string) => void;
  setPort: (port: number) => void;
  setTesting: (testing: boolean) => void;

  applyStatus: (status: ConnectionStatus) => void;

  pollingIntervalMs: () => number;
}

export const useConnection = create<ConnectionState>((set, get) => ({
  host: "192.168.4.1",
  port: 50000,
  isConnected: false,
  isTesting: false,
  statusMessage: "",
  lastPingMs: null,
  networkContext: "unknown",
  setNetworkContext: (networkContext) => set({ networkContext }),
  deviceMac: null,
  deviceName: null,
  pendingNameMac: null,
  clearPendingName: () => set({ pendingNameMac: null }),
  setHost: (host) => set({ host }),
  setPort: (port) => set({ port }),
  setTesting: (isTesting) => set({ isTesting }),
  applyStatus: (status) => {
    if (status.busy) return;
    const previousMac = get().deviceMac;
    set({
      isConnected: status.connected,
      lastPingMs: status.latencyMs ?? get().lastPingMs,
      statusMessage: status.message ?? get().statusMessage,
      deviceMac: status.deviceMac ?? previousMac,
      deviceName: status.deviceName ?? get().deviceName,

      networkContext: status.connected ? "merlin" : get().networkContext,
    });

    if (status.deviceMac && status.deviceMac !== previousMac) {
      const podcasts = usePodcasts.getState();
      void podcasts.reloadSyncState();
      void podcasts.reloadSubscriptions().then(() => podcasts.loadAllFeeds());
      void useDevices.getState().reload();

      useTree.getState().reset();
    }

    if (status.newlyRegistered && status.deviceMac) {
      set({ pendingNameMac: status.deviceMac });
    }
  },
  pollingIntervalMs: () => 4_000,
}));
