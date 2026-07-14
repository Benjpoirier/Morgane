import { useEffect } from "react";
import { Play, Pause, SkipBack, SkipForward, RotateCcw, RotateCw, Loader2, X } from "lucide-react";
import { Thumbnail } from "@/components/Thumbnail";
import { usePlayer } from "@/store/player";

function fmt(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) seconds = 0;
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}

export function PlayerBar() {
  const current = usePlayer((s) => s.current);
  const playing = usePlayer((s) => s.playing);
  const loading = usePlayer((s) => s.loading);
  const currentTime = usePlayer((s) => s.currentTime);
  const duration = usePlayer((s) => s.duration);
  const togglePlay = usePlayer((s) => s.togglePlay);
  const next = usePlayer((s) => s.next);
  const previous = usePlayer((s) => s.previous);
  const seek = usePlayer((s) => s.seek);
  const skip = usePlayer((s) => s.skip);
  const stop = usePlayer((s) => s.stop);

  useEffect(() => () => usePlayer.getState().stop(), []);

  if (!current) return null;

  return (
    <div className="flex items-center gap-4 border-t bg-card px-4 py-2">
      <div className="flex min-w-0 flex-1 items-center gap-3">
        <Thumbnail src={current.imageUrl} size={40} />
        <div className="min-w-0 truncate text-sm font-medium">{current.title}</div>
      </div>

      <div className="flex w-full max-w-xl flex-[2] flex-col items-center gap-1">
        <div className="flex items-center gap-1">
          <button onClick={previous} title="Précédent" className="rounded p-1.5 text-muted-foreground hover:bg-accent hover:text-foreground">
            <SkipBack className="size-4" />
          </button>
          <button onClick={() => skip(-15)} title="Reculer de 15 s" className="rounded p-1.5 text-muted-foreground hover:bg-accent hover:text-foreground">
            <RotateCcw className="size-4" />
          </button>
          <button
            onClick={togglePlay}
            title={playing ? "Pause" : "Lecture"}
            className="flex size-9 items-center justify-center rounded-full bg-primary text-primary-foreground hover:opacity-90"
          >
            {loading ? (
              <Loader2 className="size-4 animate-spin" />
            ) : playing ? (
              <Pause className="size-4" />
            ) : (
              <Play className="size-4" />
            )}
          </button>
          <button onClick={() => skip(15)} title="Avancer de 15 s" className="rounded p-1.5 text-muted-foreground hover:bg-accent hover:text-foreground">
            <RotateCw className="size-4" />
          </button>
          <button onClick={next} title="Suivant" className="rounded p-1.5 text-muted-foreground hover:bg-accent hover:text-foreground">
            <SkipForward className="size-4" />
          </button>
        </div>
        <div className="flex w-full items-center gap-2 text-xs tabular-nums text-muted-foreground">
          <span className="w-10 text-right">{fmt(currentTime)}</span>
          <input
            type="range"
            min={0}
            max={duration || 0}
            step={1}
            value={Math.min(currentTime, duration || 0)}
            onChange={(e) => seek(Number(e.target.value))}
            style={{ accentColor: "var(--primary)" }}
            className="h-1 flex-1 cursor-pointer"
          />
          <span className="w-10">{fmt(duration)}</span>
        </div>
      </div>

      <div className="flex flex-1 justify-end">
        <button onClick={stop} title="Arrêter" className="rounded p-1.5 text-muted-foreground hover:bg-accent hover:text-foreground">
          <X className="size-4" />
        </button>
      </div>
    </div>
  );
}
