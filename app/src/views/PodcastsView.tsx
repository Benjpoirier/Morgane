import { useState } from "react";
import { motion } from "motion/react";
import { Plus, Search, Loader2, TriangleAlert, Check, Trash2, Image, Play, Pause } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Checkbox } from "@/components/ui/checkbox";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Thumbnail } from "@/components/Thumbnail";
import { ByteBadge } from "@/components/ByteBadge";
import { AddSourceSheet } from "@/components/AddSourceSheet";
import { DeleteConfirmDialog } from "@/components/DeleteConfirmDialog";
import { PlayerBar } from "@/components/PlayerBar";
import { usePodcasts, episodesOf, thumbnailUrl } from "@/store/podcasts";
import { type PlayerTrack } from "@/store/player";
import { useEpisodeRow } from "@/hooks/useEpisodeRow";
import { useFeedDetail } from "@/hooks/useFeedDetail";
import { useSearchResults } from "@/hooks/useSearchResults";
import { MAX_TITLE_BYTES } from "@/lib/text";
import type { Episode, Subscription } from "@/lib/types";
import { cn } from "@/lib/utils";

export function PodcastsView() {
  const [search, setSearch] = useState("");
  const [selectedFeed, setSelectedFeed] = useState<string | null>(null);
  const [showOnlySelected, setShowOnlySelected] = useState(false);
  const [showAdd, setShowAdd] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState<Subscription | null>(null);

  const subscriptions = usePodcasts((s) => s.subscriptions);
  const episodesByFeed = usePodcasts((s) => s.episodesByFeed);
  const loadingFeeds = usePodcasts((s) => s.loadingFeeds);
  const errorsByFeed = usePodcasts((s) => s.errorsByFeed);

  const query = search.trim().toLowerCase();
  const current = subscriptions.find((s) => s.feedUrl === selectedFeed) ?? null;

  return (
    <div className="flex h-full flex-col">
      <div className="flex min-h-0 flex-1">
      {}
      <div className="flex w-72 flex-col border-r">
        <div className="flex items-center gap-2 p-3 pb-2">
          <div className="relative flex-1">
            <Search className="absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Rechercher…"
              className="pl-8"
            />
          </div>
          <Button size="icon" variant="outline" onClick={() => setShowAdd(true)}>
            <Plus />
          </Button>
        </div>
        <div className="flex-1 overflow-y-auto px-2 pb-2">
          {subscriptions.map((sub) => (
            <PodcastRow
              key={sub.feedUrl}
              sub={sub}
              selected={selectedFeed === sub.feedUrl}
              loading={loadingFeeds.has(sub.feedUrl)}
              hasError={sub.feedUrl in errorsByFeed}
              episodeCount={episodesOf(sub, episodesByFeed).length}
              onSelect={() => setSelectedFeed(sub.feedUrl)}
              onDelete={() => setConfirmDelete(sub)}
            />
          ))}
          {subscriptions.length === 0 && (
            <p className="px-2 py-6 text-center text-sm text-muted-foreground">
              Aucun abonnement. Ajoute un flux RSS avec ＋.
            </p>
          )}
        </div>
      </div>

      {}
      <div className="min-w-0 flex-1">
        {query ? (
          <SearchResults query={query} onOpenFeed={(f) => { setSelectedFeed(f); setSearch(""); }} />
        ) : current ? (
          <FeedDetail
            sub={current}
            showOnlySelected={showOnlySelected}
            setShowOnlySelected={setShowOnlySelected}
          />
        ) : (
          <div className="flex h-full items-center justify-center px-8 text-center text-sm text-muted-foreground">
            Sélectionne un podcast à gauche, ou ajoute un flux RSS.
          </div>
        )}
      </div>
      </div>

      <PlayerBar />
      <AddSourceSheet open={showAdd} onOpenChange={setShowAdd} onAdded={setSelectedFeed} />
      <DeleteConfirmDialog
        sub={confirmDelete}
        onClose={() => setConfirmDelete(null)}
        onDeleted={(feedUrl) => {
          if (selectedFeed === feedUrl) setSelectedFeed(null);
        }}
      />
    </div>
  );
}

function PodcastRow({
  sub,
  selected,
  loading,
  hasError,
  episodeCount,
  onSelect,
  onDelete,
}: {
  sub: Subscription;
  selected: boolean;
  loading: boolean;
  hasError: boolean;
  episodeCount: number;
  onSelect: () => void;
  onDelete: () => void;
}) {
  const reloadFeed = usePodcasts((s) => s.reloadFeed);
  const newCount = usePodcasts((s) => (s.newByFeed[sub.feedUrl] ?? []).length);
  const title = sub.title || sub.feedUrl;
  const selectedCount = sub.selectedEpisodeGuids.length;

  const badge = loading ? (
    <Loader2 className="size-3.5 animate-spin text-muted-foreground" />
  ) : hasError ? (
    <TriangleAlert className="size-3.5 text-[var(--warning)]" />
  ) : selectedCount > 0 ? (
    <span className="text-xs text-primary tabular-nums">{selectedCount} sél.</span>
  ) : (
    <span className="text-xs text-muted-foreground tabular-nums">{episodeCount} ép.</span>
  );

  return (
    <DropdownMenu>
      <div
        onClick={onSelect}
        className={cn(
          "group flex cursor-default items-center gap-2.5 rounded-md px-2 py-1.5 transition-colors",
          selected ? "bg-primary/12" : "hover:bg-accent",
        )}
      >
        <Thumbnail src={sub.feedImageUrl} size={36} />
        <div className="min-w-0 flex-1">
          <div className="truncate text-sm font-medium">{title}</div>
        </div>
        {newCount > 0 && (
          <span className="rounded-full bg-[var(--success)] px-1.5 text-[10px] font-semibold text-white tabular-nums">
            {newCount}
          </span>
        )}
        {badge}
        <DropdownMenuTrigger
          onClick={(e) => e.stopPropagation()}
          className="rounded px-1 text-muted-foreground opacity-0 hover:text-foreground group-hover:opacity-100 data-[state=open]:opacity-100"
        >
          ⋯
        </DropdownMenuTrigger>
      </div>
      <DropdownMenuContent align="end">
        <DropdownMenuItem onSelect={() => reloadFeed(sub.feedUrl)}>
          Recharger le flux
        </DropdownMenuItem>
        <DropdownMenuItem variant="destructive" onSelect={onDelete}>
          Supprimer…
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function FeedDetail({
  sub,
  showOnlySelected,
  setShowOnlySelected,
}: {
  sub: Subscription;
  showOnlySelected: boolean;
  setShowOnlySelected: (v: boolean) => void;
}) {
  const { episodes, error, loading, visible, tracks, newGuids, markFeedSeen } = useFeedDetail(
    sub,
    showOnlySelected,
  );

  return (
    <div className="flex h-full flex-col">
      <header className="border-b px-6 py-3">
        <div className="flex items-center gap-3">
          <Thumbnail src={sub.feedImageUrl} size={48} />
          <div className="min-w-0 flex-1">
            <h1 className="truncate text-lg font-semibold">{sub.title || sub.feedUrl}</h1>
            <label className="mt-1 flex items-center gap-2 text-sm text-muted-foreground">
              <Checkbox
                checked={showOnlySelected}
                onCheckedChange={(v) => setShowOnlySelected(v === true)}
              />
              Afficher seulement les sélectionnés
            </label>
          </div>
          {newGuids.length > 0 && (
            <Button
              size="sm"
              variant="outline"
              className="shrink-0"
              onClick={() => markFeedSeen(sub.feedUrl)}
            >
              <Check className="size-4" /> Marquer vus ({newGuids.length})
            </Button>
          )}
        </div>
        {error && (
          <p className="mt-2 text-sm text-destructive">Erreur de chargement : {error}</p>
        )}
      </header>
      <div className="flex-1 overflow-y-auto px-4 py-2">
        {visible.map((ep, i) => (
          <EpisodeRow key={ep.guid} sub={sub} episode={ep} tracks={tracks} index={i} />
        ))}
        {episodes.length === 0 && loading && (
          <div className="flex justify-center py-8">
            <Loader2 className="size-5 animate-spin text-muted-foreground" />
          </div>
        )}
      </div>
    </div>
  );
}

function EpisodeRow({
  sub,
  episode,
  tracks,
  index,
}: {
  sub: Subscription;
  episode: Episode;
  tracks: PlayerTrack[];
  index: number;
}) {
  const {
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
  } = useEpisodeRow(sub, episode);

  return (
    <motion.div
      layout
      className="flex items-center gap-2.5 rounded-md px-2 py-1.5 hover:bg-accent"
    >
      <Checkbox
        checked={selected}
        onCheckedChange={() => toggleSelection(sub.feedUrl, episode.guid)}
      />
      <Thumbnail src={thumbnailUrl(episode, sub)} size={28} />
      <button
        onClick={() => toggleTrack(tracks, index)}
        disabled={!episode.audioUrl}
        className={cn(
          "flex size-7 shrink-0 items-center justify-center rounded-full border transition-colors hover:bg-accent disabled:opacity-40",
          isCurrent ? "border-primary text-primary" : "border-transparent text-muted-foreground",
        )}
        title={playing ? "Pause" : "Écouter"}
      >
        {loadingAudio ? (
          <Loader2 className="size-3.5 animate-spin" />
        ) : playing ? (
          <Pause className="size-3.5" />
        ) : (
          <Play className="size-3.5" />
        )}
      </button>

      <div className="min-w-0 flex-1">
        {title.editing ? (
          <Input defaultValue={displayedTitle} {...title.inputProps} className="h-7" />
        ) : (
          <div
            className="flex items-center gap-1.5"
            onDoubleClick={title.begin}
            title={`Double-clic pour renommer (${MAX_TITLE_BYTES} octets max)`}
          >
            <span className="truncate text-sm">{displayedTitle}</span>
            {isNew && (
              <span className="shrink-0 rounded bg-[var(--success)]/15 px-1 py-0.5 text-[9px] font-bold uppercase leading-none text-[var(--success)]">
                New
              </span>
            )}
            <ByteBadge value={displayedTitle} />
          </div>
        )}
        {episode.publishedAt && (
          <div className="text-xs text-muted-foreground tabular-nums">
            {new Date(episode.publishedAt).toLocaleDateString("fr-FR")}
          </div>
        )}
      </div>

      {}
      {meta?.guessedNumber != null ? (
        <span className="text-xs text-muted-foreground tabular-nums">
          #{meta.guessedNumber}
        </span>
      ) : numberEdit.editing ? (
        <Input
          defaultValue={numberOverride?.toString() ?? ""}
          {...numberEdit.inputProps}
          className="h-7 w-12 text-center"
        />
      ) : (
        <button
          onClick={numberEdit.begin}
          className="text-xs text-muted-foreground hover:text-foreground"
          title="Numéro d'épisode manuel"
        >
          {numberOverride != null ? `#${numberOverride}` : "#…"}
        </button>
      )}

      {}
      {image.editing ? (
        <Input
          autoFocus
          value={image.draft ?? ""}
          onChange={(e) => image.setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") void image.commitDraft();
            if (e.key === "Escape") image.cancel();
          }}
          onBlur={() => void image.commitDraft()}
          placeholder="URL ou /chemin.jpg"
          className="h-7 w-48"
        />
      ) : (
        <button
          onClick={() => image.begin(syncState?.episodeImageOverrides[episode.guid] ?? "")}
          className={cn(
            "rounded p-1 hover:bg-muted",
            hasImageOverride ? "text-primary" : "text-muted-foreground",
          )}
          title="Image personnalisée (URL ou fichier local)"
        >
          <Image className="size-3.5" />
        </button>
      )}

      {}
      {record?.pendingDeletion ? (
        <Trash2 className="size-4 text-[var(--warning)]" />
      ) : record ? (
        <Check className="size-4 text-[var(--success)]" />
      ) : null}
    </motion.div>
  );
}

function SearchResults({
  query,
  onOpenFeed,
}: {
  query: string;
  onOpenFeed: (feedUrl: string) => void;
}) {
  const { podcastMatches, episodeMatches, tracks } = useSearchResults(query);

  return (
    <div className="flex h-full flex-col">
      <header className="border-b px-6 py-3">
        <h1 className="text-lg font-semibold">Résultats pour « {query} »</h1>
      </header>
      <div className="flex-1 overflow-y-auto px-4 py-2">
        <div className="px-2 py-1 text-xs font-semibold text-muted-foreground uppercase">
          Podcasts
        </div>
        {podcastMatches.map((sub) => (
          <button
            key={sub.feedUrl}
            onClick={() => onOpenFeed(sub.feedUrl)}
            className="flex w-full items-center gap-2.5 rounded-md px-2 py-1.5 text-left hover:bg-accent"
          >
            <Thumbnail src={sub.feedImageUrl} size={28} />
            <span className="truncate text-sm">{sub.title || sub.feedUrl}</span>
          </button>
        ))}
        <div className="mt-3 px-2 py-1 text-xs font-semibold text-muted-foreground uppercase">
          Épisodes
        </div>
        {episodeMatches.map(({ sub, episode }, i) => (
          <EpisodeRow key={`${sub.feedUrl}:${episode.guid}`} sub={sub} episode={episode} tracks={tracks} index={i} />
        ))}
      </div>
    </div>
  );
}
