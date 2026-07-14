import { useEffect, useState } from "react";
import { Search, Loader2, Check, Plus, FolderOpen, TriangleAlert } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Thumbnail } from "@/components/Thumbnail";
import { addDirect, addRss, curatedPodcasts, popularKidsPodcasts, searchPodcasts } from "@/lib/ipc";
import { pickAudio } from "@/lib/pickAudio";
import { pickImage } from "@/lib/pickImage";
import { usePodcasts } from "@/store/podcasts";
import { useConnection } from "@/store/connection";
import type { PodcastSearchResult } from "@/lib/types";

function baseName(path: string): string {
  return path.split(/[/\\]/).pop()?.replace(/\.[^.]+$/, "") ?? "";
}

function isKidsGenre(result: PodcastSearchResult): boolean {
  const genre = (result.genre ?? "").toLowerCase();
  return genre.includes("enfant") || genre.includes("famille") || genre.includes("kids");
}

export function AddSourceSheet({
  open,
  onOpenChange,
  onAdded,
}: {
  open: boolean;
  onOpenChange: (v: boolean) => void;
  onAdded: (feedUrl: string) => void;
}) {
  const reloadSubscriptions = usePodcasts((s) => s.reloadSubscriptions);
  const loadAllFeeds = usePodcasts((s) => s.loadAllFeeds);

  const networkContext = useConnection((s) => s.networkContext);
  const canSearch = networkContext !== "merlin" && networkContext !== "offline";

  const [query, setQuery] = useState("");
  const [results, setResults] = useState<PodcastSearchResult[]>([]);
  const [searched, setSearched] = useState(false);
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [added, setAdded] = useState<Set<string>>(new Set());
  const [suggestions, setSuggestions] = useState<PodcastSearchResult[]>([]);
  const [popular, setPopular] = useState<PodcastSearchResult[]>([]);
  const [kidsOnly, setKidsOnly] = useState(false);

  useEffect(() => {
    curatedPodcasts().then(setSuggestions).catch(() => {});
  }, []);

  useEffect(() => {
    if (open && canSearch && popular.length === 0) {
      popularKidsPodcasts().then(setPopular).catch(() => {});
    }
  }, [open, canSearch, popular.length]);

  const [rssUrl, setRssUrl] = useState("");
  const [directTitle, setDirectTitle] = useState("");
  const [directAudio, setDirectAudio] = useState("");
  const [directImage, setDirectImage] = useState("");

  useEffect(() => {
    const q = query.trim();
    if (q.length < 2) {
      setResults([]);
      setSearched(false);
      setSearching(false);
      setSearchError(null);
      return;
    }
    let cancelled = false;
    setSearching(true);
    const handle = setTimeout(() => {
      searchPodcasts(q)
        .then((res) => {
          if (cancelled) return;
          setResults(res);
          setSearched(true);
          setSearchError(null);
        })
        .catch((e) => {
          if (!cancelled) setSearchError(String(e));
        })
        .finally(() => {
          if (!cancelled) setSearching(false);
        });
    }, 300);
    return () => {
      cancelled = true;
      clearTimeout(handle);
    };
  }, [query]);

  const renderRow = (r: PodcastSearchResult) => (
    <div key={r.feedUrl} className="flex items-center gap-3 rounded-md px-1 py-2 hover:bg-accent/50">
      <Thumbnail src={r.imageUrl} size={40} />
      <div className="min-w-0 flex-1">
        <div className="truncate text-sm font-medium">{r.title}</div>
        <div className="truncate text-xs text-muted-foreground">
          {r.episodeCount != null && `${r.episodeCount} épisode(s)`}
          {r.episodeCount != null && r.genre ? " · " : ""}
          {r.genre}
        </div>
      </div>
      {added.has(r.feedUrl) ? (
        <span className="flex items-center gap-1 text-xs text-[var(--success)]">
          <Check className="size-4" /> Ajouté
        </span>
      ) : (
        <Button size="sm" variant="outline" onClick={() => addFeed(r.feedUrl, true)}>
          <Plus className="size-4" /> Ajouter
        </Button>
      )}
    </div>
  );

  const addFeed = async (feedUrl: string, keepOpen: boolean) => {
    await addRss(feedUrl);
    await reloadSubscriptions();
    void loadAllFeeds();
    if (keepOpen) {
      setAdded((s) => new Set(s).add(feedUrl));
    } else {
      onAdded(feedUrl);
      onOpenChange(false);
    }
  };

  const submitRss = async () => {
    const url = rssUrl.trim();
    if (!url) return;
    await addFeed(url, false);
    setRssUrl("");
  };

  const chooseAudio = async () => {
    const path = await pickAudio();
    if (!path) return;
    setDirectAudio(`file://${path}`);
    if (!directTitle.trim()) setDirectTitle(baseName(path));
  };

  const chooseImage = async () => {
    const path = await pickImage();
    if (path) setDirectImage(`file://${path}`);
  };

  const submitDirect = async () => {
    const title = directTitle.trim();
    const audio = directAudio.trim();
    if (!title || !audio) return;
    await addDirect(title, audio, directImage.trim() || null);
    await reloadSubscriptions();
    setDirectTitle("");
    setDirectAudio("");
    setDirectImage("");
    onOpenChange(false);
  };

  const shownResults = kidsOnly ? results.filter(isKidsGenre) : results;

  const curatedFeeds = new Set(suggestions.map((s) => s.feedUrl));
  const popularKids = popular.filter((p) => isKidsGenre(p) && !curatedFeeds.has(p.feedUrl));

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent fullscreen>
        <div className="mx-auto flex min-h-0 w-full max-w-2xl flex-1 flex-col">
        <DialogHeader>
          <DialogTitle>Ajouter une source</DialogTitle>
        </DialogHeader>
        <Tabs defaultValue="search" className="mt-4 flex min-h-0 flex-1 flex-col">
          <TabsList>
            <TabsTrigger value="search">Rechercher</TabsTrigger>
            <TabsTrigger value="rss">Flux RSS</TabsTrigger>
            <TabsTrigger value="direct">Fichier audio</TabsTrigger>
          </TabsList>

          <TabsContent value="search" className="mt-4 flex min-h-0 flex-1 flex-col gap-3">
            {!canSearch && (
              <div className="flex items-start gap-2 rounded-md border border-[var(--warning)]/40 bg-[var(--warning)]/10 px-3 py-2 text-sm">
                <TriangleAlert className="mt-0.5 size-4 shrink-0 text-[var(--warning)]" />
                <span>
                  {networkContext === "merlin"
                    ? "Connecté à la Merlin : pas d'accès internet. Reconnecte-toi à ton WiFi habituel pour chercher des podcasts."
                    : "Aucun accès internet. La recherche de podcasts a besoin d'internet."}
                </span>
              </div>
            )}
            <div className="relative">
              <Search className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
              <Input
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder="Rechercher un podcast (ex. Encore une histoire, professeur…)"
                className="pl-9 pr-9"
                disabled={!canSearch}
                autoFocus
              />
              {searching && (
                <Loader2 className="absolute right-3 top-1/2 size-4 -translate-y-1/2 animate-spin text-muted-foreground" />
              )}
            </div>
            <label className="flex items-center gap-2 text-sm text-muted-foreground">
              <Checkbox checked={kidsOnly} onCheckedChange={(v) => setKidsOnly(v === true)} />
              Podcasts pour enfants uniquement
            </label>
            {searchError && <p className="text-sm text-destructive">{searchError}</p>}
            <div className="min-h-0 flex-1 overflow-y-auto">
              {shownResults.length > 0 ? (
                shownResults.map(renderRow)
              ) : searching ? null : (
                <>
                  {searched && (
                    <p className="px-1 pt-2 text-sm text-muted-foreground">
                      Aucun résultat{kidsOnly ? " pour enfants" : ""} — voici quelques suggestions :
                    </p>
                  )}
                  <p className="px-1 pb-1 pt-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
                    Suggestions pour enfants
                  </p>
                  {suggestions.map(renderRow)}
                  {popularKids.length > 0 && (
                    <>
                      <p className="px-1 pb-1 pt-4 text-xs font-medium uppercase tracking-wide text-muted-foreground">
                        Populaires en ce moment
                      </p>
                      {popularKids.map(renderRow)}
                    </>
                  )}
                </>
              )}
            </div>
          </TabsContent>

          <TabsContent value="rss" className="mt-4 flex flex-col gap-3">
            <label className="text-sm text-muted-foreground">URL du flux RSS</label>
            <Input
              value={rssUrl}
              onChange={(e) => setRssUrl(e.target.value)}
              placeholder="https://…/feed.xml"
              onKeyDown={(e) => { if (e.key === "Enter") void submitRss(); }}
            />
            <Button className="self-end" disabled={!rssUrl.trim()} onClick={submitRss}>
              Ajouter
            </Button>
          </TabsContent>

          <TabsContent value="direct" className="mt-4 flex flex-col gap-3">
            <Input
              value={directTitle}
              onChange={(e) => setDirectTitle(e.target.value)}
              placeholder="Titre"
            />
            <div className="flex gap-2">
              <Input
                value={directAudio}
                onChange={(e) => setDirectAudio(e.target.value)}
                placeholder="Fichier audio de l'ordinateur, ou URL"
              />
              <Button variant="outline" onClick={chooseAudio} title="Choisir un fichier">
                <FolderOpen />
              </Button>
            </div>
            <div className="flex gap-2">
              <Input
                value={directImage}
                onChange={(e) => setDirectImage(e.target.value)}
                placeholder="Image (optionnelle) — fichier ou URL"
              />
              <Button variant="outline" onClick={chooseImage} title="Choisir une image">
                <FolderOpen />
              </Button>
            </div>
            <Button
              className="self-end"
              disabled={!directTitle.trim() || !directAudio.trim()}
              onClick={submitDirect}
            >
              Ajouter
            </Button>
          </TabsContent>
        </Tabs>
        </div>
      </DialogContent>
    </Dialog>
  );
}
