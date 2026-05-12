import { useEffect, useState } from "react";

import { getClassicalExtendedNote } from "../../api/classical";
import type { ExtendedNote } from "../../types/classical";

/**
 * Phase 9 (F9.6 / D-044) — "About this work" section. The USP of the
 * Hub: 5 sub-sections of editorial prose per canon work. Loads the
 * `ExtendedNote` from the backend (v2 snapshot) on mount; renders a
 * markdown-light body with optional collapsible sections.
 *
 * Markdown-light means:
 *   • `_text_` and `*text*` → italic.
 *   • `**text**` → bold.
 *   • `[label](url)` → external link.
 *   • blank lines split paragraphs.
 *   • everything else → plain text.
 *
 * No HTML parsing, no script execution. The escape strategy is: render
 * children as text by default, only emit a span/em/strong/anchor when
 * we recognise a known token. URLs are forced through `noopener
 * noreferrer` and `target=_blank`.
 *
 * Locale: defaults to "en". A future locale switch can be threaded
 * down from a settings hook; for V1 we ship "en" and "es" via the
 * snapshot.
 */

interface AboutThisWorkProps {
  workMbid: string;
  /** Phase 5 fallback — when no extended note exists, the small
   *  editor_note from the v1 snapshot still renders inside this
   *  section if provided. Keeps Phase 5 behaviour intact for works
   *  outside the v2 set. */
  fallbackEditorNote?: string;
  /** Phase 1 fallback — Wikipedia summary; rendered after the
   *  editor_note when neither extended nor editor_note carries the
   *  weight of the section. */
  fallbackWikipediaSummary?: string;
  fallbackWikipediaUrl?: string;
  locale?: string;
}

const SECTION_ORDER: Array<{
  key: keyof ExtendedNote["body"];
  labelEn: string;
  labelEs: string;
  collapsedByDefault: boolean;
}> = [
  {
    key: "origin",
    labelEn: "Origin & commission",
    labelEs: "Origen y encargo",
    collapsedByDefault: false,
  },
  {
    key: "premiere",
    labelEn: "Premiere & reception",
    labelEs: "Estreno y recepción",
    collapsedByDefault: true,
  },
  {
    key: "highlights",
    labelEn: "Musical highlights",
    labelEs: "Aspectos musicales",
    collapsedByDefault: false,
  },
  {
    key: "context",
    labelEn: "Historical context",
    labelEs: "Contexto histórico",
    collapsedByDefault: true,
  },
  {
    key: "notableRecordingsEssay",
    labelEn: "Notable recordings",
    labelEs: "Grabaciones de referencia",
    collapsedByDefault: true,
  },
];

export default function AboutThisWork({
  workMbid,
  fallbackEditorNote,
  fallbackWikipediaSummary,
  fallbackWikipediaUrl,
  locale,
}: AboutThisWorkProps) {
  const [note, setNote] = useState<ExtendedNote | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setNote(null);
    getClassicalExtendedNote(workMbid, locale)
      .then((n) => {
        if (cancelled) {
          return;
        }
        setNote(n);
        setLoading(false);
      })
      .catch((e: unknown) => {
        console.warn("[classical] extended note fetch failed:", e);
        if (cancelled) {
          return;
        }
        setNote(null);
        setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [workMbid, locale]);

  // Loading: render a thin skeleton instead of nothing so layout
  // doesn't jump when the note arrives.
  if (loading) {
    return (
      <section className="space-y-3">
        <div className="h-5 w-48 animate-pulse rounded bg-th-surface/50" />
        <div className="h-3 w-3/4 animate-pulse rounded bg-th-surface/40" />
        <div className="h-3 w-2/3 animate-pulse rounded bg-th-surface/40" />
      </section>
    );
  }

  // No extended note → fallback to Phase 5 + Phase 1.
  if (!note) {
    if (!fallbackEditorNote && !fallbackWikipediaSummary) {
      return null;
    }
    return (
      <section aria-label="About this work">
        <h2 className="mb-3 text-[18px] font-bold tracking-tight text-th-text-primary">
          About this work
        </h2>
        {fallbackEditorNote && (
          <p className="mb-4 rounded-lg border border-th-accent/30 bg-th-accent/5 px-3 py-2 text-[14px] italic leading-relaxed text-th-text-primary/85">
            {fallbackEditorNote}
          </p>
        )}
        {fallbackWikipediaSummary && (
          <p className="whitespace-pre-line text-[14px] leading-relaxed text-th-text-primary/85">
            {fallbackWikipediaSummary}
          </p>
        )}
        {fallbackWikipediaUrl && (
          <p className="mt-3 text-[11px] text-th-text-secondary/70">
            From{" "}
            <a
              href={fallbackWikipediaUrl}
              target="_blank"
              rel="noopener noreferrer"
              className="underline hover:text-th-text-primary"
            >
              Wikipedia
            </a>{" "}
            · CC BY-SA
          </p>
        )}
      </section>
    );
  }

  const isEs = note.language === "es";
  return (
    <section aria-label="About this work">
      <h2 className="mb-1 text-[18px] font-bold tracking-tight text-th-text-primary">
        {isEs ? "Sobre esta obra" : "About this work"}
      </h2>
      <p className="mb-5 text-[11px] uppercase tracking-widest text-th-text-secondary">
        {isEs ? "Editorial · mySone" : "Editorial · mySone"}
      </p>

      <div className="space-y-4">
        {SECTION_ORDER.map(({ key, labelEn, labelEs, collapsedByDefault }) => {
          const text = note.body[key];
          if (!text) {
            return null;
          }
          return (
            <SubSection
              key={key as string}
              label={isEs ? labelEs : labelEn}
              text={text}
              collapsedByDefault={collapsedByDefault}
            />
          );
        })}
      </div>

      {note.sources.length > 0 && (
        <p className="mt-6 text-[11px] text-th-text-secondary/70">
          {isEs ? "Fuentes" : "Sources"}:{" "}
          {note.sources.map((s, idx) => (
            <span key={idx}>
              {idx > 0 && " · "}
              {renderSourceCitation(s)}
            </span>
          ))}
        </p>
      )}
    </section>
  );
}

interface SubSectionProps {
  label: string;
  text: string;
  collapsedByDefault: boolean;
}

function SubSection({ label, text, collapsedByDefault }: SubSectionProps) {
  const [open, setOpen] = useState(!collapsedByDefault);
  return (
    <div className="rounded-lg border border-th-border-subtle/50 bg-th-elevated/40 px-4 py-3">
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className="flex w-full items-center justify-between text-[14px] font-semibold text-th-text-primary"
        aria-expanded={open}
      >
        <span>{label}</span>
        <span className="text-[11px] text-th-text-muted">
          {open ? "−" : "+"}
        </span>
      </button>
      {open && (
        <div className="mt-3 text-[14px] leading-relaxed text-th-text-primary/90">
          {renderMarkdownLight(text)}
        </div>
      )}
    </div>
  );
}

/** Markdown-light renderer. Splits on blank lines for paragraphs;
 *  each paragraph is tokenised inline for italic / bold / link. */
function renderMarkdownLight(text: string) {
  const paragraphs = text.split(/\n\s*\n/);
  return paragraphs.map((p, idx) => (
    <p key={idx} className={idx > 0 ? "mt-3" : ""}>
      {renderInlineTokens(p)}
    </p>
  ));
}

const INLINE_TOKEN_REGEX = /(\*\*[^*]+\*\*|_[^_]+_|\*[^*]+\*|\[[^\]]+\]\([^)]+\))/g;

function renderInlineTokens(text: string) {
  // Split keeps the matched delimiters in the array.
  const parts = text.split(INLINE_TOKEN_REGEX).filter(Boolean);
  return parts.map((part, idx) => {
    if (part.startsWith("**") && part.endsWith("**")) {
      return (
        <strong key={idx}>{part.slice(2, -2)}</strong>
      );
    }
    if (part.startsWith("_") && part.endsWith("_") && part.length >= 3) {
      return <em key={idx}>{part.slice(1, -1)}</em>;
    }
    if (part.startsWith("*") && part.endsWith("*") && part.length >= 3) {
      return <em key={idx}>{part.slice(1, -1)}</em>;
    }
    const linkMatch = part.match(/^\[([^\]]+)\]\(([^)]+)\)$/);
    if (linkMatch) {
      const label = linkMatch[1];
      const url = linkMatch[2];
      // Only follow http(s) URLs.
      if (!/^https?:\/\//i.test(url)) {
        return <span key={idx}>{label}</span>;
      }
      return (
        <a
          key={idx}
          href={url}
          target="_blank"
          rel="noopener noreferrer"
          className="underline hover:text-th-accent"
        >
          {label}
        </a>
      );
    }
    return <span key={idx}>{part}</span>;
  });
}

function renderSourceCitation(s: import("../../types/classical").ExtendedSource) {
  if (s.kind === "wikipedia" && s.url) {
    return (
      <a
        href={s.url}
        target="_blank"
        rel="noopener noreferrer"
        className="underline hover:text-th-text-primary"
      >
        Wikipedia
      </a>
    );
  }
  if (s.kind === "wikidata" && s.qid) {
    return (
      <a
        href={`https://www.wikidata.org/wiki/${s.qid}`}
        target="_blank"
        rel="noopener noreferrer"
        className="underline hover:text-th-text-primary"
      >
        Wikidata · {s.qid}
      </a>
    );
  }
  if (s.kind === "editor") {
    return <span>{s.name ?? "mySone team"}</span>;
  }
  return <span>{s.name ?? s.kind}</span>;
}
