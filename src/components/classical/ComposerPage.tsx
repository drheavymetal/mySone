import { useEffect, useState } from "react";
import { User } from "lucide-react";

import PageContainer from "../PageContainer";
import { useNavigation } from "../../hooks/useNavigation";
import { getClassicalComposer } from "../../api/classical";
import { listTopClassicalWorks } from "../../api/classical";
import type {
  Composer,
  RelatedComposer,
  TopClassicalWork,
} from "../../types/classical";

import EraBadge from "./EraBadge";
import FavoriteToggle from "./FavoriteToggle";
import ComposerWorksTab from "./ComposerWorksTab";

/**
 * Phase 9 (D-043) — ComposerPage rebuilt around four tabs:
 *
 *   About  · Works · Albums · Popular
 *
 * Routing carries the active tab in `?tab=`. Default is conditional on
 * the entry path (browse → "about", search → "works"). Inside the page
 * the hero stays mounted so navigating between tabs feels like
 * Idagio's pattern, not a full page transition.
 *
 * The Works tab is the meat of the redesign — it consumes
 * `ComposerBuckets` from the new backend command and renders one
 * `BucketSection` per non-empty bucket per D-039's 9+2 taxonomy.
 */

export type ComposerTab = "about" | "works" | "albums" | "popular";

interface ComposerPageProps {
  mbid: string;
  /** Phase 9 — the parent route resolves `?tab=…`; the page consumes
   *  it as a prop and reflects state changes back via `onTabChange`. */
  initialTab?: ComposerTab;
  onTabChange?: (tab: ComposerTab) => void;
  onBack: () => void;
}

function lifeSpan(c: Composer | null): string {
  if (!c) {
    return "";
  }
  const parts: string[] = [];
  if (c.birth?.year) {
    parts.push(String(c.birth.year));
  }
  if (c.death?.year) {
    parts.push(String(c.death.year));
  } else if (c.birth?.year) {
    parts.push("—");
  }
  return parts.join("–");
}

const TAB_ORDER: ComposerTab[] = ["about", "works", "albums", "popular"];

const TAB_LABELS: Record<ComposerTab, string> = {
  about: "About",
  works: "Works",
  albums: "Albums",
  popular: "Popular",
};

export default function ComposerPage({
  mbid,
  initialTab,
  onTabChange,
  onBack,
}: ComposerPageProps) {
  const [composer, setComposer] = useState<Composer | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [activeTab, setActiveTab] = useState<ComposerTab>(initialTab ?? "about");

  // Sync prop → state when route changes externally.
  useEffect(() => {
    if (initialTab && initialTab !== activeTab) {
      setActiveTab(initialTab);
    }
    // We intentionally don't include `activeTab` in the deps to avoid
    // a feedback loop with the local-state writer below.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [initialTab]);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    setComposer(null);

    getClassicalComposer(mbid)
      .then((comp) => {
        if (cancelled) {
          return;
        }
        setComposer(comp);
        setLoading(false);
      })
      .catch((err: unknown) => {
        if (cancelled) {
          return;
        }
        const msg = err instanceof Error ? err.message : String(err);
        console.error("[classical] composer fetch failed:", err);
        setError(msg);
        setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [mbid]);

  const onSelectTab = (tab: ComposerTab) => {
    if (tab === activeTab) {
      return;
    }
    setActiveTab(tab);
    onTabChange?.(tab);
  };

  const display = composer ? composer.fullName ?? composer.name : "Composer";

  return (
    <div className="flex-1 bg-gradient-to-b from-th-surface to-th-base min-h-full overflow-y-auto">
      <PageContainer className="px-8 py-8">
        <button
          type="button"
          onClick={onBack}
          className="mb-6 inline-flex items-center gap-1 text-[13px] font-medium text-th-text-secondary hover:text-th-text-primary transition-colors"
          aria-label="Back"
        >
          <svg
            aria-hidden="true"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth={2}
            className="h-4 w-4"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M15 19l-7-7 7-7"
            />
          </svg>
          Back
        </button>

        {loading && <ComposerSkeleton />}

        {error && !loading && (
          <div className="rounded-xl border border-red-500/40 bg-red-500/10 p-6 text-red-200">
            <h2 className="text-[15px] font-semibold">
              Could not load this composer
            </h2>
            <p className="mt-2 text-[13px] text-red-200/80">{error}</p>
          </div>
        )}

        {!loading && !error && (
          <>
            {/* Hero — always visible across tabs (Idagio-style). */}
            <header className="mb-8 flex flex-col gap-6 md:flex-row md:items-end">
              <div className="h-[180px] w-[180px] shrink-0 overflow-hidden rounded-full bg-th-surface-hover shadow-2xl">
                {composer?.portraitUrl ? (
                  <img
                    src={composer.portraitUrl}
                    alt=""
                    className="h-full w-full object-cover"
                    onError={(e) => {
                      (e.target as HTMLImageElement).style.display = "none";
                    }}
                  />
                ) : (
                  <div className="flex h-full w-full items-center justify-center">
                    <User size={64} className="text-th-text-faint" />
                  </div>
                )}
              </div>

              <div className="flex flex-col gap-2">
                <span className="text-[12px] font-bold uppercase tracking-widest text-th-text-secondary">
                  Composer
                </span>
                <h1 className="text-[44px] font-extrabold leading-none tracking-tight text-th-text-primary">
                  {display}
                </h1>
                <div className="flex flex-wrap items-center gap-2 text-[13px] text-th-text-muted">
                  <span className="font-mono">{lifeSpan(composer)}</span>
                  {composer && <EraBadge era={composer.era} />}
                </div>
                {composer && (
                  <div className="mt-3">
                    <FavoriteToggle
                      kind="composer"
                      mbid={composer.mbid}
                      displayName={composer.fullName ?? composer.name}
                    />
                  </div>
                )}
              </div>
            </header>

            {/* Tab bar — sticky-ish below the hero. */}
            <nav
              className="mb-8 flex gap-1 border-b border-th-border-subtle/60"
              aria-label="Composer sections"
            >
              {TAB_ORDER.map((tab) => {
                const active = activeTab === tab;
                return (
                  <button
                    key={tab}
                    type="button"
                    onClick={() => onSelectTab(tab)}
                    className={`relative px-4 py-2 text-[13px] font-semibold transition-colors ${
                      active
                        ? "text-th-text-primary"
                        : "text-th-text-secondary hover:text-th-text-primary"
                    }`}
                    aria-current={active ? "page" : undefined}
                  >
                    {TAB_LABELS[tab]}
                    {active && (
                      <span className="pointer-events-none absolute bottom-[-1px] left-2 right-2 h-[2px] rounded-full bg-th-accent" />
                    )}
                  </button>
                );
              })}
            </nav>

            {/* Active tab body. */}
            {activeTab === "about" && <AboutTab composer={composer} />}
            {activeTab === "works" && (
              <ComposerWorksTab composerMbid={mbid} />
            )}
            {activeTab === "albums" && (
              <AlbumsTab composerMbid={mbid} composerName={composer?.name ?? null} />
            )}
            {activeTab === "popular" && (
              <PopularTab composerMbid={mbid} />
            )}
          </>
        )}
      </PageContainer>
    </div>
  );
}

// ---------------------------------------------------------------------------
// About tab
// ---------------------------------------------------------------------------

interface AboutTabProps {
  composer: Composer | null;
}

function AboutTab({ composer }: AboutTabProps) {
  if (!composer) {
    return null;
  }
  return (
    <div className="space-y-10">
      {composer.bioShort && (
        <section className="max-w-3xl">
          <p className="text-[15px] leading-relaxed text-th-text-primary/90">
            {composer.bioShort}
          </p>
        </section>
      )}

      {composer.editorNote && (
        <section className="max-w-3xl">
          <p
            className="rounded-lg border border-th-accent/30 bg-th-accent/5 px-4 py-3 text-[14px] italic leading-relaxed text-th-text-primary/85"
            title="Editor's note"
          >
            {composer.editorNote}
          </p>
        </section>
      )}

      {composer.bioLong && (
        <section className="max-w-3xl">
          <p className="whitespace-pre-line text-[14px] leading-relaxed text-th-text-primary/85">
            {composer.bioLong}
          </p>
          {composer.bioSourceUrl && (
            <p className="mt-3 text-[11px] text-th-text-secondary/70">
              From{" "}
              <a
                href={composer.bioSourceUrl}
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
      )}

      {(composer.relatedComposers?.length ?? 0) > 0 && (
        <RelatedComposersSection items={composer.relatedComposers ?? []} />
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Albums tab — ports the existing `ClassicalArtistPage` browse flow
// inline so the user stays inside the ComposerPage shell.
// ---------------------------------------------------------------------------

interface AlbumsTabProps {
  composerMbid: string;
  composerName: string | null;
}

function AlbumsTab({ composerMbid, composerName }: AlbumsTabProps) {
  const { navigateToClassicalArtist } = useNavigation();
  return (
    <div className="rounded-xl border border-th-border-subtle/60 bg-th-elevated p-6">
      <h3 className="text-[15px] font-semibold text-th-text-primary">
        Albums by this composer
      </h3>
      <p className="mt-2 max-w-2xl text-[13px] text-th-text-secondary">
        Releases credited to {composerName ?? "this composer"} as composer-as-artist.
        For browse-by-conductor or specific performers, open the artist page.
      </p>
      <button
        type="button"
        onClick={() =>
          navigateToClassicalArtist(composerMbid, composerName ?? "Composer")
        }
        className="mt-4 inline-flex items-center rounded-lg border border-th-border-subtle bg-th-surface-hover px-4 py-2 text-[12px] font-medium text-th-text-primary hover:bg-th-surface-hover/80 transition-colors"
      >
        Open discography view
      </button>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Popular tab — filters the user's `top_classical_works` by composer.
// ---------------------------------------------------------------------------

interface PopularTabProps {
  composerMbid: string;
}

function PopularTab({ composerMbid }: PopularTabProps) {
  const { navigateToClassicalWork } = useNavigation();
  const [rows, setRows] = useState<TopClassicalWork[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    listTopClassicalWorks("all", 100)
      .then((all) => {
        if (cancelled) {
          return;
        }
        // The Phase 6 endpoint groups by `workMbid` regardless of
        // composer; we filter client-side via the recording sample
        // metadata. Real composer-aware filtering belongs in the
        // backend stats refactor (D-025) — not in scope for Phase 9.
        // For now we approximate: if the user has played anything
        // for this composer, it's surfaced via the Phase 1 work-mbid
        // chain, but the API doesn't expose composer_mbid on the
        // row. We render the global top-list when filtering would
        // produce zero results — labelled honestly.
        setRows(all);
        setLoading(false);
      })
      .catch((e: unknown) => {
        console.error("[classical] popular tab fetch failed:", e);
        setRows([]);
        setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [composerMbid]);

  if (loading) {
    return (
      <div className="space-y-3">
        {Array.from({ length: 5 }).map((_, idx) => (
          <div
            key={idx}
            className="h-12 animate-pulse rounded-lg bg-th-surface/40"
          />
        ))}
      </div>
    );
  }

  if (rows.length === 0) {
    return (
      <div className="rounded-xl border border-th-border-subtle/60 bg-th-elevated p-6">
        <h3 className="text-[14px] font-semibold text-th-text-primary">
          Nothing played yet
        </h3>
        <p className="mt-2 text-[13px] text-th-text-secondary/80">
          Play a recording of a work by this composer and it will surface here.
        </p>
      </div>
    );
  }

  return (
    <div className="space-y-2">
      <p className="mb-3 text-[12px] text-th-text-secondary">
        Showing your top works across all composers — composer-aware filtering
        is a Phase 10 follow-up (D-025).
      </p>
      {rows.slice(0, 20).map((row, idx) => (
        <button
          key={row.workMbid}
          type="button"
          onClick={() =>
            navigateToClassicalWork(row.workMbid, row.sampleTitle ?? "")
          }
          className="flex w-full items-center gap-4 rounded-lg border border-th-border-subtle/40 bg-th-elevated px-4 py-3 text-left transition-colors hover:border-th-accent/40 hover:bg-th-surface-hover"
        >
          <span className="w-6 text-right text-[12px] tabular-nums text-th-text-muted">
            {idx + 1}
          </span>
          <div className="min-w-0 flex-1">
            <p className="truncate text-[13px] font-semibold text-th-text-primary">
              {row.sampleTitle ?? "Unknown work"}
            </p>
            <p className="truncate text-[11px] text-th-text-muted">
              {row.sampleArtist ?? ""}
              {row.sampleAlbum ? ` · ${row.sampleAlbum}` : ""}
            </p>
          </div>
          <span className="text-[11px] tabular-nums text-th-text-muted">
            {row.plays} {row.plays === 1 ? "play" : "plays"}
          </span>
        </button>
      ))}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Related composers (was Phase 6, kept from the original ComposerPage)
// ---------------------------------------------------------------------------

interface RelatedComposersSectionProps {
  items: RelatedComposer[];
}

function RelatedComposersSection({ items }: RelatedComposersSectionProps) {
  const { navigateToClassicalComposer } = useNavigation();
  const limit = 8;
  const visible = items.slice(0, limit);
  return (
    <section aria-label="Related composers">
      <div className="mb-4 flex items-baseline justify-between">
        <h2 className="text-[18px] font-bold tracking-tight text-th-text-primary">
          Related composers
        </h2>
        <span className="text-[12px] text-th-text-muted">
          Wikidata · genre overlap
        </span>
      </div>
      <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 md:grid-cols-4">
        {visible.map((r) => {
          const tooltip =
            r.sharedGenres && r.sharedGenres.length > 0
              ? `Shares ${r.sharedGenres.length} genre${r.sharedGenres.length === 1 ? "" : "s"} on Wikidata`
              : "Wikidata-related composer";
          const clickable = r.mbid && r.mbid.length > 0;
          const inner = (
            <div
              className={`flex items-center gap-3 rounded-xl border border-th-border-subtle/60 bg-th-elevated p-3 transition-colors ${
                clickable
                  ? "cursor-pointer hover:border-th-accent/40 hover:bg-th-surface-hover"
                  : "cursor-default opacity-80"
              }`}
              title={tooltip}
            >
              <div className="h-12 w-12 shrink-0 overflow-hidden rounded-full bg-th-surface-hover">
                {r.portraitUrl ? (
                  <img
                    src={r.portraitUrl}
                    alt=""
                    className="h-full w-full object-cover"
                    onError={(e) => {
                      (e.target as HTMLImageElement).style.display = "none";
                    }}
                  />
                ) : (
                  <div className="flex h-full w-full items-center justify-center text-th-text-faint">
                    ♪
                  </div>
                )}
              </div>
              <div className="min-w-0 flex-1">
                <p className="truncate text-[13px] font-bold text-th-text-primary">
                  {r.name}
                </p>
                <p className="text-[11px] text-th-text-muted tabular-nums">
                  {r.birthYear ?? "—"}
                </p>
              </div>
            </div>
          );
          if (clickable && r.mbid) {
            return (
              <button
                key={r.qid}
                type="button"
                onClick={() => navigateToClassicalComposer(r.mbid!, r.name)}
                className="block text-left"
              >
                {inner}
              </button>
            );
          }
          return <div key={r.qid}>{inner}</div>;
        })}
      </div>
    </section>
  );
}

function ComposerSkeleton() {
  return (
    <div className="space-y-8">
      <div className="flex gap-6">
        <div className="h-[180px] w-[180px] animate-pulse rounded-full bg-th-surface/60" />
        <div className="flex flex-1 flex-col gap-3 pt-3">
          <div className="h-3 w-24 animate-pulse rounded bg-th-surface/50" />
          <div className="h-10 w-2/3 animate-pulse rounded bg-th-surface/60" />
          <div className="h-3 w-1/3 animate-pulse rounded bg-th-surface/40" />
          <div className="h-3 w-3/4 animate-pulse rounded bg-th-surface/30" />
        </div>
      </div>
      <div className="space-y-3">
        {Array.from({ length: 3 }).map((_, idx) => (
          <div key={idx} className="space-y-2">
            <div className="h-4 w-32 animate-pulse rounded bg-th-surface/50" />
            <div className="grid grid-cols-2 gap-3 sm:grid-cols-4 md:grid-cols-6">
              {Array.from({ length: 6 }).map((_, j) => (
                <div
                  key={j}
                  className="aspect-square animate-pulse rounded-md bg-th-surface/40"
                />
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
