export interface Subscription {
  feedUrl: string;
  title: string;
  kind: string;
  category: string;
  selectedEpisodeGuids: string[];
  feedImageUrl: string | null;
  directAudioUrl: string | null;
  directTitle: string | null;
  directImageUrl: string | null;
}

export interface Episode {
  guid: string;
  title: string;
  audioUrl: string;
  imageUrl: string | null;
  publishedAt: string | null;
  duration: string | null;
}

export interface Podcast {
  feedUrl: string;
  title: string;
  imageUrl: string | null;
  episodes: Episode[];
}

export type PlaylistNode =
  | ({ kind: "folder" } & PlaylistFolder)
  | { kind: "sound"; uuid: string; title: string };

export interface PlaylistFolder {
  uuid: string;
  title: string;
  children: PlaylistNode[];
  isFavorite: boolean;
  isSynthetic: boolean;
}

export interface SyncedRecord {
  episodeUuid: string;
  title: string;
  folderTitle: string;
  syncedAt: string;
  pendingDeletion: boolean;
}

export interface ManualCategory {
  uuid: string;
  title: string;
  imageSource: string;
}

export interface PodcastCategoryAssignment {
  feedUrl: string;
  groupKey: string;
  targetCategoryUuid: string;
  targetCategoryTitle: string;
}

export type MissingFile =
  | { type: "image"; remoteName: string }
  | { type: "audio"; baseUuid: string };

export type IssueKind = "folder" | "sound";

export interface Issue {
  uuid: string;
  title: string;
  kind: IssueKind;
  missingFiles: MissingFile[];
}

export type TreeEdit =
  | { type: "renamedFolder"; uuid: string; newTitle: string }
  | { type: "renamedSound"; uuid: string; newTitle: string }
  | { type: "moved"; uuid: string; toParentUuid: string }
  | { type: "removed"; uuid: string };

export interface EditDetail {
  uuid: string;
  kind: TreeEdit["type"];
  title: string;
  newTitle: string | null;
  destTitle: string | null;
}

export interface DeviceFile {
  name: string;
  size: number;
}

export interface RegisteredDevice {
  mac: string;
  name: string;
  isActive: boolean;
  lastConnectedAt: number | null;
}

export interface PodcastSearchResult {
  title: string;
  feedUrl: string;
  imageUrl: string | null;
  episodeCount: number | null;
  genre: string | null;
  author: string | null;
}

export type SyncProgressPhase =
  | { type: "preparing"; data: { done: number; total: number } }
  | { type: "connecting" }
  | { type: "sending"; data: { bytesDone: number; bytesTotal: number } }
  | { type: "finished"; data: { count: number } }
  | { type: "failed"; data: string };

export interface ConnectionStatus {
  connected: boolean;
  latencyMs: number | null;
  message: string | null;
  busy: boolean;
  deviceMac: string | null;

  deviceName: string | null;

  newlyRegistered: boolean;
}

export interface SelectedPair {
  subscription: Subscription;
  episode: Episode;
}

export interface PendingGroup {
  feedUrl: string;
  groupKey: string;
  uuid: string;
  title: string;
  feedImageUrl: string | null;
  episodes: Episode[];
}

export interface StepPayload {
  label: string | null;
  fraction: number;
}

export interface ThumbnailReadyPayload {
  uuid: string;
  path: string;
}

export interface IntegrityProgressPayload {
  done: number;
  total: number;
}

export interface FfmpegProgressPayload {
  phase: "downloading" | "verifying" | "extracting" | "done";
  bytes: number;
  totalBytes: number;
}
