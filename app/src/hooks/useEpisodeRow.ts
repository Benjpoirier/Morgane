import { usePodcasts } from "@/store/podcasts";
import { usePlayer } from "@/store/player";
import { useInlineEdit } from "@/hooks/useInlineEdit";
import { useImageOverride } from "@/hooks/useImageOverride";
import {
  setEpisodeImageOverride,
  setEpisodeNumberOverride,
  setEpisodeTitleOverride,
} from "@/lib/ipc";
import type { Episode, Subscription } from "@/lib/types";

export function useEpisodeRow(sub: Subscription, episode: Episode) {
  const toggleSelection = usePodcasts((s) => s.toggleSelection);
  const reloadSyncState = usePodcasts((s) => s.reloadSyncState);
  const syncState = usePodcasts((s) => s.syncState);
  const meta = usePodcasts((s) => s.episodeMeta[episode.guid]);

  const isCurrent = usePlayer((s) => s.current?.guid === episode.guid);
  const playing = usePlayer((s) => s.playing && s.current?.guid === episode.guid);
  const loadingAudio = usePlayer((s) => s.loading && s.current?.guid === episode.guid);
  const toggleTrack = usePlayer((s) => s.toggleTrack);
  const isNew = usePodcasts((s) => (s.newByFeed[sub.feedUrl] ?? []).includes(episode.guid));

  const selected = usePodcasts(
    (s) =>
      s.subscriptions
        .find((x) => x.feedUrl === sub.feedUrl)
        ?.selectedEpisodeGuids.includes(episode.guid) ?? false,
  );

  const displayedTitle = syncState?.episodeTitleOverrides[episode.guid] ?? episode.title;
  const numberOverride = syncState?.episodeNumberOverrides[episode.guid];
  const hasImageOverride = !!syncState?.episodeImageOverrides[episode.guid];
  const record = syncState?.syncedRecords.find((r) => r.episodeUuid === meta?.uuid);

  const title = useInlineEdit(async (value) => {
    const v = value.trim();
    if (v && v !== episode.title) {
      await setEpisodeTitleOverride(episode.guid, v);
      await reloadSyncState();
    }
  });
  const numberEdit = useInlineEdit(async (value) => {
    const parsed = parseInt(value.trim(), 10);
    await setEpisodeNumberOverride(episode.guid, Number.isFinite(parsed) ? parsed : null);
    await reloadSyncState();
  });
  const image = useImageOverride(async (source) => {
    await setEpisodeImageOverride(episode.guid, source);
    await reloadSyncState();
  });

  return {
    toggleSelection,
    syncState,
    meta,
    isCurrent,
    playing,
    loadingAudio,
    toggleTrack,
    isNew,
    selected,
    displayedTitle,
    numberOverride,
    hasImageOverride,
    record,
    title,
    numberEdit,
    image,
  };
}
