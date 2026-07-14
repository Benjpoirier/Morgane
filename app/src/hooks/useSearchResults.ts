import { useMemo } from "react";
import { usePodcasts, episodesOf, thumbnailUrl } from "@/store/podcasts";
import { type PlayerTrack } from "@/store/player";
import type { Episode, Subscription } from "@/lib/types";

export function useSearchResults(query: string) {
  const subscriptions = usePodcasts((s) => s.subscriptions);
  const episodesByFeed = usePodcasts((s) => s.episodesByFeed);

  const podcastMatches = useMemo(
    () => subscriptions.filter((s) => (s.title || s.feedUrl).toLowerCase().includes(query)),
    [subscriptions, query],
  );
  const episodeMatches = useMemo(() => {
    const out: { sub: Subscription; episode: Episode }[] = [];
    for (const sub of subscriptions) {
      for (const ep of episodesOf(sub, episodesByFeed)) {
        if (ep.title.toLowerCase().includes(query)) out.push({ sub, episode: ep });
      }
    }
    return out.slice(0, 100);
  }, [subscriptions, episodesByFeed, query]);

  const tracks: PlayerTrack[] = episodeMatches.map(({ sub, episode }) => ({
    guid: episode.guid,
    title: episode.title,
    audioUrl: episode.audioUrl,
    imageUrl: thumbnailUrl(episode, sub),
  }));

  return { podcastMatches, episodeMatches, tracks };
}
