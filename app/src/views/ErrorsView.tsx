import { AnimatePresence, motion } from "motion/react";
import { Button } from "@/components/ui/button";
import { useErrors } from "@/store/errors";

function formatTime(ts: number): string {
  return new Date(ts).toLocaleTimeString("fr-FR");
}

export function ErrorsView() {
  const { entries, clear } = useErrors();

  return (
    <div className="flex h-full flex-col">
      <header className="flex items-center justify-between border-b px-6 py-3">
        <h1 className="text-lg font-semibold">Erreurs</h1>
        <Button variant="outline" size="sm" onClick={clear} disabled={!entries.length}>
          Vider
        </Button>
      </header>
      {entries.length === 0 ? (
        <div className="flex flex-1 items-center justify-center text-sm text-muted-foreground">
          Aucune erreur cette session.
        </div>
      ) : (
        <div className="flex-1 overflow-y-auto px-6 py-3">
          <AnimatePresence initial={false}>
            {entries.map((e) => (
              <motion.div
                key={e.id}
                layout
                initial={{ opacity: 0, y: -6 }}
                animate={{ opacity: 1, y: 0 }}
                className="border-b py-2.5 last:border-0"
              >
                <div className="flex items-baseline gap-2">
                  <span className="font-semibold text-destructive">{e.context}</span>
                  <span className="text-xs text-muted-foreground tabular-nums">
                    {formatTime(e.timestamp)}
                  </span>
                </div>
                <div className="mt-0.5 text-sm text-foreground/90 select-text">
                  {e.message}
                </div>
              </motion.div>
            ))}
          </AnimatePresence>
        </div>
      )}
    </div>
  );
}
