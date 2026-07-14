import { useState } from "react";
import { DndContext, useDroppable } from "@dnd-kit/core";
import {
  RefreshCw,
  FolderPlus,
  Loader2,
  Star,
  Folder,
  Search,
  Stethoscope,
  Inbox,
  Speaker,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Thumbnail } from "@/components/Thumbnail";
import { SyncProgressModal } from "@/components/SyncProgressModal";
import { AddCategorySheet } from "@/components/AddCategorySheet";
import { CategoryDetailPane } from "@/components/CategoryDetailPane";
import { ChangesetPane } from "@/components/ChangesetPane";
import { PrepareDetailModal } from "@/components/PrepareDetailModal";
import { OrphanPane } from "@/components/OrphanPane";
import { IntegrityPane } from "@/components/IntegrityPane";
import { useTree } from "@/store/tree";
import { useSyncView } from "@/hooks/useSyncView";
import { useSyncFooter } from "@/hooks/useSyncFooter";
import type { PendingGroup, PlaylistFolder } from "@/lib/types";
import { cn } from "@/lib/utils";

type Selection =
  | { kind: "pending" }
  | { kind: "category"; uuid: string }
  | { kind: "orphans" }
  | { kind: "integrity" };

function countDescendants(folder: PlaylistFolder): number {
  return folder.children.reduce(
    (n, c) => n + (c.kind === "folder" ? 1 + countDescendants(c) : 1),
    0,
  );
}

export function SyncView() {
  const [selection, setSelection] = useState<Selection>({ kind: "pending" });
  const [showAddCategory, setShowAddCategory] = useState(false);
  const {
    tree,
    host,
    port,
    pendingGroups,
    changeset,
    unplaced,
    missingImages,
    assignedCounts,
    selectedCount,
    deletionCount,
    integrityMissing,
    onDragEnd,
  } = useSyncView();

  return (
    <DndContext onDragEnd={onDragEnd}>
      <div className="flex h-full flex-col">
        {}
        <div className="flex items-center justify-between border-b px-4 py-2 text-sm">
          <div className="flex items-center gap-3">
            <span className="font-medium">
              {selectedCount} épisode(s) sélectionné(s)
            </span>
            {deletionCount > 0 && (
              <span className="rounded-full bg-destructive/15 px-2 py-0.5 text-xs font-medium text-destructive tabular-nums">
                {deletionCount} suppression(s)
              </span>
            )}
          </div>
          <span className="flex items-center gap-1.5 text-xs text-muted-foreground tabular-nums">
            <Speaker className="size-3.5" />
            {host}:{port}
          </span>
        </div>
        <div className="flex min-h-0 flex-1">
          {}
          <div className="flex w-72 flex-col border-r">
            <div className="flex items-center gap-2 p-3 pb-2">
              <Button
                size="sm"
                variant="outline"
                disabled={tree.loading}
                onClick={() => tree.refresh(host, port)}
              >
                {tree.loading ? <Loader2 className="animate-spin" /> : <RefreshCw />}
                Actualiser
              </Button>
              <Button size="sm" variant="ghost" onClick={() => setShowAddCategory(true)}>
                <FolderPlus />
                Catégorie
              </Button>
            </div>
            {tree.loadError && (
              <p className="px-3 text-sm text-destructive">{tree.loadError}</p>
            )}
            <div className="flex-1 overflow-y-auto px-2 pb-2">
              <button
                onClick={() => setSelection({ kind: "pending" })}
                className={cn(
                  "flex w-full items-center gap-2 rounded-md px-2.5 py-2 text-left text-sm",
                  selection.kind === "pending" ? "bg-primary/12" : "hover:bg-accent",
                )}
              >
                <Inbox className="size-4" />
                <span className="flex-1">À synchroniser</span>
                <span
                  className={cn(
                    "text-xs tabular-nums",
                    unplaced > 0 ? "text-[var(--warning)]" : "text-muted-foreground",
                  )}
                >
                  {changeset.total}
                </span>
              </button>

              <div className="my-1.5 h-px bg-border" />

              {tree.folders
                .filter((f) => !f.isSynthetic)
                .map((f) => (
                  <CategoryRow
                    key={f.uuid}
                    folder={f}
                    selected={selection.kind === "category" && selection.uuid === f.uuid}
                    assignedCount={assignedCounts[f.uuid] ?? 0}
                    onSelect={() => setSelection({ kind: "category", uuid: f.uuid })}
                  />
                ))}

              <div className="mt-3 px-2 text-xs font-semibold text-muted-foreground uppercase">
                Outils
              </div>
              <ToolRow
                icon={Search}
                label="Fichiers retrouvés"
                active={selection.kind === "orphans"}
                onClick={() => setSelection({ kind: "orphans" })}
              />
              <ToolRow
                icon={Stethoscope}
                label="Vérifier l'intégrité"
                active={selection.kind === "integrity"}
                onClick={() => setSelection({ kind: "integrity" })}
                badge={integrityMissing > 0 ? integrityMissing : undefined}
              />
            </div>
          </div>

          {}
          <div className="min-w-0 flex-1 overflow-y-auto">
            {selection.kind === "pending" && <ChangesetPane changeset={changeset} />}
            {selection.kind === "category" &&
              (() => {
                const folder = tree.folders.find((f) => f.uuid === selection.uuid);
                return folder ? (
                  <CategoryDetailPane
                    category={folder}
                    pendingGroups={pendingGroups}
                    onDeleted={() => setSelection({ kind: "pending" })}
                  />
                ) : null;
              })()}
            {selection.kind === "orphans" && <OrphanPane />}
            {selection.kind === "integrity" && <IntegrityPane />}
          </div>
        </div>

        <SyncFooter
          pendingGroups={pendingGroups}
          unplaced={unplaced}
          missingImages={missingImages}
          changeTotal={changeset.total}
        />
      </div>

      <AddCategorySheet open={showAddCategory} onOpenChange={setShowAddCategory} />
      <SyncProgressModal />
    </DndContext>
  );
}

function CategoryRow({
  folder,
  selected,
  assignedCount,
  onSelect,
}: {
  folder: PlaylistFolder;
  selected: boolean;
  assignedCount: number;
  onSelect: () => void;
}) {
  const thumb = useTree((s) => s.thumbnails[folder.uuid]);
  const { setNodeRef, isOver } = useDroppable({
    id: `cat:${folder.uuid}`,
    data: { kind: "category", uuid: folder.uuid, title: folder.title },
  });
  return (
    <button
      ref={setNodeRef}
      onClick={onSelect}
      className={cn(
        "flex w-full items-center gap-2 rounded-md px-2.5 py-1.5 text-left text-sm transition-colors",
        selected ? "bg-primary/12" : "hover:bg-accent",
        isOver && "ring-2 ring-primary ring-inset",
      )}
    >
      {thumb ? (
        <Thumbnail src={thumb} size={22} />
      ) : folder.isFavorite ? (
        <Star className="size-4 text-[var(--warning)]" />
      ) : (
        <Folder className="size-4 text-muted-foreground" />
      )}
      <span className="flex-1 truncate">{folder.title}</span>
      <span className="text-xs text-muted-foreground tabular-nums">
        {countDescendants(folder)}
      </span>
      {assignedCount > 0 && (
        <span className="rounded-full bg-primary/15 px-1.5 text-xs text-primary tabular-nums">
          +{assignedCount}
        </span>
      )}
    </button>
  );
}

function ToolRow({
  icon: Icon,
  label,
  active,
  onClick,
  badge,
}: {
  icon: typeof Search;
  label: string;
  active: boolean;
  onClick: () => void;
  badge?: number;
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "flex w-full items-center gap-2 rounded-md px-2.5 py-1.5 text-left text-sm",
        active ? "bg-primary/12" : "hover:bg-accent",
      )}
    >
      <Icon className={cn("size-4", badge ? "text-[var(--warning)]" : "text-muted-foreground")} />
      <span className="flex-1">{label}</span>
      {badge !== undefined && (
        <span className="rounded-full bg-[var(--warning)]/20 px-1.5 text-xs font-semibold text-[var(--warning)] tabular-nums">
          {badge}
        </span>
      )}
    </button>
  );
}

function SyncFooter({
  pendingGroups,
  unplaced,
  missingImages,
  changeTotal,
}: {
  pendingGroups: PendingGroup[];
  unplaced: number;
  missingImages: number;
  changeTotal: number;
}) {
  const [showPrepareDetail, setShowPrepareDetail] = useState(false);
  const {
    host,
    port,
    isConnected,
    status,
    canSync,
    launch,
    allPrepared,
    needPreparePairs,
    needCount,
    notPreparedCount,
    failedCount,
  } = useSyncFooter({ pendingGroups, unplaced, missingImages, changeTotal });

  return (
    <div className="flex items-center justify-between border-t px-4 py-2.5">
      <span className="text-sm text-muted-foreground">{status}</span>
      <div className="flex items-center gap-3">
        {!allPrepared &&
          (notPreparedCount > failedCount ? (
            <button
              onClick={() => setShowPrepareDetail(true)}
              className="flex items-center gap-1.5 text-xs text-muted-foreground hover:text-foreground"
              title="Voir le détail de la préparation"
            >
              <Loader2 className="size-3.5 animate-spin" />
              Préparation… {needCount - notPreparedCount}/{needCount}
            </button>
          ) : failedCount > 0 ? (
            <button
              onClick={() => setShowPrepareDetail(true)}
              className="text-xs text-destructive underline underline-offset-2"
            >
              {failedCount} échec(s) de préparation — voir le détail
            </button>
          ) : null)}
        <span className="text-xs text-muted-foreground tabular-nums">
          {host}:{port}
        </span>
        <Button
          disabled={!canSync}
          onClick={launch}
          title={
            !allPrepared
              ? "Préparation des fichiers en cours — patiente avant de synchroniser."
              : !isConnected
                ? "Connecte-toi à ta Merlin pour transférer."
                : unplaced > 0
                  ? "Des groupes n'ont pas de catégorie : glisse-les à gauche."
                  : missingImages > 0
                    ? "Des épisodes n'ont aucun visuel : ajoute une image dans Podcasts."
                    : undefined
          }
        >
          Synchroniser
        </Button>
      </div>
      <PrepareDetailModal
        open={showPrepareDetail}
        onOpenChange={setShowPrepareDetail}
        pairs={needPreparePairs}
      />
    </div>
  );
}

