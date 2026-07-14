import { useEffect } from "react";
import { usePodcasts, collectSelectedPairs } from "@/store/podcasts";
import { usePrepare } from "@/store/prepare";
import { useTree, deviceSoundUuids } from "@/store/tree";
import {
  onPrepareEnded,
  onPrepareEpisodeFailed,
  onPrepareEpisodeReady,
  onPrepareProgress,
} from "@/lib/ipc";

export function usePrepareOrchestration() {
  const subscriptions = usePodcasts((s) => s.subscriptions);
  const episodesByFeed = usePodcasts((s) => s.episodesByFeed);
  const episodeMeta = usePodcasts((s) => s.episodeMeta);
  const syncState = usePodcasts((s) => s.syncState);
  const treeFolders = useTree((s) => s.folders);

  const assetsKey = JSON.stringify({
    cats: (syncState?.manualCategories ?? []).map((c) => c.imageSource).sort(),
    ovr: Object.entries(syncState?.folderImageOverrides ?? {}).sort(),
    epOvr: Object.entries(syncState?.episodeImageOverrides ?? {}).sort(),
  });

  useEffect(() => {
    const listeners = [
      onPrepareEpisodeReady((guid) => usePrepare.getState().markReady(guid)),
      onPrepareEpisodeFailed((p) => usePrepare.getState().markFailed(p.guid, p.error)),
      onPrepareProgress((p) => usePrepare.getState().markProgress(p.guid, p.fraction)),
      onPrepareEnded(() => usePrepare.getState().onEnded()),
    ];
    return () => listeners.forEach((l) => void l.then((unlisten) => unlisten()));
  }, []);

  useEffect(() => {
    const onDevice = new Set(deviceSoundUuids(treeFolders));
    const pairs = collectSelectedPairs(subscriptions, episodesByFeed).filter((p) => {
      const uuid = episodeMeta[p.episode.guid]?.uuid;
      return !(uuid && onDevice.has(uuid));
    });
    const handle = setTimeout(() => void usePrepare.getState().reconcile(pairs, assetsKey), 500);
    return () => clearTimeout(handle);
  }, [subscriptions, episodesByFeed, assetsKey, treeFolders, episodeMeta]);
}
