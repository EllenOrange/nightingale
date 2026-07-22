import { getLanguageName } from "@/lib/languages";

export function LanguageBadge({ language }: { language?: string | null }) {
  if (!language) return null;

  const shortCode = language.slice(0, 2).toUpperCase();

  return (
    <span
      className="inline-grid size-5 shrink-0 place-items-center rounded-sm bg-foreground/8 p-0 text-center font-mono text-[0.5625rem] leading-none font-semibold tracking-tight text-muted-foreground ring-1 ring-foreground/10 ring-inset"
      title={getLanguageName(language)}
      aria-label={`Language: ${getLanguageName(language)}`}
    >
      {shortCode}
    </span>
  );
}
