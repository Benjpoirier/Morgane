import { useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Stethoscope, Loader2, FileUp, CheckCircle2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { onIntegrityProgress, repairIntegrity } from "@/lib/ipc";
import { Progress } from "@/components/ui/progress";
import { formatDuration, freshEstimator, observe, remainingSeconds } from "@/lib/eta";
import { useConnection } from "@/store/connection";
import { useTree } from "@/store/tree";
import type { IntegrityProgressPayload, MissingFile } from "@/lib/types";

function missingKey(m: MissingFile): string {
  return m.type === "image" ? `image:${m.remoteName}` : `audio:${m.baseUuid}`;
}

function missingLabel(m: MissingFile): string {
  return m.type === "image" ? `Image ${m.remoteName}` : `Audio ${m.baseUuid}.mp3`;
}

export function IntegrityPane() {
  const { host, port } = useConnection();
  const issues = useTree((s) => s.integrityIssues);
  const checking = useTree((s) => s.checkingIntegrity);
  const storeError = useTree((s) => s.integrityError);
  const runCheckAction = useTree((s) => s.checkIntegrity);

  const [progress, setProgress] = useState<IntegrityProgressPayload | null>(null);
  const [rate, setRate] = useState<number | null>(null);
  const estimator = useRef(freshEstimator());

  useEffect(() => {
    const unlisten = onIntegrityProgress((p) => {

      const smoothed = observe(estimator.current, p.done, performance.now());
      if (smoothed !== null) setRate(smoothed);
      setProgress(p);
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  const [repairing, setRepairing] = useState(false);
  const [repairError, setRepairError] = useState<string | null>(null);
  const [fixes, setFixes] = useState<Record<string, { missing: MissingFile; path: string }>>({});

  const runCheck = async () => {
    setRepairError(null);
    setFixes({});
    estimator.current = freshEstimator();
    setRate(null);
    setProgress(null);
    await runCheckAction(host, port);
    setProgress(null);
  };

  const chooseFile = async (missing: MissingFile) => {
    const extensions =
      missing.type === "image"
        ? ["png", "jpg", "jpeg", "webp"]
        : ["mp3", "m4a", "aac", "wav", "ogg", "flac"];
    const selected = await open({ multiple: false, filters: [{ name: "Fichier", extensions }] });
    if (typeof selected === "string") {
      setFixes((f) => ({ ...f, [missingKey(missing)]: { missing, path: selected } }));
    }
  };

  const runRepair = async () => {
    const list = Object.values(fixes);
    if (list.length === 0) return;
    setRepairing(true);
    setRepairError(null);
    try {
      await repairIntegrity(host, port, list);
      await runCheck();
    } catch (e) {
      setRepairError(String(e));
    } finally {
      setRepairing(false);
    }
  };

  const allIssues = issues ?? [];
  const totalMissing = allIssues.reduce((n, i) => n + i.missingFiles.length, 0);
  const error = repairError ?? storeError;

  const remaining = progress
    ? remainingSeconds(rate, progress.done, progress.total)
    : null;

  return (
    <div className="p-4">
      <div className="flex items-center justify-between">
        <h1 className="text-lg font-semibold">Vérifier l'intégrité</h1>
        <div className="flex gap-2">
          {Object.keys(fixes).length > 0 && (
            <Button size="sm" disabled={repairing} onClick={runRepair}>
              {repairing ? <Loader2 className="animate-spin" /> : <FileUp />}
              Envoyer et vérifier
            </Button>
          )}
          <Button size="sm" variant="outline" disabled={checking} onClick={runCheck}>
            {checking ? <Loader2 className="animate-spin" /> : <Stethoscope />}
            {issues === null ? "Lancer la vérification" : "Relancer"}
          </Button>
        </div>
      </div>
      <p className="mt-1 text-sm text-muted-foreground">
        Fichiers référencés par le menu mais absents de la carte SD.
      </p>
      {error && <p className="mt-2 text-sm text-destructive">{error}</p>}

      {checking && progress !== null && progress.total > 0 && (
        <div className="mt-3">
          <Progress value={(progress.done / progress.total) * 100} />
          <div className="mt-1 flex justify-between text-xs text-muted-foreground tabular-nums">
            <span>
              {progress.done} / {progress.total} vérifications
            </span>
            <span>{remaining === null ? "estimation…" : `${formatDuration(remaining)} restantes`}</span>
          </div>
        </div>
      )}

      <div className="mt-4">
        {issues === null ? (
          <p className="py-6 text-center text-sm text-muted-foreground">
            Lance une vérification pour détecter les fichiers manquants.
          </p>
        ) : totalMissing === 0 ? (
          <div className="flex flex-col items-center gap-2 py-8 text-center">
            <CheckCircle2 className="size-8 text-[var(--success)]" />
            <p className="text-sm text-muted-foreground">Aucun fichier manquant. Tout est intègre.</p>
          </div>
        ) : (
          allIssues.map((issue) => (
            <div key={issue.uuid} className="mb-3 border-b pb-3 last:border-0">
              <div className="text-sm font-medium">{issue.title}</div>
              <div className="font-mono text-xs text-muted-foreground">{issue.uuid}</div>
              <div className="mt-1.5 space-y-1">
                {issue.missingFiles.map((m) => {
                  const fix = fixes[missingKey(m)];
                  return (
                    <div key={missingKey(m)} className="flex items-center gap-2 text-sm">
                      <span className="text-[var(--warning)]">⚠</span>
                      <span className="flex-1">{missingLabel(m)}</span>
                      {fix && (
                        <span className="max-w-40 truncate text-xs text-[var(--success)]">
                          {fix.path.split("/").pop()}
                        </span>
                      )}
                      <Button size="sm" variant="ghost" onClick={() => chooseFile(m)}>
                        Choisir un fichier…
                      </Button>
                    </div>
                  );
                })}
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
