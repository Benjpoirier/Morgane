import { useMemo } from "react";
import { usePodcasts, collectSelectedPairs } from "@/store/podcasts";
import { usePrepare } from "@/store/prepare";
import { useSync } from "@/store/sync";
import { useTree, deviceSoundUuids, alreadyOnDevice } from "@/store/tree";
import { useConnection } from "@/store/connection";
import type { PendingGroup } from "@/lib/types";

export function useSyncFooter({
  pendingGroups,
  unplaced,
  missingImages,
  changeTotal,
}: {
  pendingGroups: PendingGroup[];
  unplaced: number;
  missingImages: number;
  changeTotal: number;
}) {
  const { host, port, isConnected } = useConnection();
  const syncState = usePodcasts((s) => s.syncState);
  const subscriptions = usePodcasts((s) => s.subscriptions);
  const episodesByFeed = usePodcasts((s) => s.episodesByFeed);
  const episodeMeta = usePodcasts((s) => s.episodeMeta);
  const tree = useTree();
  const sync = useSync();
  const prepared = usePrepare((s) => s.prepared);
  const failed = usePrepare((s) => s.failed);

  const deletions =
    (syncState?.syncedRecords.filter((r) => r.pendingDeletion).length ?? 0) +
    tree.pendingOrphanDeletions.length;

  const selectedPairs = useMemo(
    () => collectSelectedPairs(subscriptions, episodesByFeed),
    [subscriptions, episodesByFeed],
  );
  const deviceUuids = useMemo(() => new Set(deviceSoundUuids(tree.folders)), [tree.folders]);

  const needPreparePairs = selectedPairs.filter((p) => {
    const uuid = episodeMeta[p.episode.guid]?.uuid;
    return !(uuid && deviceUuids.has(uuid));
  });
  const needPrepareGuids = needPreparePairs.map((p) => p.episode.guid);
  const notPrepared = needPrepareGuids.filter((g) => !prepared.has(g));
  const failedCount = needPrepareGuids.filter((g) => g in failed).length;
  const allPrepared = notPrepared.length === 0;

  const canSync =
    !sync.running &&
    unplaced === 0 &&
    missingImages === 0 &&
    changeTotal > 0 &&
    allPrepared &&
    isConnected;

  const launch = async () => {
    const pairs = collectSelectedPairs(subscriptions, episodesByFeed).map(
      ({ subscription, episode }) => ({ subscription, episode }),
    );
    const filesToDelete: Record<string, string[]> = {};
    syncState?.syncedRecords
      .filter((r) => r.pendingDeletion)
      .forEach((r) => {
        filesToDelete[r.episodeUuid] = [`${r.episodeUuid}.mp3`, `${r.episodeUuid}.jpg`];
      });

    tree.pendingOrphanDeletions.forEach((name) => {
      filesToDelete[name] = [name];
    });
    await sync.start({
      pairs,
      host,
      port,
      alreadySynced: alreadyOnDevice(syncState?.syncedRecords, tree.folders),
      filesToDelete,
      treeEdits: tree.pendingEdits,
    });
  };

  const status = useMemo(() => {
    let s = `${pendingGroups.length} groupe(s) · ${deletions} suppression(s) · ${tree.pendingEdits.length} édition(s)`;
    if (unplaced > 0) s += ` · ${unplaced} non placé(s)`;
    if (missingImages > 0) s += ` · ${missingImages} visuel(s) manquant(s)`;
    return s;
  }, [pendingGroups.length, deletions, tree.pendingEdits.length, unplaced, missingImages]);

  return {
    host,
    port,
    isConnected,
    status,
    canSync,
    launch,
    allPrepared,
    needPreparePairs,
    needCount: needPrepareGuids.length,
    notPreparedCount: notPrepared.length,
    failedCount,
    unplaced,
    missingImages,
  };
}
