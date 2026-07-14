import { useEffect } from "react";
import { usePodcasts } from "@/store/podcasts";
import { useDevices } from "@/store/devices";

export function useAppBootstrap() {
  useEffect(() => {
    void useDevices.getState().reload();
    const { reloadSubscriptions, reloadSyncState, loadAllFeeds } = usePodcasts.getState();
    void reloadSyncState();
    void reloadSubscriptions().then(() => loadAllFeeds());
  }, []);
}
