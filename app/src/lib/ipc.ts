import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  ConnectionStatus,
  FfmpegProgressPayload,
  IntegrityProgressPayload,
  Issue,
  ManualCategory,
  PendingGroup,
  Podcast,
  PodcastCategoryAssignment,
  SelectedPair,
  StepPayload,
  Subscription,
  SyncedRecord,
  SyncProgressPhase,
  ThumbnailReadyPayload,
  TreeEdit,
} from "./types";

export const testConnection = (host: string, port: number, manual: boolean) =>
  invoke<ConnectionStatus>("test_connection", { host, port, manual });

export const checkInternet = () => invoke<boolean>("check_internet");

export const listRegisteredDevices = () =>
  invoke<import("./types").RegisteredDevice[]>("list_registered_devices");
export const setActiveDevice = (mac: string) =>
  invoke<void>("set_active_device", { mac });
export const renameRegisteredDevice = (mac: string, name: string) =>
  invoke<void>("rename_registered_device", { mac, name });
export const removeRegisteredDevice = (mac: string) =>
  invoke<void>("remove_registered_device", { mac });

export const listSubscriptions = () =>
  invoke<Subscription[]>("list_subscriptions");
export const addRss = (url: string) => invoke<void>("add_rss", { url });
export const addDirect = (
  title: string,
  audioUrl: string,
  imageUrl: string | null,
) => invoke<void>("add_direct", { title, audioUrl, imageUrl });
export const deleteSubscription = (feedUrl: string) =>
  invoke<void>("delete_subscription", { feedUrl });
export const setSelectedGuids = (feedUrl: string, guids: string[]) =>
  invoke<void>("set_selected_guids", { feedUrl, guids });
export const loadFeed = (feedUrl: string) =>
  invoke<Podcast>("load_feed", { feedUrl });
export const searchPodcasts = (query: string) =>
  invoke<import("./types").PodcastSearchResult[]>("search_podcasts", { query });
export const curatedPodcasts = () =>
  invoke<import("./types").PodcastSearchResult[]>("curated_podcasts");
export const popularKidsPodcasts = () =>
  invoke<import("./types").PodcastSearchResult[]>("popular_kids_podcasts");
export const newEpisodes = (feedUrl: string, guids: string[]) =>
  invoke<string[]>("new_episodes", { feedUrl, guids });
export const markFeedSeen = (feedUrl: string, guids: string[]) =>
  invoke<void>("mark_feed_seen", { feedUrl, guids });

export interface SyncStateSnapshot {
  syncedRecords: SyncedRecord[];
  episodeTitleOverrides: Record<string, string>;
  episodeNumberOverrides: Record<string, number>;
  groupTitleOverrides: Record<string, string>;
  categoryAssignments: Record<string, PodcastCategoryAssignment>;
  folderImageOverrides: Record<string, string>;
  episodeImageOverrides: Record<string, string>;
  manualCategories: ManualCategory[];
}

export const getSyncState = () =>
  invoke<SyncStateSnapshot>("get_sync_state");
export const markPendingDeletion = (episodeUuid: string, pending: boolean) =>
  invoke<void>("mark_pending_deletion", { episodeUuid, pending });
export const setEpisodeTitleOverride = (guid: string, title: string | null) =>
  invoke<void>("set_episode_title_override", { guid, title });
export const setEpisodeNumberOverride = (guid: string, number: number | null) =>
  invoke<void>("set_episode_number_override", { guid, number });
export const setEpisodeImageOverride = (guid: string, source: string | null) =>
  invoke<void>("set_episode_image_override", { guid, source });
export const setGroupTitleOverride = (
  feedUrl: string,
  groupKey: string,
  title: string | null,
) => invoke<void>("set_group_title_override", { feedUrl, groupKey, title });
export const setCategoryAssignment = (assignment: PodcastCategoryAssignment) =>
  invoke<void>("set_category_assignment", { assignment });
export const setFolderImageOverride = (folderUuid: string, source: string | null) =>
  invoke<void>("set_folder_image_override", { folderUuid, source });

export const computePendingGroups = (
  pairs: SelectedPair[],
  alreadySynced: string[],
) => invoke<PendingGroup[]>("compute_pending_groups", { pairs, alreadySynced });
export const guessNumbers = (titles: string[]) =>
  invoke<(number | null)[]>("guess_numbers", { titles });
export const episodeUuids = (guids: string[]) =>
  invoke<string[]>("episode_uuids", { guids });

export interface TreeView {
  folders: import("./types").PlaylistFolder[];
  pendingEdits: TreeEdit[];
  editDetails: import("./types").EditDetail[];
  pendingOrphanDeletions: string[];
  thumbnailUuids: string[];
}

export const refreshTree = (host: string, port: number) =>
  invoke<TreeView>("refresh_tree", { host, port });
export const renameFolder = (uuid: string, newTitle: string) =>
  invoke<TreeView>("rename_folder", { uuid, newTitle });
export const renameSound = (uuid: string, newTitle: string) =>
  invoke<TreeView>("rename_sound", { uuid, newTitle });
export const moveNode = (uuid: string, destinationUuid: string) =>
  invoke<TreeView>("move_node", { uuid, destinationUuid });
export const deleteFolder = (uuid: string) =>
  invoke<TreeView>("delete_folder", { uuid });
export const cancelTreeEdit = (uuid: string, kind: TreeEdit["type"]) =>
  invoke<TreeView>("cancel_tree_edit", { uuid, kind });
export const addManualCategory = (title: string, imageSource: string) =>
  invoke<TreeView>("add_manual_category", { title, imageSource });
export const removeManualCategory = (uuid: string) =>
  invoke<TreeView>("remove_manual_category", { uuid });
export const toggleOrphan = (uuid: string) =>
  invoke<TreeView>("toggle_orphan", { uuid });
export const toggleAllOrphans = () => invoke<TreeView>("toggle_all_orphans");
export const clearPendingEdits = () => invoke<TreeView>("clear_pending_edits");
export const clearPendingOrphanDeletions = () =>
  invoke<TreeView>("clear_pending_orphan_deletions");
export const searchOrphans = (host: string, port: number) =>
  invoke<TreeView>("search_orphans", { host, port });
export const checkIntegrity = (host: string, port: number) =>
  invoke<Issue[]>("check_integrity", { host, port });
export const repairIntegrity = (
  host: string,
  port: number,
  fixes: { missing: import("./types").MissingFile; path: string }[],
) => invoke<void>("repair_integrity", { host, port, fixes });
export const downloadThumbnails = (host: string, port: number, uuids: string[]) =>
  invoke<void>("download_thumbnails", { host, port, uuids });

export interface SyncLaunch {
  pairs: SelectedPair[];
  host: string;
  port: number;
  alreadySynced: string[];
  filesToDelete: Record<string, string[]>;
  treeEdits: TreeEdit[];
}

export const startSync = (launch: SyncLaunch) =>
  invoke<void>("start_sync", { launch });
export const cancelSync = () => invoke<void>("cancel_sync");

export const prepareSelection = (pairs: SelectedPair[]) =>
  invoke<void>("prepare_selection", { pairs });

export const preparedGuids = (guids: string[]) =>
  invoke<string[]>("prepared_guids", { guids });

export const onIntegrityProgress = (
  cb: (p: IntegrityProgressPayload) => void,
): Promise<UnlistenFn> =>
  listen<IntegrityProgressPayload>("integrity://progress", (e) => cb(e.payload));
export const onSyncPhase = (cb: (p: SyncProgressPhase) => void): Promise<UnlistenFn> =>
  listen<SyncProgressPhase>("sync://phase", (e) => cb(e.payload));
export const onSyncLog = (cb: (line: string) => void): Promise<UnlistenFn> =>
  listen<string>("sync://log", (e) => cb(e.payload));
export const onSyncStep = (cb: (s: StepPayload) => void): Promise<UnlistenFn> =>
  listen<StepPayload>("sync://step", (e) => cb(e.payload));
export const onEpisodeUploaded = (cb: (uuid: string) => void): Promise<UnlistenFn> =>
  listen<string>("sync://episode-uploaded", (e) => cb(e.payload));
export const onDeletionsCompleted = (cb: (uuids: string[]) => void): Promise<UnlistenFn> =>
  listen<string[]>("sync://deletions-completed", (e) => cb(e.payload));
export const onTreeEditsApplied = (cb: () => void): Promise<UnlistenFn> =>
  listen("sync://tree-edits-applied", () => cb());
export const onSyncEnded = (cb: () => void): Promise<UnlistenFn> =>
  listen("sync://ended", () => cb());
export const onPrepareEpisodeReady = (cb: (guid: string) => void): Promise<UnlistenFn> =>
  listen<string>("prepare://episode-ready", (e) => cb(e.payload));
export const onPrepareEpisodeFailed = (
  cb: (p: { guid: string; error: string }) => void,
): Promise<UnlistenFn> =>
  listen<{ guid: string; error: string }>("prepare://episode-failed", (e) => cb(e.payload));
export const onPrepareProgress = (
  cb: (p: { guid: string; fraction: number }) => void,
): Promise<UnlistenFn> =>
  listen<{ guid: string; fraction: number }>("prepare://progress", (e) => cb(e.payload));
export const onPrepareEnded = (cb: () => void): Promise<UnlistenFn> =>
  listen("prepare://ended", () => cb());
export const onThumbnailReady = (
  cb: (p: ThumbnailReadyPayload) => void,
): Promise<UnlistenFn> =>
  listen<ThumbnailReadyPayload>("thumbnail://ready", (e) => cb(e.payload));

export const ffmpegReady = () => invoke<boolean>("ffmpeg_ready");
export const downloadFfmpeg = () => invoke<void>("download_ffmpeg");
export const onFfmpegProgress = (
  cb: (p: FfmpegProgressPayload) => void,
): Promise<UnlistenFn> =>
  listen<FfmpegProgressPayload>("ffmpeg://progress", (e) => cb(e.payload));
