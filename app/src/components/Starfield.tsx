import { useMemo } from "react";

export function Starfield({ count = 70 }: { count?: number }) {
  const stars = useMemo(() => {
    let seed = 7;
    const rnd = () => {
      seed = (seed * 1103515245 + 12345) & 0x7fffffff;
      return seed / 0x7fffffff;
    };
    return Array.from({ length: count }, (_, i) => {
      const size = (rnd() * 2 + 1).toFixed(1);
      return {
        key: i,
        left: (rnd() * 100).toFixed(2) + "%",
        top: (rnd() * 100).toFixed(2) + "%",
        size,
        gold: rnd() > 0.85,
        opacity: Number((rnd() * 0.6 + 0.2).toFixed(2)),
        duration: (rnd() * 3 + 2).toFixed(1),
        delay: (rnd() * 3).toFixed(1),
      };
    });
  }, [count]);

  return (
    <div className="pointer-events-none absolute inset-0 overflow-hidden">
      {stars.map((s) => (
        <span
          key={s.key}
          style={{
            position: "absolute",
            left: s.left,
            top: s.top,
            width: `${s.size}px`,
            height: `${s.size}px`,
            borderRadius: "50%",
            background: s.gold ? "oklch(0.9 0.09 85)" : "#fff",
            opacity: s.opacity,
            animation: `n-twinkle ${s.duration}s ease-in-out ${s.delay}s infinite`,
          }}
        />
      ))}
    </div>
  );
}
