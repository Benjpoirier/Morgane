import { Check, X, Loader2, Clock } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { usePrepare } from "@/store/prepare";
import type { SelectedPair } from "@/lib/types";

export function PrepareDetailModal({
  open,
  onOpenChange,
  pairs,
}: {
  open: boolean;
  onOpenChange: (v: boolean) => void;
  pairs: SelectedPair[];
}) {
  const prepared = usePrepare((s) => s.prepared);
  const failed = usePrepare((s) => s.failed);
  const currentGuid = usePrepare((s) => s.currentGuid);
  const progress = usePrepare((s) => s.progress);
  const retry = usePrepare((s) => s.retry);

  const total = pairs.length;
  const readyCount = pairs.filter((p) => prepared.has(p.episode.guid)).length;
  const failedCount = pairs.filter((p) => p.episode.guid in failed).length;
  const overall = total > 0 ? readyCount / total : 0;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>Préparation des fichiers</DialogTitle>
        </DialogHeader>

        <div>
          <div className="mb-1 flex justify-between text-sm text-muted-foreground tabular-nums">
            <span>{readyCount}/{total} prêts</span>
            {failedCount > 0 && <span className="text-destructive">{failedCount} échec(s)</span>}
          </div>
          <div className="h-2 overflow-hidden rounded-full bg-muted">
            <div
              className="h-full rounded-full bg-primary transition-[width]"
              style={{ width: `${Math.round(overall * 100)}%` }}
            />
          </div>
        </div>

        <div className="flex max-h-[50vh] flex-col gap-1 overflow-y-auto">
          {pairs.map(({ episode }) => {
            const guid = episode.guid;
            const isReady = prepared.has(guid);
            const isFailed = guid in failed;
            const isCurrent = currentGuid === guid && !isReady;
            return (
              <div key={guid} className="flex flex-col gap-1 rounded-md px-1 py-1.5">
                <div className="flex items-center gap-2 text-sm">
                  {isReady ? (
                    <Check className="size-4 shrink-0 text-[var(--success)]" />
                  ) : isFailed ? (
                    <X className="size-4 shrink-0 text-destructive" />
                  ) : isCurrent ? (
                    <Loader2 className="size-4 shrink-0 animate-spin text-primary" />
                  ) : (
                    <Clock className="size-4 shrink-0 text-muted-foreground/60" />
                  )}
                  <span className="min-w-0 flex-1 truncate">{episode.title}</span>
                  {isCurrent && (
                    <span className="text-xs text-muted-foreground tabular-nums">
                      {Math.round(progress * 100)}%
                    </span>
                  )}
                </div>
                {isCurrent && (
                  <div className="ml-6 h-1 overflow-hidden rounded-full bg-muted">
                    <div
                      className="h-full rounded-full bg-primary"
                      style={{ width: `${Math.round(progress * 100)}%` }}
                    />
                  </div>
                )}
                {isFailed && (
                  <div className="ml-6 truncate text-xs text-destructive" title={failed[guid]}>
                    {failed[guid]}
                  </div>
                )}
              </div>
            );
          })}
        </div>

        {failedCount > 0 && (
          <Button variant="outline" className="self-end" onClick={() => void retry()}>
            Réessayer les échecs
          </Button>
        )}
      </DialogContent>
    </Dialog>
  );
}
