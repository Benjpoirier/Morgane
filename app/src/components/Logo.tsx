export function Logo({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 512 512"
      fill="none"
      className={className}
      aria-hidden="true"
    >
      <path
        d="M140 171 Q140 146 165 146 Q185 146 200 158 L256 210 L312 158
           Q327 146 347 146 Q372 146 372 171 L372 330 Q372 352 347 352
           Q272 352 256 367 Q240 352 165 352 Q140 352 140 330 Z"
        stroke="currentColor"
        strokeWidth={37}
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
