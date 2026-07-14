import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import type { ImageOverride } from "@/hooks/useImageOverride";

export function ImageOverrideEditor({ image }: { image: ImageOverride }) {
  return (
    <div className="flex gap-2">
      <Input
        autoFocus
        value={image.draft ?? ""}
        onChange={(e) => image.setDraft(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") void image.commitDraft();
          if (e.key === "Escape") image.cancel();
        }}
        placeholder="URL ou /chemin.jpg"
        className="h-7"
      />
      <Button size="sm" variant="outline" onClick={image.browse}>
        Parcourir…
      </Button>
    </div>
  );
}
