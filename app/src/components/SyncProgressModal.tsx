import { useEffect, useRef } from "react";
import { AnimatePresence, motion } from "motion/react";
import { Loader2, CheckCircle2, XCircle, Ban } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { useSync } from "@/store/sync";
import { formatDuration } from "@/lib/eta";

export function SyncProgressModal() {
  const {
    phase,
    log,
    stepFraction,
    running,
    modalOpen,
    setModalOpen,
    cancel,
    currentActivity,
    sendingDetail,
    etaSeconds,
  } = useSync();
  const eta = etaSeconds();
  const logRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    logRef.current?.scrollTo({ top: logRef.current.scrollHeight });
  }, [log]);

  let globalPct = 0;
  if (phase.kind === "preparing" && phase.total > 0) {
    globalPct = (phase.done / phase.total) * 100;
  } else if (phase.kind === "sending" && phase.bytesTotal > 0) {
    globalPct = (phase.bytesDone / phase.bytesTotal) * 100;
  } else if (phase.kind === "finished") {
    globalPct = 100;
  }

  const title =
    phase.kind === "finished"
      ? `Synchronisation terminée (${phase.count} épisode(s))`
      : phase.kind === "failed"
        ? "Échec de la synchronisation"
        : phase.kind === "cancelled"
          ? "Synchronisation annulée"
          : "Synchronisation en cours…";

  const icon =
    phase.kind === "finished" ? (
      <CheckCircle2 className="size-5 text-[var(--success)]" />
    ) : phase.kind === "failed" ? (
      <XCircle className="size-5 text-destructive" />
    ) : phase.kind === "cancelled" ? (
      <Ban className="size-5 text-muted-foreground" />
    ) : (
      <Loader2 className="size-5 animate-spin text-primary" />
    );

  return (
    <AnimatePresence>
      {modalOpen && (
        <>
          <motion.div
            className="fixed inset-0 z-50 bg-black/50 backdrop-blur-sm"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
          />
          <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
            <motion.div
              initial={{ opacity: 0, scale: 0.95, y: 8 }}
              animate={{ opacity: 1, scale: 1, y: 0 }}
              exit={{ opacity: 0, scale: 0.97 }}
              transition={{ type: "spring", stiffness: 320, damping: 26 }}
              className="w-full max-w-lg rounded-xl border bg-popover p-6 shadow-xl"
            >
              <div className="flex items-center gap-2.5">
                {icon}
                <h2 className="text-base font-semibold">{title}</h2>
              </div>

              <div className="mt-4 space-y-3">
                <div>
                  <div className="mb-1 flex justify-between text-sm">
                    <span className="truncate text-muted-foreground">{currentActivity()}</span>
                    {sendingDetail() && (
                      <span className="shrink-0 pl-2 text-xs text-muted-foreground tabular-nums">
                        {sendingDetail()}
                      </span>
                    )}
                  </div>
                  <Progress value={stepFraction * 100} />
                </div>
                <div>
                  <Progress value={globalPct} className="h-1.5 opacity-80" />
                  {phase.kind === "sending" && phase.bytesTotal > 0 && (
                    <div className="mt-1 flex justify-between text-xs text-muted-foreground tabular-nums">
                      <span>{Math.round(globalPct)} %</span>
                      <span>{eta === null ? "estimation…" : `${formatDuration(eta)} restantes`}</span>
                    </div>
                  )}
                </div>
              </div>

              {phase.kind === "failed" && (
                <p className="mt-3 rounded-md bg-destructive/10 px-3 py-2 text-sm text-destructive select-text">
                  {phase.message}
                </p>
              )}

              <div
                ref={logRef}
                className="mt-3 h-28 overflow-y-auto rounded-md bg-muted/50 p-2 font-mono text-xs text-muted-foreground select-text"
              >
                {log.slice(-80).map((line, i) => (
                  <div key={i}>{line}</div>
                ))}
              </div>

              <div className="mt-4 flex justify-end">
                {running ? (
                  <Button variant="outline" onClick={cancel}>
                    Annuler
                  </Button>
                ) : (
                  <Button onClick={() => setModalOpen(false)}>Fermer</Button>
                )}
              </div>
            </motion.div>
          </div>
        </>
      )}
    </AnimatePresence>
  );
}
