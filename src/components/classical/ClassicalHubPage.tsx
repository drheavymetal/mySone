import { useEffect, useMemo, useState } from "react";
import {
  ArrowRight,
  Calendar,
  Clock,
  Compass,
  Crown,
  ListMusic,
  Star,
} from "lucide-react";

import PageContainer from "../PageContainer";
import { useNavigation } from "../../hooks/useNavigation";
import {
  getClassicalExtendedTotal,
  listClassicalEditorialPicks,
  listClassicalTopComposers,
  listRecentClassicalSessions,
  listTopClassicalWorks,
} from "../../api/classical";
import type {
  ComposerSummary,
  EditorialPick,
  RecentClassicalSession,
  TopClassicalWork,
} from "../../types/classical";

import ComposerCard from "./ComposerCard";

interface ClassicalHubPageProps {
  onBack: () => void;
}

type Tab = "listen" | "browse";

/**
 * Phase 2 root of the Classical Hub. The user lands here after clicking
 * the "Classical Hub" pill in Explore. Two tabs:
 *   - Listen Now: featured composers (from OpenOpus snapshot — synchronous,
 *     no network on mount), placeholder cards for future personalized
 *     recommendations / editor's choice (filled in Phase 5 + 6).
 *   - Browse: three browse axes (composers, periods, genres) as quick-entry
 *     cards. Each axis drills into its own list page.
 *
 * The page never touches audio routing. All work happens through the
 * Tauri commands that wrap the read-only catalog service.
 */
export default function ClassicalHubPage({ onBack }: ClassicalHubPageProps) {
  const {
    navigateToClassicalComposer,
    navigateToClassicalBrowse,
    navigateToClassicalSearch,
    navigateToClassicalWork,
    navigateToClassicalLibrary,
  } = useNavigation();
  const [tab, setTab] = useState<Tab>("listen");
  const [composers, setComposers] = useState<ComposerSummary[]>([]);
  const [picks, setPicks] = useState<EditorialPick[]>([]);
  const [topWorks, setTopWorks] = useState<TopClassicalWork[]>([]);
  const [recentSessions, setRecentSessions] = useState<RecentClassicalSession[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  // Phase 7 (F7.3) — total composers in the extended snapshot. Surfaced
  // by the Hub footer chip ("Catalog: X composers indexed").
  const [extendedTotal, setExtendedTotal] = useState<number>(0);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    Promise.all([
      listClassicalTopComposers(30).catch((err: unknown) => {
        console.error("[classical] load top composers failed:", err);
        return [] as ComposerSummary[];
      }),
      listClassicalEditorialPicks(12).catch((err: unknown) => {
        console.error("[classical] load editorial picks failed:", err);
        return [] as EditorialPick[];
      }),
      listTopClassicalWorks("all", 8).catch((err: unknown) => {
        console.error("[classical] load top works failed:", err);
        return [] as TopClassicalWork[];
      }),
      listRecentClassicalSessions(7 * 24 * 3600, 6).catch((err: unknown) => {
        console.error("[classical] load recent sessions failed:", err);
        return [] as RecentClassicalSession[];
      }),
      getClassicalExtendedTotal().catch((err: unknown) => {
        console.error("[classical] load extended total failed:", err);
        return 0;
      }),
    ])
      .then(([cs, ps, tw, rs, total]) => {
        if (!cancelled) {
          setComposers(cs);
          setPicks(ps);
          setTopWorks(tw);
          setRecentSessions(rs);
          setExtendedTotal(total);
          setLoading(false);
        }
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : String(err));
          setLoading(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const featured = useMemo(() => composers.slice(0, 12), [composers]);

  const handlePickClick = (pick: EditorialPick) => {
    // The pick carries `composerMbid` + `titleCanonical` but no
    // `workMbid` (D-020 stores logical pointer, the work_mbid is
    // resolved at runtime). We deep-link to the search page with a
    // pre-built query so the user sees the work in context — this is
    // intentional: it both validates the search path AND surfaces
    // adjacent recordings.
    const composer = pick.composerName.split(" ").pop() ?? pick.composerName;
    const q = pick.catalogue
      ? `${composer} ${pick.catalogue}`
      : `${composer} ${pick.titleCanonical}`;
    navigateToClassicalSearch(q);
  };

  return (
    <div className="flex-1 bg-gradient-to-b from-th-surface to-th-base min-h-full overflow-y-auto">
      <PageContainer className="px-8 py-10">
        {/* Back affordance */}
        <button
          type="button"
          onClick={onBack}
          className="mb-6 inline-flex items-center gap-1 text-[13px] font-medium text-th-text-secondary hover:text-th-text-primary transition-colors"
          aria-label="Back to Explore"
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
          Back to Explore
        </button>

        {/* Hero */}
        <header className="mb-8">
          <div className="flex items-center gap-3">
            <span
              aria-hidden="true"
              className="inline-flex h-10 w-10 items-center justify-center rounded-lg bg-gradient-to-br from-th-accent/40 to-th-accent/10 text-2xl"
            >
              ♪
            </span>
            <h1 className="text-[36px] font-extrabold tracking-tight text-th-text-primary">
              Classical Hub
            </h1>
          </div>
          <p className="mt-2 max-w-2xl text-[14px] text-th-text-muted">
            Discover composers, works, and recordings. Powered by MusicBrainz +
            OpenOpus + Wikipedia, with bit-perfect playback through your Tidal
            account.
          </p>
        </header>

        {/* Tabs */}
        <div
          role="tablist"
          aria-label="Hub sections"
          className="mb-8 flex gap-1 rounded-full border border-th-border-subtle/60 bg-th-surface/40 p-1"
        >
          <TabButton
            active={tab === "listen"}
            onClick={() => setTab("listen")}
            label="Listen Now"
          />
          <TabButton
            active={tab === "browse"}
            onClick={() => setTab("browse")}
            label="Browse"
          />
          <TabButton
            label="Search"
            onClick={() => navigateToClassicalSearch()}
          />
          <TabButton
            label="Library"
            onClick={() => navigateToClassicalLibrary()}
          />
        </div>

        {tab === "listen" && (
          <ListenNow
            composers={featured}
            picks={picks}
            topWorks={topWorks}
            recentSessions={recentSessions}
            loading={loading}
            error={error}
            onComposerClick={(c) =>
              navigateToClassicalComposer(c.mbid, c.fullName ?? c.name)
            }
            onPickClick={handlePickClick}
            onPickLongClick={(pick) =>
              navigateToClassicalComposer(
                pick.composerMbid,
                pick.composerName,
              )
            }
            onWorkClick={navigateToClassicalWork}
          />
        )}

        {tab === "browse" && (
          <BrowseGrid
            onPickComposers={() => navigateToClassicalBrowse("composers")}
            onPickPeriods={() => navigateToClassicalBrowse("periods")}
            onPickGenres={() => navigateToClassicalBrowse("genres")}
          />
        )}

        {/*
         * Phase 7 (F7.3) — catalog completeness footer chip. Subtle so
         * it doesn't compete with the primary navigation; useful for
         * the user to know the universe is in the thousands, not the
         * 33-composer canon snapshot.
         */}
        {extendedTotal > 0 && (
          <p className="mt-12 text-center text-[11px] text-th-text-faint">
            Catalog: {extendedTotal.toLocaleString()} composers indexed.{" "}
            <button
              type="button"
              onClick={() => navigateToClassicalBrowse("composers")}
              className="underline hover:text-th-text-secondary transition-colors"
            >
              Browse all
            </button>
          </p>
        )}
      </PageContainer>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

interface TabButtonProps {
  label: string;
  active?: boolean;
  disabled?: boolean;
  onClick?: () => void;
}

function TabButton({ label, active, disabled, onClick }: TabButtonProps) {
  return (
    <button
      type="button"
      role="tab"
      aria-selected={active ?? false}
      disabled={disabled}
      onClick={onClick}
      className={`rounded-full px-4 py-1.5 text-[13px] font-semibold transition-colors ${
        active
          ? "bg-th-accent text-black shadow"
          : disabled
            ? "cursor-not-allowed text-th-text-faint/60"
            : "text-th-text-secondary hover:bg-th-surface-hover hover:text-th-text-primary"
      }`}
    >
      {label}
    </button>
  );
}

interface ListenNowProps {
  composers: ComposerSummary[];
  picks: EditorialPick[];
  topWorks: TopClassicalWork[];
  recentSessions: RecentClassicalSession[];
  loading: boolean;
  error: string | null;
  onComposerClick: (c: ComposerSummary) => void;
  onPickClick: (p: EditorialPick) => void;
  onPickLongClick: (p: EditorialPick) => void;
  onWorkClick: (workMbid: string, title?: string) => void;
}

function ListenNow({
  composers,
  picks,
  topWorks,
  recentSessions,
  loading,
  error,
  onComposerClick,
  onPickClick,
  onPickLongClick,
  onWorkClick,
}: ListenNowProps) {
  return (
    <div className="space-y-10">
      {/* Phase 6 — Recently played classical sessions (only when present) */}
      {recentSessions.length > 0 && (
        <section aria-labelledby="recent-heading">
          <div className="mb-4 flex items-baseline justify-between">
            <h2
              id="recent-heading"
              className="flex items-center gap-2 text-[20px] font-bold text-th-text-primary tracking-tight"
            >
              <Clock size={18} className="text-th-accent" aria-hidden="true" />
              Recently played
            </h2>
            <span className="text-[12px] text-th-text-muted">
              From your stats · last 7 days
            </span>
          </div>
          <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
            {recentSessions.map((s) => (
              <RecentSessionCard
                key={s.workMbid}
                session={s}
                onOpen={() => onWorkClick(s.workMbid, s.sampleTitle)}
              />
            ))}
          </div>
        </section>
      )}

      {/* Phase 6 — Your top works (only when present) */}
      {topWorks.length > 0 && (
        <section aria-labelledby="top-works-heading">
          <div className="mb-4 flex items-baseline justify-between">
            <h2
              id="top-works-heading"
              className="flex items-center gap-2 text-[20px] font-bold text-th-text-primary tracking-tight"
            >
              <Crown size={18} className="text-th-accent" aria-hidden="true" />
              Your top works
            </h2>
            <span className="text-[12px] text-th-text-muted">
              All time · ranked by play count
            </span>
          </div>
          <div className="grid grid-cols-1 gap-2 md:grid-cols-2">
            {topWorks.map((w) => (
              <TopWorkCard
                key={w.workMbid}
                work={w}
                onOpen={() => onWorkClick(w.workMbid, w.sampleTitle)}
              />
            ))}
          </div>
        </section>
      )}

      {/* Featured composers */}
      <section aria-labelledby="featured-heading">
        <div className="mb-4 flex items-baseline justify-between">
          <h2
            id="featured-heading"
            className="text-[20px] font-bold text-th-text-primary tracking-tight"
          >
            Featured composers
          </h2>
          <span className="text-[12px] text-th-text-muted">
            From the canon
          </span>
        </div>

        {loading && <FeaturedSkeleton />}

        {error && !loading && (
          <div className="rounded-xl border border-red-500/40 bg-red-500/10 p-4 text-[13px] text-red-200">
            Could not load composers: {error}
          </div>
        )}

        {!loading && !error && composers.length > 0 && (
          <div className="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6">
            {composers.map((c) => (
              <ComposerCard
                key={c.mbid}
                composer={c}
                onClick={() => onComposerClick(c)}
              />
            ))}
          </div>
        )}
      </section>

      {/* Editor's Choice (Phase 5 — D-020) */}
      <section aria-labelledby="editors-choice-heading">
        <div className="mb-4 flex items-baseline justify-between">
          <h2
            id="editors-choice-heading"
            className="flex items-center gap-2 text-[20px] font-bold text-th-text-primary tracking-tight"
          >
            <Star size={18} className="text-th-accent" aria-hidden="true" />
            Editor's Choice
          </h2>
          <span className="text-[12px] text-th-text-muted">
            Curated from canon · auditable
          </span>
        </div>

        {loading && <PicksSkeleton />}

        {!loading && picks.length > 0 && (
          <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
            {picks.map((p) => (
              <EditorialPickCard
                key={`${p.composerMbid}:${p.titleCanonical}`}
                pick={p}
                onClick={() => onPickClick(p)}
                onComposerClick={() => onPickLongClick(p)}
              />
            ))}
          </div>
        )}

        {!loading && picks.length === 0 && (
          <div className="rounded-xl border border-th-border-subtle/60 bg-th-surface/30 p-4 text-[12px] text-th-text-secondary">
            Editorial picks unavailable.
          </div>
        )}
      </section>

      {/* Empty-state hint when the user hasn't accumulated any
          classical history yet — surfaced INSTEAD of the top-works /
          recent-sessions sections, which gate themselves on data. */}
      {topWorks.length === 0 && recentSessions.length === 0 && !loading && (
        <section aria-label="Build your classical history">
          <div className="rounded-xl border border-dashed border-th-border-subtle/60 bg-th-surface/30 p-6 text-center text-[13px] text-th-text-muted">
            Play a classical track from any Work page to populate your
            personal "top works" and "recently played" lists.
          </div>
        </section>
      )}
    </div>
  );
}

interface RecentSessionCardProps {
  session: RecentClassicalSession;
  onOpen: () => void;
}

function RecentSessionCard({ session, onOpen }: RecentSessionCardProps) {
  const last = new Date(session.lastStartedAt * 1000);
  const elapsed = Math.max(
    0,
    Math.floor(Date.now() / 1000 - session.lastStartedAt),
  );
  const ago =
    elapsed < 3600
      ? `${Math.floor(elapsed / 60)}m ago`
      : elapsed < 86400
        ? `${Math.floor(elapsed / 3600)}h ago`
        : `${Math.floor(elapsed / 86400)}d ago`;
  return (
    <button
      type="button"
      onClick={onOpen}
      className="rounded-xl border border-th-border-subtle/60 bg-th-elevated p-4 text-left transition-[background-color,border-color] duration-200 hover:border-th-accent/40 hover:bg-th-surface-hover"
    >
      <p className="truncate text-[14px] font-bold text-th-text-primary">
        {session.sampleAlbum ?? session.sampleTitle ?? "Untitled work"}
      </p>
      <p className="mt-1 truncate text-[12px] text-th-text-secondary">
        {session.sampleArtist ?? "Unknown artist"}
      </p>
      <p
        className="mt-2 text-[11px] text-th-text-faint"
        title={last.toLocaleString()}
      >
        {session.plays} play{session.plays > 1 ? "s" : ""} · {ago}
      </p>
    </button>
  );
}

interface TopWorkCardProps {
  work: TopClassicalWork;
  onOpen: () => void;
}

function TopWorkCard({ work, onOpen }: TopWorkCardProps) {
  return (
    <button
      type="button"
      onClick={onOpen}
      className="flex items-center justify-between rounded-md bg-th-surface/40 px-3 py-2 text-left transition-colors hover:bg-th-surface-hover"
    >
      <span className="min-w-0 flex-1">
        <p className="truncate text-[13px] font-bold text-th-text-primary">
          {work.sampleTitle ?? "Untitled work"}
        </p>
        <p className="truncate text-[11px] text-th-text-muted">
          {work.sampleArtist ?? ""}
          {work.distinctRecordings > 1 && (
            <span className="ml-2 text-th-text-faint">
              · {work.distinctRecordings} versions
            </span>
          )}
        </p>
      </span>
      <span className="ml-3 shrink-0 tabular-nums text-[14px] font-bold text-th-accent">
        {work.plays}
      </span>
    </button>
  );
}

interface EditorialPickCardProps {
  pick: EditorialPick;
  onClick: () => void;
  onComposerClick: () => void;
}

function EditorialPickCard({
  pick,
  onClick,
  onComposerClick,
}: EditorialPickCardProps) {
  const choice = pick.editorsChoice;
  const performerSummary = choice.conductor
    ? `${choice.conductor} · ${choice.performer}`
    : choice.performer;
  return (
    <article className="rounded-xl border border-th-border-subtle/60 bg-th-surface/40 p-4 transition-colors hover:border-th-accent/40">
      <button
        type="button"
        onClick={onClick}
        className="block w-full text-left"
      >
        <div className="flex items-baseline gap-2">
          <Star
            size={14}
            className="shrink-0 text-th-accent"
            aria-hidden="true"
          />
          <h3 className="flex-1 truncate text-[14px] font-bold text-th-text-primary">
            {pick.titleCanonical}
            {pick.catalogue && (
              <span className="ml-2 font-mono text-[12px] text-th-text-muted">
                {pick.catalogue}
              </span>
            )}
          </h3>
        </div>
        <p className="mt-1 truncate text-[12px] font-semibold text-th-text-secondary">
          {performerSummary}
          {choice.year && (
            <span className="ml-2 font-mono text-th-text-muted">
              {choice.year}
            </span>
          )}
        </p>
        {choice.note && (
          <p className="mt-2 line-clamp-2 text-[12px] italic leading-relaxed text-th-text-muted">
            {choice.note}
          </p>
        )}
      </button>
      <div className="mt-3 flex items-center justify-between border-t border-th-border-subtle/30 pt-2">
        <button
          type="button"
          onClick={onComposerClick}
          className="text-[11px] font-medium text-th-text-secondary hover:text-th-text-primary transition-colors"
        >
          {pick.composerName}
        </button>
        {choice.label && (
          <span className="text-[11px] text-th-text-faint">
            {choice.label}
          </span>
        )}
      </div>
    </article>
  );
}

function PicksSkeleton() {
  return (
    <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
      {Array.from({ length: 4 }).map((_, idx) => (
        <div
          key={idx}
          className="h-28 animate-pulse rounded-xl bg-th-surface/40"
        />
      ))}
    </div>
  );
}

interface BrowseGridProps {
  onPickComposers: () => void;
  onPickPeriods: () => void;
  onPickGenres: () => void;
}

function BrowseGrid({
  onPickComposers,
  onPickPeriods,
  onPickGenres,
}: BrowseGridProps) {
  return (
    <div className="grid grid-cols-1 gap-4 md:grid-cols-3">
      <BrowseAxisCard
        icon={<Compass size={22} />}
        title="Browse Composers"
        subtitle="The canon, filterable by era"
        onClick={onPickComposers}
      />
      <BrowseAxisCard
        icon={<Calendar size={22} />}
        title="Browse Periods"
        subtitle="From Medieval to Contemporary"
        onClick={onPickPeriods}
      />
      <BrowseAxisCard
        icon={<ListMusic size={22} />}
        title="Browse Genres"
        subtitle="Symphonies, concertos, chamber, opera"
        onClick={onPickGenres}
      />
    </div>
  );
}

interface BrowseAxisCardProps {
  icon: React.ReactNode;
  title: string;
  subtitle: string;
  onClick: () => void;
}

function BrowseAxisCard({
  icon,
  title,
  subtitle,
  onClick,
}: BrowseAxisCardProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="group flex items-center gap-4 rounded-xl border border-th-border-subtle/60 bg-th-elevated p-5 text-left transition-[background-color,border-color] duration-200 hover:border-th-accent/40 hover:bg-th-surface-hover"
    >
      <span
        aria-hidden="true"
        className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-th-accent/15 text-th-accent"
      >
        {icon}
      </span>
      <span className="flex-1">
        <span className="block text-[15px] font-bold text-th-text-primary">
          {title}
        </span>
        <span className="mt-0.5 block text-[12px] text-th-text-muted">
          {subtitle}
        </span>
      </span>
      <ArrowRight
        size={18}
        className="shrink-0 text-th-text-muted transition-transform group-hover:translate-x-0.5 group-hover:text-th-text-primary"
      />
    </button>
  );
}

function FeaturedSkeleton() {
  return (
    <div className="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6">
      {Array.from({ length: 12 }).map((_, idx) => (
        <div
          key={idx}
          className="flex flex-col items-center gap-2 rounded-xl bg-th-elevated p-3"
        >
          <div className="aspect-square w-full animate-pulse rounded-full bg-th-surface/60" />
          <div className="h-3 w-2/3 animate-pulse rounded bg-th-surface/50" />
          <div className="h-2 w-1/3 animate-pulse rounded bg-th-surface/40" />
        </div>
      ))}
    </div>
  );
}
