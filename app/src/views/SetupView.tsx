import { useEffect } from "react";
import { Logo } from "@/components/Logo";
import { Progress } from "@/components/ui/progress";
import { Button } from "@/components/ui/button";
import { useSetup } from "@/store/setup";

const PHASE_LABEL: Record<string, string> = {
  downloading: "Téléchargement du moteur audio…",
  verifying: "Vérification de l'intégrité…",
  extracting: "Installation…",
  done: "Prêt",
};

function mb(n: number) {
  return (n / (1024 * 1024)).toFixed(1);
}

export function SetupView() {
  const progress = useSetup((s) => s.progress);
  const error = useSetup((s) => s.error);
  const downloading = useSetup((s) => s.downloading);
  const download = useSetup((s) => s.download);

  useEffect(() => {
    download();
  }, [download]);

  const pct =
    progress && progress.totalBytes > 0
      ? Math.round((progress.bytes / progress.totalBytes) * 100)
      : 0;
  const unsupported = !!error && error.toLowerCase().includes("plateforme");

  return (
    <div className="flex h-full w-full flex-col items-center justify-center gap-6 p-8 text-center select-none">
      <Logo className="size-16 text-brand" />
      <div className="space-y-1">
        <h1 className="font-brand-serif text-3xl">Préparation de Morgane</h1>
        <p className="text-sm text-muted-foreground">
          Installation du moteur audio (FFmpeg), une seule fois.
        </p>
      </div>

      {error ? (
        <div className="flex max-w-sm flex-col items-center gap-3">
          <p className="text-sm text-destructive">{error}</p>
          {!unsupported && (
            <Button onClick={() => download()} disabled={downloading}>
              Réessayer
            </Button>
          )}
          <p className="text-xs text-muted-foreground">
            Tu peux aussi installer ffmpeg toi-même (par ex. « brew install
            ffmpeg ») puis relancer Morgane.
          </p>
        </div>
      ) : (
        <div className="flex w-full max-w-sm flex-col gap-2">
          <Progress value={pct} />
          <div className="flex justify-between text-xs text-muted-foreground tabular-nums">
            <span>{PHASE_LABEL[progress?.phase ?? "downloading"]}</span>
            {progress && progress.totalBytes > 0 && (
              <span>
                {mb(progress.bytes)} / {mb(progress.totalBytes)} Mo
              </span>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
