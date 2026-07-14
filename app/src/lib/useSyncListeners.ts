import { useEffect } from "react";
import {
  onDeletionsCompleted,
  onEpisodeUploaded,
  onSyncEnded,
  onSyncLog,
  onSyncPhase,
  onSyncStep,
  onThumbnailReady,
  onTreeEditsApplied,
} from "./ipc";
import { useSync } from "@/store/sync";
import { usePodcasts } from "@/store/podcasts";
import { useTree } from "@/store/tree";
import { useConnection } from "@/store/connection";

export function useSyncListeners() {
  useEffect(() => {
    const unlisteners = Promise.all([
      onSyncPhase((p) => useSync.getState().applyPhase(p)),
      onSyncLog((line) => useSync.getState().pushLog(line)),
      onSyncStep((s) => useSync.getState().setStep(s.label, s.fraction)),
      onEpisodeUploaded((uuid) => useSync.getState().markUploaded(uuid)),
      onDeletionsCompleted(() => {
        void usePodcasts.getState().reloadSyncState();

        void useTree.getState().clearPendingOrphanDeletions();
      }),
      onTreeEditsApplied(() => {

        void useTree.getState().clearPendingEdits();
      }),
      onSyncEnded(() => {
        useSync.getState().ended();
        void usePodcasts.getState().reloadSyncState();
        const { phase } = useSync.getState();
        if (phase.kind === "finished") {
          const { host, port } = useConnection.getState();
          void useTree.getState().refresh(host, port);
        }
      }),
      onThumbnailReady((p) => useTree.getState().setThumbnail(p.uuid, p.path)),
    ]);
    return () => {
      void unlisteners.then((fns) => fns.forEach((fn) => fn()));
    };
  }, []);
}
