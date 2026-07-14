import { create } from "zustand";
import { convertFileSrc } from "@tauri-apps/api/core";
import {
  addManualCategory as addManualCategoryCmd,
  checkIntegrity as checkIntegrityCmd,
  clearPendingEdits as clearPendingEditsCmd,
  clearPendingOrphanDeletions as clearPendingOrphanDeletionsCmd,
  cancelTreeEdit as cancelTreeEditCmd,
  deleteFolder as deleteFolderCmd,
  downloadThumbnails,
  moveNode as moveNodeCmd,
  refreshTree,
  removeManualCategory as removeManualCategoryCmd,
  renameFolder as renameFolderCmd,
  renameSound as renameSoundCmd,
  searchOrphans as searchOrphansCmd,
  toggleAllOrphans as toggleAllOrphansCmd,
  toggleOrphan as toggleOrphanCmd,
  type TreeView,
} from "@/lib/ipc";
import type { EditDetail, Issue, PlaylistFolder, PlaylistNode, TreeEdit } from "@/lib/types";

export function deviceSoundUuids(folders: PlaylistFolder[]): string[] {
  const out: string[] = [];
  const walk = (nodes: PlaylistNode[]) => {
    for (const node of nodes) {
      if (node.kind === "sound") out.push(node.uuid);
      else walk(node.children);
    }
  };
  folders.forEach((folder) => walk(folder.children));
  return out;
}

export function alreadyOnDevice(
  syncedRecords: { episodeUuid: string }[] | undefined,
  folders: PlaylistFolder[],
): string[] {
  return Array.from(
    new Set([...(syncedRecords?.map((r) => r.episodeUuid) ?? []), ...deviceSoundUuids(folders)]),
  );
}

interface TreeState {
  folders: PlaylistFolder[];
  pendingEdits: TreeEdit[];
  editDetails: EditDetail[];
  pendingOrphanDeletions: string[];
  thumbnailUuids: string[];

  thumbnails: Record<string, string>;
  loading: boolean;
  loadError: string | null;
  hasLoadedOnce: boolean;
  searchingOrphans: boolean;

  integrityIssues: Issue[] | null;
  checkingIntegrity: boolean;
  integrityError: string | null;

  apply: (view: TreeView) => void;

  reset: () => void;
  setThumbnail: (uuid: string, path: string) => void;
  refresh: (host: string, port: number) => Promise<void>;
  renameFolder: (uuid: string, title: string) => Promise<void>;
  renameSound: (uuid: string, title: string) => Promise<void>;
  moveNode: (uuid: string, destinationUuid: string) => Promise<void>;
  deleteFolder: (uuid: string) => Promise<void>;
  cancelEdit: (uuid: string, kind: TreeEdit["type"]) => Promise<void>;
  addCategory: (title: string, imageSource: string) => Promise<void>;
  removeCategory: (uuid: string) => Promise<void>;
  searchOrphans: (host: string, port: number) => Promise<void>;
  toggleOrphan: (uuid: string) => Promise<void>;
  toggleAllOrphans: () => Promise<void>;
  clearPendingEdits: () => Promise<void>;
  clearPendingOrphanDeletions: () => Promise<void>;
  checkIntegrity: (host: string, port: number) => Promise<void>;
}

export const useTree = create<TreeState>((set, get) => ({
  folders: [],
  pendingEdits: [],
  editDetails: [],
  pendingOrphanDeletions: [],
  thumbnailUuids: [],
  thumbnails: {},
  loading: false,
  loadError: null,
  hasLoadedOnce: false,
  searchingOrphans: false,
  integrityIssues: null,
  checkingIntegrity: false,
  integrityError: null,

  apply: (view) =>
    set({
      folders: view.folders,
      pendingEdits: view.pendingEdits,
      editDetails: view.editDetails,
      pendingOrphanDeletions: view.pendingOrphanDeletions,
      thumbnailUuids: view.thumbnailUuids,
    }),

  reset: () =>
    set({
      folders: [],
      pendingEdits: [],
      editDetails: [],
      pendingOrphanDeletions: [],
      thumbnailUuids: [],
      thumbnails: {},
      hasLoadedOnce: false,
      loadError: null,
      integrityIssues: null,
    }),

  setThumbnail: (uuid, path) =>
    set((s) => ({ thumbnails: { ...s.thumbnails, [uuid]: convertFileSrc(path) } })),

  refresh: async (host, port) => {
    set({ loading: true, loadError: null });
    try {
      const view = await refreshTree(host, port);
      get().apply(view);
      set({ hasLoadedOnce: true });

      if (view.thumbnailUuids.length > 0) {
        void downloadThumbnails(host, port, view.thumbnailUuids);
      }
    } catch (e) {
      set({ loadError: String(e) });
    } finally {
      set({ loading: false });
    }
  },

  renameFolder: async (uuid, title) => get().apply(await renameFolderCmd(uuid, title)),
  renameSound: async (uuid, title) => get().apply(await renameSoundCmd(uuid, title)),
  moveNode: async (uuid, dst) => get().apply(await moveNodeCmd(uuid, dst)),
  deleteFolder: async (uuid) => get().apply(await deleteFolderCmd(uuid)),
  cancelEdit: async (uuid, kind) => get().apply(await cancelTreeEditCmd(uuid, kind)),
  addCategory: async (title, image) => get().apply(await addManualCategoryCmd(title, image)),
  removeCategory: async (uuid) => get().apply(await removeManualCategoryCmd(uuid)),

  searchOrphans: async (host, port) => {
    set({ searchingOrphans: true });
    try {
      get().apply(await searchOrphansCmd(host, port));
    } finally {
      set({ searchingOrphans: false });
    }
  },
  toggleOrphan: async (uuid) => get().apply(await toggleOrphanCmd(uuid)),
  toggleAllOrphans: async () => get().apply(await toggleAllOrphansCmd()),
  clearPendingEdits: async () => get().apply(await clearPendingEditsCmd()),
  clearPendingOrphanDeletions: async () =>
    get().apply(await clearPendingOrphanDeletionsCmd()),

  checkIntegrity: async (host, port) => {
    set({ checkingIntegrity: true, integrityError: null });
    try {
      set({ integrityIssues: await checkIntegrityCmd(host, port) });
    } catch (e) {
      set({ integrityError: String(e) });
    } finally {
      set({ checkingIntegrity: false });
    }
  },
}));
