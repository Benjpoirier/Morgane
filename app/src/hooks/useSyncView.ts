import { useEffect, useMemo, useState } from "react";
import type { DragEndEvent } from "@dnd-kit/core";
import { usePodcasts, collectSelectedPairs } from "@/store/podcasts";
import { useTree, alreadyOnDevice } from "@/store/tree";
import { useConnection } from "@/store/connection";
import { computePendingGroups, setCategoryAssignment } from "@/lib/ipc";
import { buildChangeset } from "@/lib/changeset";
import type { PendingGroup } from "@/lib/types";

const groupKey = (feedUrl: string, gk: string) => `${feedUrl}|${gk}`;

export function useSyncView() {
  const [pendingGroups, setPendingGroups] = useState<PendingGroup[]>([]);

  const subscriptions = usePodcasts((s) => s.subscriptions);
  const episodesByFeed = usePodcasts((s) => s.episodesByFeed);
  const syncState = usePodcasts((s) => s.syncState);
  const reloadSyncState = usePodcasts((s) => s.reloadSyncState);
  const tree = useTree();
  const { host, port } = useConnection();

  useEffect(() => {
    if (!tree.hasLoadedOnce && !tree.loading) void tree.refresh(host, port);

  }, []);

  useEffect(() => {
    let stale = false;
    const pairs = collectSelectedPairs(subscriptions, episodesByFeed);
    const alreadySynced = alreadyOnDevice(syncState?.syncedRecords, tree.folders);
    if (pairs.length === 0) {
      setPendingGroups([]);
      return;
    }
    void computePendingGroups(pairs, alreadySynced).then((groups) => {
      if (!stale) setPendingGroups(groups);
    });
    return () => {
      stale = true;
    };
  }, [subscriptions, episodesByFeed, syncState, tree.folders]);

  const assignments = syncState?.categoryAssignments ?? {};
  const changeset = useMemo(
    () =>
      buildChangeset(
        pendingGroups,
        syncState,
        tree.editDetails,
        tree.pendingOrphanDeletions,
        tree.folders,
      ),
    [pendingGroups, syncState, tree.editDetails, tree.pendingOrphanDeletions, tree.folders],
  );

  const unplaced = pendingGroups.filter(
    (g) => !assignments[groupKey(g.feedUrl, g.groupKey)],
  ).length;
  const missingImages = pendingGroups.filter((g) =>
    g.episodes.some(
      (e) => !e.imageUrl && !g.feedImageUrl && !syncState?.episodeImageOverrides[e.guid],
    ),
  ).length;
  const assignedCounts = useMemo(() => {
    const counts: Record<string, number> = {};
    for (const g of pendingGroups) {
      const target = assignments[groupKey(g.feedUrl, g.groupKey)]?.targetCategoryUuid;
      if (target) counts[target] = (counts[target] ?? 0) + 1;
    }
    return counts;
  }, [pendingGroups, assignments]);

  const selectedCount = useMemo(
    () => collectSelectedPairs(subscriptions, episodesByFeed).length,
    [subscriptions, episodesByFeed],
  );
  const deletionCount =
    (syncState?.syncedRecords.filter((r) => r.pendingDeletion).length ?? 0) +
    tree.pendingOrphanDeletions.length;
  const integrityMissing =
    tree.integrityIssues?.reduce((n, i) => n + i.missingFiles.length, 0) ?? 0;

  const onDragEnd = async (event: DragEndEvent) => {
    const over = event.over;
    const active = event.active.data.current as
      | { kind: "pendingGroup"; feedUrl: string; groupKey: string }
      | { kind: "treeNode"; uuid: string }
      | undefined;
    if (!over || !active) return;
    const target = over.data.current as
      | { kind: "category" | "folder"; uuid: string; title: string }
      | undefined;
    if (!target) return;
    if (active.kind === "pendingGroup" && target.kind === "category") {
      await setCategoryAssignment({
        feedUrl: active.feedUrl,
        groupKey: active.groupKey,
        targetCategoryUuid: target.uuid,
        targetCategoryTitle: target.title,
      });
      await reloadSyncState();
    } else if (active.kind === "treeNode" && target.uuid !== active.uuid) {
      await tree.moveNode(active.uuid, target.uuid);
    }
  };

  return {
    tree,
    host,
    port,
    pendingGroups,
    changeset,
    unplaced,
    missingImages,
    assignedCounts,
    selectedCount,
    deletionCount,
    integrityMissing,
    onDragEnd,
  };
}
