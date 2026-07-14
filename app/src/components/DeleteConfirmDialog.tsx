import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { deleteSubscription } from "@/lib/ipc";
import { usePodcasts } from "@/store/podcasts";
import type { Subscription } from "@/lib/types";

export function DeleteConfirmDialog({
  sub,
  onClose,
  onDeleted,
}: {
  sub: Subscription | null;
  onClose: () => void;
  onDeleted: (feedUrl: string) => void;
}) {
  const reloadSubscriptions = usePodcasts((s) => s.reloadSubscriptions);

  const confirm = async () => {
    if (!sub) return;
    await deleteSubscription(sub.feedUrl);
    await reloadSubscriptions();
    onDeleted(sub.feedUrl);
    onClose();
  };

  return (
    <Dialog open={sub !== null} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle>Supprimer cet abonnement ?</DialogTitle>
          <DialogDescription className="break-all">
            {sub?.title || sub?.feedUrl}
          </DialogDescription>
        </DialogHeader>
        <p className="text-sm text-muted-foreground">
          Les épisodes déjà sur l'enceinte ne sont pas touchés.
        </p>
        <div className="flex justify-end gap-2">
          <Button variant="outline" onClick={onClose}>
            Annuler
          </Button>
          <Button variant="destructive" onClick={confirm}>
            Supprimer
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
