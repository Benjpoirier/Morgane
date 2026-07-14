import { useState } from "react";
import { useDraggable } from "@dnd-kit/core";
import { Plus, Pencil, Image, X, ChevronRight, ChevronDown, GripVertical } from "lucide-react";
import { Input } from "@/components/ui/input";
import { Thumbnail } from "@/components/Thumbnail";
import { ByteBadge } from "@/components/ByteBadge";
import { ImageOverrideEditor } from "@/components/ImageOverrideEditor";
import { useInlineEdit } from "@/hooks/useInlineEdit";
import { useImageOverride } from "@/hooks/useImageOverride";
import { usePodcasts } from "@/store/podcasts";
import { useTree } from "@/store/tree";
import {
  markPendingDeletion,
  setEpisodeImageOverride,
  setEpisodeTitleOverride,
  setFolderImageOverride,
  setGroupTitleOverride,
} from "@/lib/ipc";
import type { Addition, Changeset, SimpleChange, UndoAction } from "@/lib/changeset";
import type { Episode } from "@/lib/types";
import { cn } from "@/lib/utils";

function useUndo() {
  const deselectEpisodes = usePodcasts((s) => s.deselectEpisodes);
  const reloadSyncState = usePodcasts((s) => s.reloadSyncState);
  const cancelEdit = useTree((s) => s.cancelEdit);
  const toggleOrphan = useTree((s) => s.toggleOrphan);
  return async (action: UndoAction) => {
    switch (action.kind) {
      case "deselectGroup":
        await deselectEpisodes(action.feedUrl, action.guids);
        await reloadSyncState();
        break;
      case "cancelEdit":
        await cancelEdit(action.uuid, action.editType);
        break;
      case "unmarkDeletion":
        await markPendingDeletion(action.episodeUuid, false);
        await reloadSyncState();
        break;
      case "toggleOrphan":
        await toggleOrphan(action.name);
        break;
      case "clearFolderImage":
        await setFolderImageOverride(action.uuid, null);
        await reloadSyncState();
        break;
    }
  };
}

export function ChangesetPane({ changeset: cs }: { changeset: Changeset }) {
  const undo = useUndo();

  if (cs.total === 0) {
    return (
      <div className="flex h-full items-center justify-center px-8 text-center text-sm text-muted-foreground">
        Aucun changement à synchroniser. Sélectionne des épisodes, édite ou supprime du contenu.
      </div>
    );
  }

  return (
    <div className="space-y-5 p-4">
      <Section title="À ajouter" count={cs.additions.length} accent="add">
        {cs.additions.map((a) => (
          <AdditionRow key={a.group.uuid} addition={a} onUndo={() => undo(a.undo)} />
        ))}
      </Section>
      <Section title="Modifier" count={cs.modifications.length} accent="modify">
        {cs.modifications.map((c) => (
          <SimpleRow key={c.id} change={c} onUndo={() => undo(c.undo)} accent="modify" />
        ))}
      </Section>
      <Section title="Supprimer" count={cs.deletions.length} accent="delete">
        {cs.deletions.map((c) => (
          <SimpleRow key={c.id} change={c} onUndo={() => undo(c.undo)} accent="delete" />
        ))}
      </Section>
    </div>
  );
}

type Accent = "add" | "modify" | "delete";

const ACCENT: Record<Accent, { bar: string; border: string; text: string; chip: string }> = {
  add: {
    bar: "bg-[var(--success)]",
    border: "border-[var(--success)]/40",
    text: "text-[var(--success)]",
    chip: "bg-[var(--success)]/15 text-[var(--success)]",
  },
  modify: {
    bar: "bg-[var(--warning)]",
    border: "border-[var(--warning)]/40",
    text: "text-[var(--warning)]",
    chip: "bg-[var(--warning)]/15 text-[var(--warning)]",
  },
  delete: {
    bar: "bg-destructive",
    border: "border-destructive/40",
    text: "text-destructive",
    chip: "bg-destructive/15 text-destructive",
  },
};

function Section({
  title,
  count,
  accent,
  children,
}: {
  title: string;
  count: number;
  accent: Accent;
  children: React.ReactNode;
}) {
  if (count === 0) return null;
  const a = ACCENT[accent];
  return (
    <div>
      <div className="mb-2 flex items-center gap-2">
        <span className={cn("h-4 w-1 rounded-full", a.bar)} />
        <h2 className={cn("text-sm font-semibold uppercase tracking-wide", a.text)}>{title}</h2>
        <span className={cn("rounded-full px-1.5 text-xs font-semibold tabular-nums", a.chip)}>{count}</span>
      </div>
      <div className={cn("space-y-1 border-l-2 pl-3", a.border)}>{children}</div>
    </div>
  );
}

function UndoButton({ onUndo, title }: { onUndo: () => void; title: string }) {
  return (
    <button
      onClick={onUndo}
      title={title}
      className="rounded p-1 text-muted-foreground/60 hover:bg-muted hover:text-foreground"
    >
      <X className="size-3.5" />
    </button>
  );
}

function SimpleRow({ change, onUndo, accent }: { change: SimpleChange; onUndo: () => void; accent: Accent }) {
  return (
    <div className="flex items-center gap-2 rounded-md px-1 py-1 text-sm hover:bg-accent/50">
      <span className="min-w-0 flex-1 truncate">
        {change.label}
        {change.detail && <span className="ml-1.5 text-xs text-muted-foreground">{change.detail}</span>}
      </span>
      <UndoButton onUndo={onUndo} title={accent === "delete" ? "Ne pas supprimer" : "Annuler la modification"} />
    </div>
  );
}

function AdditionRow({ addition, onUndo }: { addition: Addition; onUndo: () => void }) {
  const { group, targetCategoryTitle, renamed, hasImage } = addition;
  const reloadSyncState = usePodcasts((s) => s.reloadSyncState);
  const [expanded, setExpanded] = useState(false);
  const { attributes, listeners, setNodeRef, transform, isDragging } = useDraggable({
    id: `group:${group.uuid}`,
    data: { kind: "pendingGroup", feedUrl: group.feedUrl, groupKey: group.groupKey },
  });
  const rename = useInlineEdit(async (value) => {
    const v = value.trim();
    if (v && v !== group.title) {
      await setGroupTitleOverride(group.feedUrl, group.groupKey, v);
      await reloadSyncState();
    }
  });
  const image = useImageOverride(async (source) => {
    await setFolderImageOverride(group.uuid, source);
    await reloadSyncState();
  });

  return (
    <div className="rounded-md">
      <div
        ref={setNodeRef}
        style={{
          transform: transform ? `translate(${transform.x}px, ${transform.y}px)` : undefined,
          opacity: isDragging ? 0.5 : 1,
        }}
        className="flex items-center gap-1.5 px-1 py-1"
      >
        <button onClick={() => setExpanded((e) => !e)} className="text-muted-foreground">
          {expanded ? <ChevronDown className="size-4" /> : <ChevronRight className="size-4" />}
        </button>
        <button {...listeners} {...attributes} className="cursor-grab text-muted-foreground active:cursor-grabbing">
          <GripVertical className="size-4" />
        </button>
        <Plus className="size-3.5 text-[var(--success)]" />
        <Thumbnail src={group.feedImageUrl} size={28} />
        <div className="min-w-0 flex-1">
          {rename.editing ? (
            <Input defaultValue={group.title} {...rename.inputProps} className="h-7" />
          ) : (
            <div className="flex items-center gap-1.5">
              <span className="truncate text-sm font-medium" onDoubleClick={rename.begin}>
                {group.title}
              </span>
              <ByteBadge value={group.title} />
              {renamed && <Badge>renommé</Badge>}
              {hasImage && <Badge>image</Badge>}
            </div>
          )}
          {targetCategoryTitle ? (
            <div className="text-xs text-[var(--success)]">→ {targetCategoryTitle}</div>
          ) : (
            <div className="text-xs text-[var(--warning)]">non placé — glisser vers une catégorie</div>
          )}
        </div>
        <span className="shrink-0 text-xs text-muted-foreground tabular-nums">{group.episodes.length}</span>
        <button
          onClick={rename.begin}
          title="Renommer"
          className="rounded p-1 text-muted-foreground/50 hover:bg-muted hover:text-foreground"
        >
          <Pencil className="size-3.5" />
        </button>
        <button
          onClick={image.browse}
          title="Changer le visuel"
          className={cn(
            "rounded p-1 hover:bg-muted",
            hasImage ? "text-primary" : "text-muted-foreground/50 hover:text-foreground",
          )}
        >
          <Image className="size-3.5" />
        </button>
        <UndoButton onUndo={onUndo} title="Retirer de la synchro (dé-sélectionner)" />
      </div>
      {expanded && (
        <div className="ml-9 pl-2">
          {group.episodes.map((e) => (
            <AdditionEpisodeRow key={e.guid} episode={e} feedImageUrl={group.feedImageUrl} />
          ))}
        </div>
      )}
    </div>
  );
}

function AdditionEpisodeRow({ episode, feedImageUrl }: { episode: Episode; feedImageUrl: string | null }) {
  const syncState = usePodcasts((s) => s.syncState);
  const reloadSyncState = usePodcasts((s) => s.reloadSyncState);
  const hasImage = !!syncState?.episodeImageOverrides[episode.guid];
  const rename = useInlineEdit(async (value) => {
    const v = value.trim();
    if (v && v !== episode.title) {
      await setEpisodeTitleOverride(episode.guid, v);
      await reloadSyncState();
    }
  });
  const image = useImageOverride(async (source) => {
    await setEpisodeImageOverride(episode.guid, source);
    await reloadSyncState();
  });
  return (
    <div>
      <div className="flex items-center gap-1.5 py-0.5 text-sm">
        <Thumbnail src={episode.imageUrl || feedImageUrl} size={24} />
        <div className="min-w-0 flex-1">
          {rename.editing ? (
            <Input defaultValue={episode.title} {...rename.inputProps} className="h-6" />
          ) : (
            <div className="flex items-center gap-1.5">
              <span className="truncate text-muted-foreground" onDoubleClick={rename.begin}>
                {episode.title}
              </span>
              <ByteBadge value={episode.title} />
            </div>
          )}
        </div>
        <button
          onClick={rename.begin}
          title="Renommer"
          className="rounded p-1 text-muted-foreground/50 hover:bg-muted hover:text-foreground"
        >
          <Pencil className="size-3.5" />
        </button>
        <button
          onClick={() => image.begin(syncState?.episodeImageOverrides[episode.guid] ?? "")}
          title="Changer le visuel"
          className={cn(
            "rounded p-1 hover:bg-muted",
            hasImage ? "text-primary" : "text-muted-foreground/50 hover:text-foreground",
          )}
        >
          <Image className="size-3.5" />
        </button>
      </div>
      {image.editing && (
        <div className="py-1 pl-5">
          <ImageOverrideEditor image={image} />
        </div>
      )}
    </div>
  );
}

function Badge({ children }: { children: React.ReactNode }) {
  return (
    <span className="rounded-full bg-[var(--warning)]/15 px-1.5 text-[10px] font-medium text-[var(--warning)]">
      {children}
    </span>
  );
}
