import { usePodcasts, episodesOf, thumbnailUrl } from "@/store/podcasts";
import { type PlayerTrack } from "@/store/player";
import type { Subscription } from "@/lib/types";

export function useFeedDetail(sub: Subscription, showOnlySelected: boolean) {
  const episodesByFeed = usePodcasts((s) => s.episodesByFeed);
  const errorsByFeed = usePodcasts((s) => s.errorsByFeed);
  const loadingFeeds = usePodcasts((s) => s.loadingFeeds);
  const newGuids = usePodcasts((s) => s.newByFeed[sub.feedUrl] ?? []);
  const markFeedSeen = usePodcasts((s) => s.markFeedSeen);
  const isSelected = (guid: string) => sub.selectedEpisodeGuids.includes(guid);

  const episodes = episodesOf(sub, episodesByFeed);
  const error = errorsByFeed[sub.feedUrl];
  const loading = loadingFeeds.has(sub.feedUrl);

  const visible = episodes.filter((e) => !showOnlySelected || isSelected(e.guid));
  const tracks: PlayerTrack[] = visible.map((e) => ({
    guid: e.guid,
    title: e.title,
    audioUrl: e.audioUrl,
    imageUrl: thumbnailUrl(e, sub),
  }));

  return { episodes, error, loading, visible, tracks, newGuids, markFeedSeen };
}
