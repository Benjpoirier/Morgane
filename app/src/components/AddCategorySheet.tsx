import { useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { useTree } from "@/store/tree";
import { usePodcasts } from "@/store/podcasts";
import { pickImage } from "@/lib/pickImage";
import { utf8Len, MAX_TITLE_BYTES } from "@/lib/text";

export function AddCategorySheet({
  open: isOpen,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (v: boolean) => void;
}) {
  const addCategory = useTree((s) => s.addCategory);
  const reloadSyncState = usePodcasts((s) => s.reloadSyncState);
  const [title, setTitle] = useState("");
  const [image, setImage] = useState("");

  const tooLong = utf8Len(title) > MAX_TITLE_BYTES;
  const canCreate = title.trim().length > 0 && image.trim().length > 0 && !tooLong;

  const browse = async () => {
    const path = await pickImage();
    if (path) setImage(path);
  };

  const create = async () => {
    if (!canCreate) return;
    await addCategory(title.trim(), image.trim());
    await reloadSyncState();
    setTitle("");
    setImage("");
    onOpenChange(false);
  };

  return (
    <Dialog open={isOpen} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Nouvelle catégorie</DialogTitle>
        </DialogHeader>
        <div className="flex flex-col gap-3">
          <div>
            <Input
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder="Titre de la catégorie"
            />
            {tooLong && (
              <p className="mt-1 text-xs text-[var(--warning)]">
                Trop long : {utf8Len(title)} octets (max {MAX_TITLE_BYTES}).
              </p>
            )}
          </div>
          <div className="flex gap-2">
            <Input
              value={image}
              onChange={(e) => setImage(e.target.value)}
              placeholder="URL d'image ou chemin local"
              className="flex-1"
            />
            <Button variant="outline" onClick={browse}>
              Parcourir…
            </Button>
          </div>
          <Button className="self-end" disabled={!canCreate} onClick={create}>
            Créer
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
