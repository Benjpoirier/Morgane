import type { EditDetail, PendingGroup, PlaylistFolder, TreeEdit } from "./types";
import type { SyncStateSnapshot } from "./ipc";

export type UndoAction =
  | { kind: "deselectGroup"; feedUrl: string; guids: string[] }
  | { kind: "cancelEdit"; uuid: string; editType: TreeEdit["type"] }
  | { kind: "unmarkDeletion"; episodeUuid: string }
  | { kind: "toggleOrphan"; name: string }
  | { kind: "clearFolderImage"; uuid: string };

export interface Addition {
  group: PendingGroup;
  targetCategoryTitle: string | null;
  renamed: boolean;
  hasImage: boolean;
  undo: UndoAction;
}

export interface SimpleChange {
  id: string;
  label: string;
  detail?: string;
  undo: UndoAction;
}

export interface Changeset {
  additions: Addition[];
  modifications: SimpleChange[];
  deletions: SimpleChange[];
  total: number;
}

const key = (feedUrl: string, groupKey: string) => `${feedUrl}|${groupKey}`;

function folderTitles(folders: PlaylistFolder[]): Map<string, string> {
  const map = new Map<string, string>();
  const walk = (f: PlaylistFolder) => {
    map.set(f.uuid, f.title);
    for (const c of f.children) {
      if (c.kind === "folder") walk(c);
      else map.set(c.uuid, c.title);
    }
  };
  for (const f of folders) walk(f);
  return map;
}

export function buildChangeset(
  pendingGroups: PendingGroup[],
  syncState: SyncStateSnapshot | null,
  editDetails: EditDetail[],
  pendingOrphanDeletions: string[],
  folders: PlaylistFolder[],
): Changeset {
  const titles = folderTitles(folders.filter((f) => !f.isSynthetic));
  const assignments = syncState?.categoryAssignments ?? {};
  const groupOverrides = syncState?.groupTitleOverrides ?? {};
  const folderImages = syncState?.folderImageOverrides ?? {};

  const additions: Addition[] = pendingGroups.map((g) => ({
    group: g,
    targetCategoryTitle: assignments[key(g.feedUrl, g.groupKey)]?.targetCategoryTitle ?? null,
    renamed: !!groupOverrides[key(g.feedUrl, g.groupKey)],
    hasImage: !!folderImages[g.uuid],
    undo: { kind: "deselectGroup", feedUrl: g.feedUrl, guids: g.episodes.map((e) => e.guid) },
  }));

  const additionUuids = new Set(pendingGroups.map((g) => g.uuid));

  const modifications: SimpleChange[] = [];
  for (const d of editDetails) {
    if (d.kind === "renamedFolder" || d.kind === "renamedSound") {
      modifications.push({
        id: `${d.kind}:${d.uuid}`,
        label: `« ${d.title} » → « ${d.newTitle ?? ""} »`,
        detail: d.kind === "renamedFolder" ? "dossier" : "épisode",
        undo: { kind: "cancelEdit", uuid: d.uuid, editType: d.kind },
      });
    } else if (d.kind === "moved") {
      modifications.push({
        id: `move:${d.uuid}`,
        label: d.title || "Élément déplacé",
        detail: `→ ${d.destTitle ?? "…"}`,
        undo: { kind: "cancelEdit", uuid: d.uuid, editType: "moved" },
      });
    }
  }
  for (const uuid of Object.keys(folderImages)) {
    if (additionUuids.has(uuid) || !titles.has(uuid)) continue;
    modifications.push({
      id: `folder-image:${uuid}`,
      label: `Visuel changé — ${titles.get(uuid)}`,
      undo: { kind: "clearFolderImage", uuid },
    });
  }

  const deletions: SimpleChange[] = [];
  for (const record of syncState?.syncedRecords ?? []) {
    if (record.pendingDeletion) {
      deletions.push({
        id: `del-episode:${record.episodeUuid}`,
        label: record.title,
        detail: "épisode",
        undo: { kind: "unmarkDeletion", episodeUuid: record.episodeUuid },
      });
    }
  }
  for (const d of editDetails) {
    if (d.kind === "removed") {
      deletions.push({
        id: `removed:${d.uuid}`,
        label: d.title || "Élément retiré du menu",
        detail: "retiré",
        undo: { kind: "cancelEdit", uuid: d.uuid, editType: "removed" },
      });
    }
  }
  for (const name of pendingOrphanDeletions) {
    deletions.push({
      id: `orphan:${name}`,
      label: name,
      detail: "orphelin",
      undo: { kind: "toggleOrphan", name },
    });
  }

  return {
    additions,
    modifications,
    deletions,
    total: additions.length + modifications.length + deletions.length,
  };
}
