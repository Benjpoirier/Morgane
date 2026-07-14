import { useEffect, useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { useConnection } from "@/store/connection";
import { useDevices } from "@/store/devices";

export function NameDeviceDialog() {
  const pendingNameMac = useConnection((s) => s.pendingNameMac);
  const clearPendingName = useConnection((s) => s.clearPendingName);
  const rename = useDevices((s) => s.rename);
  const [name, setName] = useState("");

  useEffect(() => setName(""), [pendingNameMac]);

  if (!pendingNameMac) return null;

  const save = async () => {
    const next = name.trim();
    if (next) await rename(pendingNameMac, next);
    clearPendingName();
  };

  return (
    <Dialog open onOpenChange={(v) => { if (!v) clearPendingName(); }}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle>Nommer cette Merlin</DialogTitle>
        </DialogHeader>
        <p className="text-sm text-muted-foreground">
          Donne-lui un nom reconnaissable — tu la retrouveras hors connexion pour préparer ses
          podcasts.
        </p>
        <Input
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Ex. Chambre de Léa, Salon…"
          autoFocus
          onKeyDown={(e) => { if (e.key === "Enter") void save(); }}
        />
        <div className="flex justify-end gap-2">
          <Button variant="ghost" onClick={clearPendingName}>
            Plus tard
          </Button>
          <Button onClick={save} disabled={!name.trim()}>
            Enregistrer
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
