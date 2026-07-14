import { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { testConnection, checkInternet } from "./ipc";
import { useConnection } from "@/store/connection";

let lastProbe = 0;

export async function probeNetwork(force = false): Promise<void> {
  const now = Date.now();
  if (!force && now - lastProbe < 2000) return;
  lastProbe = now;
  const { host, port, applyStatus, setNetworkContext } = useConnection.getState();
  let status;
  try {
    status = await testConnection(host, port, false);
  } catch {
    return;
  }
  applyStatus(status);
  if (status.busy) return;
  if (!status.connected) {
    const online = await checkInternet().catch(() => false);
    setNetworkContext(online ? "internet" : "offline");
  }
}

export function useConnectionPolling() {
  const isConnected = useConnection((s) => s.isConnected);

  useEffect(() => {
    if (isConnected) return;
    let cancelled = false;
    let timer: number | null = null;
    const tick = async () => {
      await probeNetwork(true);
      if (!cancelled && !useConnection.getState().isConnected) {
        timer = window.setTimeout(tick, useConnection.getState().pollingIntervalMs());
      }
    };
    void tick();
    return () => {
      cancelled = true;
      if (timer !== null) window.clearTimeout(timer);
    };
  }, [isConnected]);

  useEffect(() => {
    const unlisten = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (focused) void probeNetwork();
    });
    return () => void unlisten.then((u) => u());
  }, []);
}
