import { create } from "zustand";
import {
  episodeUuids,
  getSyncState,
  guessNumbers,
  listSubscriptions,
  loadFeed,
  markFeedSeen as markFeedSeenApi,
  newEpisodes,
  setSelectedGuids,
  type SyncStateSnapshot,
} from "@/lib/ipc";
import type { Episode, SelectedPair, Subscription } from "@/lib/types";

export interface EpisodeMeta {
  uuid: string;
  guessedNumber: number | null;
}

interface PodcastsState {
  subscriptions: Subscription[];
  episodesByFeed: Record<string, Episode[]>;
  episodeMeta: Record<string, EpisodeMeta>;
  loadingFeeds: Set<string>;
  errorsByFeed: Record<string, string>;
  syncState: SyncStateSnapshot | null;

  newByFeed: Record<string, string[]>;

  reloadSubscriptions: () => Promise<void>;
  reloadSyncState: () => Promise<void>;
  loadAllFeeds: () => Promise<void>;
  reloadFeed: (feedUrl: string) => Promise<void>;
  toggleSelection: (feedUrl: string, guid: string) => Promise<void>;
  deselectEpisodes: (feedUrl: string, guids: string[]) => Promise<void>;
  markFeedSeen: (feedUrl: string) => Promise<void>;
}

export function episodesOf(
  sub: Subscription,
  episodesByFeed: Record<string, Episode[]>,
): Episode[] {
  if (sub.kind === "direct") {
    if (!sub.directAudioUrl) return [];
    return [
      {
        guid: sub.feedUrl,
        title: sub.directTitle ?? sub.title,
        audioUrl: sub.directAudioUrl,
        imageUrl: sub.directImageUrl,
        publishedAt: null,
        duration: null,
      },
    ];
  }
  return episodesByFeed[sub.feedUrl] ?? [];
}

export function collectSelectedPairs(
  subscriptions: Subscription[],
  episodesByFeed: Record<string, Episode[]>,
): SelectedPair[] {
  const out: SelectedPair[] = [];
  for (const sub of subscriptions) {
    for (const ep of episodesOf(sub, episodesByFeed)) {
      if (sub.selectedEpisodeGuids.includes(ep.guid)) out.push({ subscription: sub, episode: ep });
    }
  }
  return out;
}

export function thumbnailUrl(ep: Episode, sub: Subscription): string | null {
  return ep.imageUrl || sub.feedImageUrl || null;
}

async function refreshNew(
  feedUrl: string,
  episodes: Episode[],
  set: (fn: (s: PodcastsState) => Partial<PodcastsState>) => void,
) {
  try {
    const fresh = await newEpisodes(feedUrl, episodes.map((e) => e.guid));
    set((s) => ({ newByFeed: { ...s.newByFeed, [feedUrl]: fresh } }));
  } catch {

  }
}

async function fillMeta(
  episodes: Episode[],
  set: (fn: (s: PodcastsState) => Partial<PodcastsState>) => void,
) {
  if (episodes.length === 0) return;
  const guids = episodes.map((e) => e.guid);
  const titles = episodes.map((e) => e.title);
  const [uuids, numbers] = await Promise.all([
    episodeUuids(guids),
    guessNumbers(titles),
  ]);
  set((s) => {
    const meta = { ...s.episodeMeta };
    guids.forEach((g, i) => {
      meta[g] = { uuid: uuids[i], guessedNumber: numbers[i] };
    });
    return { episodeMeta: meta };
  });
}

export const usePodcasts = create<PodcastsState>((set, get) => ({
  subscriptions: [],
  episodesByFeed: {},
  episodeMeta: {},
  loadingFeeds: new Set(),
  errorsByFeed: {},
  syncState: null,
  newByFeed: {},

  reloadSubscriptions: async () => {

    const subs = await listSubscriptions();
    subs.sort((a, b) =>
      (a.title || a.feedUrl).localeCompare(b.title || b.feedUrl, "fr", {
        sensitivity: "base",
      }),
    );
    set({ subscriptions: subs });
  },

  reloadSyncState: async () => {
    set({ syncState: await getSyncState() });
  },

  loadAllFeeds: async () => {
    const { subscriptions, episodesByFeed, loadingFeeds } = get();
    const toLoad = subscriptions.filter(
      (s) =>
        s.kind === "rss" &&
        !(s.feedUrl in episodesByFeed) &&
        !loadingFeeds.has(s.feedUrl),
    );
    if (toLoad.length === 0) return;
    set((s) => {
      const loading = new Set(s.loadingFeeds);
      const errors = { ...s.errorsByFeed };
      toLoad.forEach((sub) => {
        loading.add(sub.feedUrl);
        delete errors[sub.feedUrl];
      });
      return { loadingFeeds: loading, errorsByFeed: errors };
    });
    await Promise.all(
      toLoad.map(async (sub) => {
        try {
          const podcast = await loadFeed(sub.feedUrl);
          set((s) => ({
            episodesByFeed: { ...s.episodesByFeed, [sub.feedUrl]: podcast.episodes },
          }));
          await fillMeta(podcast.episodes, set);
          await refreshNew(sub.feedUrl, podcast.episodes, set);
        } catch (e) {
          set((s) => ({
            errorsByFeed: { ...s.errorsByFeed, [sub.feedUrl]: String(e) },
          }));
        } finally {
          set((s) => {
            const loading = new Set(s.loadingFeeds);
            loading.delete(sub.feedUrl);
            return { loadingFeeds: loading };
          });
        }
      }),
    );

    await get().reloadSubscriptions();
  },

  reloadFeed: async (feedUrl) => {
    set((s) => {
      const loading = new Set(s.loadingFeeds);
      loading.add(feedUrl);
      const episodesByFeed = { ...s.episodesByFeed };
      delete episodesByFeed[feedUrl];
      const errors = { ...s.errorsByFeed };
      delete errors[feedUrl];
      return { loadingFeeds: loading, episodesByFeed, errorsByFeed: errors };
    });
    try {
      const podcast = await loadFeed(feedUrl);
      set((s) => ({
        episodesByFeed: { ...s.episodesByFeed, [feedUrl]: podcast.episodes },
      }));
      await fillMeta(podcast.episodes, set);
      await refreshNew(feedUrl, podcast.episodes, set);
    } catch (e) {
      set((s) => ({ errorsByFeed: { ...s.errorsByFeed, [feedUrl]: String(e) } }));
    } finally {
      set((s) => {
        const loading = new Set(s.loadingFeeds);
        loading.delete(feedUrl);
        return { loadingFeeds: loading };
      });
      await get().reloadSubscriptions();
    }
  },

  toggleSelection: async (feedUrl, guid) => {
    const sub = get().subscriptions.find((s) => s.feedUrl === feedUrl);
    if (!sub) return;
    const guids = sub.selectedEpisodeGuids.includes(guid)
      ? sub.selectedEpisodeGuids.filter((g) => g !== guid)
      : [...sub.selectedEpisodeGuids, guid];
    await setSelectedGuids(feedUrl, guids);
    await get().reloadSubscriptions();
  },

  deselectEpisodes: async (feedUrl, guids) => {
    const sub = get().subscriptions.find((s) => s.feedUrl === feedUrl);
    if (!sub) return;
    const drop = new Set(guids);
    await setSelectedGuids(feedUrl, sub.selectedEpisodeGuids.filter((g) => !drop.has(g)));
    await get().reloadSubscriptions();
  },

  markFeedSeen: async (feedUrl) => {
    const guids = get().newByFeed[feedUrl] ?? [];
    if (guids.length === 0) return;
    await markFeedSeenApi(feedUrl, guids);
    set((s) => ({ newByFeed: { ...s.newByFeed, [feedUrl]: [] } }));
  },
}));
