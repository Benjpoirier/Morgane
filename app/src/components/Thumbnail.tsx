import { useState, useEffect } from "react";
import { Mic } from "lucide-react";
import { cn } from "@/lib/utils";

export function Thumbnail({
  src,
  size,
  className,
}: {
  src: string | null;
  size: number;
  className?: string;
}) {
  const [failed, setFailed] = useState(false);
  useEffect(() => setFailed(false), [src]);

  const style = { width: size, height: size };
  if (!src || failed) {
    return (
      <div
        style={style}
        className={cn(
          "flex shrink-0 items-center justify-center rounded-md bg-muted text-muted-foreground",
          className,
        )}
      >
        <Mic style={{ width: size * 0.5, height: size * 0.5 }} strokeWidth={1.5} />
      </div>
    );
  }
  return (
    <img
      src={src}
      style={style}
      onError={() => setFailed(true)}
      className={cn("shrink-0 rounded-md object-cover", className)}
      draggable={false}
    />
  );
}
