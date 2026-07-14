import { useState } from "react";
import { useDraggable, useDroppable } from "@dnd-kit/core";
import {
  ChevronRight,
  ChevronDown,
  Folder,
  Star,
  Music,
  Trash2,
  Undo2,
  Check,
  Image,
  Pencil,
} from "lucide-react";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Thumbnail } from "@/components/Thumbnail";
import { ByteBadge } from "@/components/ByteBadge";
import { ImageOverrideEditor } from "@/components/ImageOverrideEditor";
import { useTree } from "@/store/tree";
import { usePodcasts } from "@/store/podcasts";
import { useInlineEdit } from "@/hooks/useInlineEdit";
import { useImageOverride } from "@/hooks/useImageOverride";
import { markPendingDeletion, setFolderImageOverride } from "@/lib/ipc";
import type { PlaylistFolder, PlaylistNode, PendingGroup } from "@/lib/types";
import { cn } from "@/lib/utils";

export function CategoryDetailPane({
  category,
  pendingGroups,
  onDeleted,
}: {
  category: PlaylistFolder;
  pendingGroups: PendingGroup[];
  onDeleted?: () => void;
}) {
  const thumbnails = useTree((s) => s.thumbnails);
  const renameFolder = useTree((s) => s.renameFolder);
  const removeCategory = useTree((s) => s.removeCategory);
  const syncState = usePodcasts((s) => s.syncState);
  const reloadSyncState = usePodcasts((s) => s.reloadSyncState);
  const [confirmingDelete, setConfirmingDelete] = useState(false);

  const title = useInlineEdit(async (value) => {
    const v = value.trim();
    if (v && v !== category.title) await renameFolder(category.uuid, v);
  });
  const image = useImageOverride(async (source) => {
    await setFolderImageOverride(category.uuid, source);
    await reloadSyncState();
  });

  const assignments = syncState?.categoryAssignments ?? {};
  const assigned = pendingGroups.filter(
    (g) => assignments[`${g.feedUrl}|${g.groupKey}`]?.targetCategoryUuid === category.uuid,
  );
  const hasImageOverride = !!syncState?.folderImageOverrides[category.uuid];
  const isManual = syncState?.manualCategories.some((c) => c.uuid === category.uuid) ?? false;

  const confirmDelete = async () => {
    await removeCategory(category.uuid);
    await reloadSyncState();
    setConfirmingDelete(false);
    onDeleted?.();
  };

  return (
    <div className="p-4">
      <div className="flex items-start gap-3">
        <Thumbnail src={thumbnails[category.uuid] ?? null} size={64} />
        <div className="min-w-0 flex-1">
          {title.editing ? (
            <Input
              defaultValue={category.title}
              {...title.inputProps}
              className="h-8 text-lg font-semibold"
            />
          ) : (
            <div className="flex items-center gap-2">
              <h1 className="truncate text-lg font-semibold" onDoubleClick={title.begin}>
                {category.title}
              </h1>
              <ByteBadge value={category.title} />
              <button
                onClick={title.begin}
                title="Renommer la catégorie"
                className="rounded p-1 text-muted-foreground/60 hover:bg-muted hover:text-foreground"
              >
                <Pencil className="size-3.5" />
              </button>
              {isManual && (
                <button
                  onClick={() => setConfirmingDelete(true)}
                  title="Supprimer cette catégorie"
                  className="rounded p-1 text-muted-foreground/60 hover:bg-muted hover:text-destructive"
                >
                  <Trash2 className="size-3.5" />
                </button>
              )}
            </div>
          )}

          {image.editing ? (
            <div className="mt-1">
              <ImageOverrideEditor image={image} />
            </div>
          ) : (
            <button
              onClick={() => image.begin(syncState?.folderImageOverrides[category.uuid] ?? "")}
              className={cn(
                "mt-1 flex items-center gap-1.5 text-sm",
                hasImageOverride ? "text-primary" : "text-muted-foreground hover:text-foreground",
              )}
            >
              <Image className="size-3.5" />
              {hasImageOverride ? "Visuel personnalisé ✓" : "Changer le visuel…"}
            </button>
          )}
        </div>
      </div>

      {assigned.length > 0 && (
        <div className="mt-4 rounded-lg bg-primary/8 p-2">
          <div className="mb-1 text-xs font-semibold text-primary uppercase">
            Groupes à envoyer ici
          </div>
          {assigned.map((g) => (
            <div key={g.uuid} className="flex items-center gap-2 py-0.5 text-sm">
              <span className="opacity-60">⏳</span>
              <span className="truncate">{g.title}</span>
              <span className="text-xs text-muted-foreground">({g.episodes.length} ép.)</span>
            </div>
          ))}
        </div>
      )}

      <div className="mt-4">
        {category.children.length === 0 && assigned.length === 0 ? (
          <p className="py-4 text-center text-sm text-muted-foreground">
            Catégorie vide. Glisse des groupes ici depuis « À synchroniser ».
          </p>
        ) : (
          category.children.map((node) => <TreeNodeRow key={node.uuid} node={node} depth={0} />)
        )}
      </div>

      <Dialog open={confirmingDelete} onOpenChange={setConfirmingDelete}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle>Supprimer la catégorie « {category.title} » ?</DialogTitle>
            <DialogDescription>
              La catégorie disparaît de tes cibles ; les groupes qui y étaient assignés repassent
              « non placés ». Les épisodes eux-mêmes ne sont pas supprimés.
            </DialogDescription>
          </DialogHeader>
          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={() => setConfirmingDelete(false)}>
              Annuler
            </Button>
            <Button variant="destructive" onClick={confirmDelete}>
              Supprimer la catégorie
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </div>
  );
}

function TreeNodeRow({ node, depth }: { node: PlaylistNode; depth: number }) {
  const renameFolder = useTree((s) => s.renameFolder);
  const renameSound = useTree((s) => s.renameSound);
  const thumbnails = useTree((s) => s.thumbnails);
  const syncState = usePodcasts((s) => s.syncState);
  const reloadSyncState = usePodcasts((s) => s.reloadSyncState);
  const [expanded, setExpanded] = useState(true);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const [confirmingFolderDelete, setConfirmingFolderDelete] = useState(false);

  const uuid = node.uuid;
  const title = node.title;
  const isFolder = node.kind === "folder";
  const hasImageOverride = !!syncState?.folderImageOverrides[uuid];

  const rename = useInlineEdit(async (value) => {
    const v = value.trim();
    if (v && v !== title) {
      if (isFolder) await renameFolder(uuid, v);
      else await renameSound(uuid, v);
    }
  });
  const image = useImageOverride(async (source) => {
    await setFolderImageOverride(uuid, source);
    await reloadSyncState();
  });
  const deleteFolder = useTree((s) => s.deleteFolder);
  const confirmFolderDelete = async () => {
    await deleteFolder(uuid);
    await reloadSyncState();
    setConfirmingFolderDelete(false);
  };

  const { attributes, listeners, setNodeRef: dragRef, isDragging } = useDraggable({
    id: `node:${uuid}`,
    data: { kind: "treeNode", uuid },
  });
  const { setNodeRef: dropRef, isOver } = useDroppable({
    id: `folder:${uuid}`,
    data: { kind: "folder", uuid, title },
    disabled: !isFolder,
  });

  const record = syncState?.syncedRecords.find((r) => r.episodeUuid === uuid);

  const requestToggleDeletion = async () => {
    if (!record) return;
    if (record.pendingDeletion) {
      await markPendingDeletion(uuid, false);
      await reloadSyncState();
    } else {
      setConfirmingDelete(true);
    }
  };
  const confirmDeletion = async () => {
    await markPendingDeletion(uuid, true);
    await reloadSyncState();
    setConfirmingDelete(false);
  };

  return (
    <div>
      <div
        ref={isFolder ? dropRef : undefined}
        style={{ paddingLeft: depth * 16 }}
        className={cn(
          "flex items-center gap-1.5 rounded-md py-1 pr-1.5 hover:bg-accent",
          isOver && "ring-2 ring-primary ring-inset",
          isDragging && "opacity-50",
        )}
      >
        <button
          ref={dragRef}
          {...listeners}
          {...attributes}
          className="cursor-grab text-muted-foreground/60 active:cursor-grabbing"
          title="Glisser pour déplacer"
        >
          {isFolder ? <Folder className="size-4" /> : <Music className="size-4" />}
        </button>

        {isFolder && node.children.length > 0 && (
          <button onClick={() => setExpanded((e) => !e)} className="text-muted-foreground">
            {expanded ? <ChevronDown className="size-3.5" /> : <ChevronRight className="size-3.5" />}
          </button>
        )}
        {isFolder && node.isFavorite && <Star className="size-3.5 text-[var(--warning)]" />}
        {isFolder && thumbnails[uuid] && <Thumbnail src={thumbnails[uuid]} size={20} />}

        <div className="min-w-0 flex-1">
          {rename.editing ? (
            <Input defaultValue={title} {...rename.inputProps} className="h-6" />
          ) : (
            <span className="flex items-center gap-1.5">
              <span className="truncate text-sm" onDoubleClick={rename.begin}>
                {title}
              </span>
              <ByteBadge value={title} />
            </span>
          )}
        </div>

        {}
        <button
          onClick={rename.begin}
          title="Renommer"
          className="rounded p-1 text-muted-foreground/50 hover:bg-muted hover:text-foreground"
        >
          <Pencil className="size-3.5" />
        </button>
        {isFolder && (
          <button
            onClick={() => image.begin(syncState?.folderImageOverrides[uuid] ?? "")}
            title="Changer le visuel du dossier"
            className={cn(
              "rounded p-1 hover:bg-muted",
              hasImageOverride ? "text-primary" : "text-muted-foreground/50 hover:text-foreground",
            )}
          >
            <Image className="size-3.5" />
          </button>
        )}
        {isFolder && (
          <button
            onClick={() => setConfirmingFolderDelete(true)}
            title="Supprimer ce dossier et tout son contenu"
            className="rounded p-1 text-muted-foreground/50 hover:bg-muted hover:text-destructive"
          >
            <Trash2 className="size-3.5" />
          </button>
        )}
        {node.kind === "sound" &&
          (record ? (
            <button
              onClick={requestToggleDeletion}
              className={cn(
                "rounded p-1",
                record.pendingDeletion
                  ? "text-[var(--warning)]"
                  : "text-muted-foreground/50 hover:text-destructive",
              )}
              title={record.pendingDeletion ? "Annuler la suppression" : "Marquer à supprimer"}
            >
              {record.pendingDeletion ? (
                <Undo2 className="size-3.5" />
              ) : (
                <Trash2 className="size-3.5" />
              )}
            </button>
          ) : (
            <span title="Présent hors Morgane">
              <Check className="size-3.5 text-[var(--success)]" />
            </span>
          ))}
      </div>

      {image.editing && (
        <div style={{ paddingLeft: (depth + 1) * 16 }} className="py-1 pr-1.5">
          <ImageOverrideEditor image={image} />
        </div>
      )}

      {isFolder && expanded && (
        <div>
          {node.children.map((child) => (
            <TreeNodeRow key={child.uuid} node={child} depth={depth + 1} />
          ))}
        </div>
      )}

      <Dialog open={confirmingDelete} onOpenChange={setConfirmingDelete}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle>Supprimer « {title} » de l'enceinte ?</DialogTitle>
            <DialogDescription>
              Le fichier sera supprimé de la carte SD au prochain « Synchroniser ». Il faudra
              re-synchroniser pour le remettre.
            </DialogDescription>
          </DialogHeader>
          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={() => setConfirmingDelete(false)}>
              Annuler
            </Button>
            <Button variant="destructive" onClick={confirmDeletion}>
              Marquer pour suppression
            </Button>
          </div>
        </DialogContent>
      </Dialog>

      <Dialog open={confirmingFolderDelete} onOpenChange={setConfirmingFolderDelete}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle>Supprimer le dossier « {title} » ?</DialogTitle>
            <DialogDescription>
              Le dossier <strong>et tout son contenu</strong> seront retirés du menu, et les
              fichiers supprimés de la carte SD au prochain « Synchroniser ».
            </DialogDescription>
          </DialogHeader>
          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={() => setConfirmingFolderDelete(false)}>
              Annuler
            </Button>
            <Button variant="destructive" onClick={confirmFolderDelete}>
              Supprimer le dossier
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </div>
  );
}
