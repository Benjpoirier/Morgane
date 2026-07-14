import { Search, Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { useTree } from "@/store/tree";
import { useConnection } from "@/store/connection";

const ORPHAN_ROOT_UUID = "merlinsync-fichiers-retrouves";

export function OrphanPane() {
  const { host, port } = useConnection();
  const folders = useTree((s) => s.folders);
  const pendingOrphanDeletions = useTree((s) => s.pendingOrphanDeletions);
  const searching = useTree((s) => s.searchingOrphans);
  const searchOrphans = useTree((s) => s.searchOrphans);
  const toggleOrphan = useTree((s) => s.toggleOrphan);
  const toggleAllOrphans = useTree((s) => s.toggleAllOrphans);

  const orphanFolder = folders.find((f) => f.uuid === ORPHAN_ROOT_UUID);
  const orphans = orphanFolder?.children ?? [];
  const marked = new Set(pendingOrphanDeletions);

  return (
    <div className="p-4">
      <div className="flex items-center justify-between">
        <h1 className="text-lg font-semibold">Fichiers retrouvés</h1>
        <div className="flex gap-2">
          {orphans.length > 0 && (
            <Button size="sm" variant="ghost" onClick={toggleAllOrphans}>
              Tout marquer / annuler
            </Button>
          )}
          <Button size="sm" variant="outline" disabled={searching} onClick={() => searchOrphans(host, port)}>
            {searching ? <Loader2 className="animate-spin" /> : <Search />}
            Rechercher
          </Button>
        </div>
      </div>
      <p className="mt-1 text-sm text-muted-foreground">
        Fichiers présents sur la carte SD mais absents du menu de l'enceinte
        (audios, images, divers) — les supprimer libère de l'espace. Les fichiers
        système et les identifiants Wi-Fi ne sont jamais listés.
      </p>

      <div className="mt-4">
        {orphans.length === 0 ? (
          <p className="py-6 text-center text-sm text-muted-foreground">
            {orphanFolder
              ? "Aucun fichier orphelin trouvé."
              : "Lance une recherche pour détecter les fichiers orphelins."}
          </p>
        ) : (
          orphans.map((node) => (
            <label
              key={node.uuid}
              className="flex items-center gap-2.5 rounded-md px-2 py-1.5 hover:bg-accent"
            >
              <Checkbox
                checked={marked.has(node.uuid)}
                onCheckedChange={() => toggleOrphan(node.uuid)}
              />
              <span className="min-w-0 flex-1 truncate font-mono text-sm">{node.uuid}</span>
              {marked.has(node.uuid) && (
                <span className="text-xs text-[var(--warning)]">
                  supprimé au prochain sync
                </span>
              )}
            </label>
          ))
        )}
      </div>
    </div>
  );
}
