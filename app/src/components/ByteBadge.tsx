import { utf8Len, MAX_TITLE_BYTES } from "@/lib/text";

export function ByteBadge({ value, limit = MAX_TITLE_BYTES }: { value: string; limit?: number }) {
  if (utf8Len(value) <= limit) return null;
  return (
    <span
      className="shrink-0 rounded bg-[var(--warning)]/20 px-1 text-[10px] font-semibold text-[var(--warning)]"
      title={`${utf8Len(value)} octets — l'enceinte tronque au-delà de ${limit}`}
    >
      {limit}+
    </span>
  );
}
