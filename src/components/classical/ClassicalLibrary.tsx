import { useEffect, useMemo, useState } from "react";
import { Heart, Library, Music, User, Users } from "lucide-react";

import PageContainer from "../PageContainer";
import { useNavigation } from "../../hooks/useNavigation";
import {
  getClassicalOverview,
  listClassicalFavorites,
  removeClassicalFavorite,
} from "../../api/classical";
import type {
  ClassicalFavorite,
  ClassicalOverview,
} from "../../types/classical";

interface ClassicalLibraryProps {
  /** Initial facet selection (deep-linked from `classical://library/{facet}`). */
  initialFacet?: ClassicalFavorite["kind"];
  onBack: () => void;
}

const FACETS: { id: ClassicalFavorite["kind"]; label: string; icon: React.ReactNode }[] = [
  { id: "work", label: "Works", icon: <Music size={16} /> },
  { id: "recording", label: "Recordings", icon: <Library size={16} /> },
  { id: "composer", label: "Composers", icon: <User size={16} /> },
  { id: "performer", label: "Performers", icon: <Users size={16} /> },
];

/**
 * Phase 6 (F6.3) — Library tab inside the Hub. Surfaces the four
 * facets persisted in `classical_favorites` (work / recording /
 * composer / performer) plus a hero overview that reads from
 * `getClassicalOverview` so the user gets an at-a-glance "how much
 * classical have I listened to?" header.
 *
 * The page never touches audio routing. It is read-only over the
 * `classical_favorites` and `plays` tables — pure stats DB reads.
 */
export default function ClassicalLibrary({
  initialFacet = "work",
  onBack,
}: ClassicalLibraryProps) {
  const {
    navigateToClassicalWork,
    navigateToClassicalComposer,
    navigateToClassicalArtist,
  } = useNavigation();
  const [facet, setFacet] = useState<ClassicalFavorite["kind"]>(initialFacet);
  const [items, setItems] = useState<ClassicalFavorite[]>([]);
  const [overview, setOverview] = useState<ClassicalOverview | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    Promise.all([
      listClassicalFavorites(facet, 100).catch((err: unknown) => {
        console.error("[classical-library] list failed:", err);
        return [] as ClassicalFavorite[];
      }),
      getClassicalOverview("all").catch((err: unknown) => {
        console.error("[classical-library] overview failed:", err);
        return null;
      }),
    ])
      .then(([rows, ov]) => {
        if (!cancelled) {
          setItems(rows);
          setOverview(ov);
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
  }, [facet]);

  const handleOpen = (item: ClassicalFavorite) => {
    if (item.kind === "work") {
      navigateToClassicalWork(item.mbid, item.displayName);
    } else if (item.kind === "composer") {
      navigateToClassicalComposer(item.mbid, item.displayName);
    } else if (item.kind === "performer") {
      navigateToClassicalArtist(item.mbid, item.displayName);
    } else {
      // Recordings have no dedicated standalone page — they live inside
      // a Work. We don't carry the parent work mbid in the favorite row,
      // so we surface the recording mbid in the explorer / no-op here.
    }
  };

  const handleRemove = async (item: ClassicalFavorite) => {
    try {
      await removeClassicalFavorite(item.kind, item.mbid);
      setItems((prev) => prev.filter((x) => x.id !== item.id));
    } catch (err) {
      console.error("[classical-library] remove failed:", err);
    }
  };

  return (
    <div className="flex-1 bg-gradient-to-b from-th-surface to-th-base min-h-full overflow-y-auto">
      <PageContainer className="px-8 py-10">
        <button
          type="button"
          onClick={onBack}
          className="mb-6 inline-flex items-center gap-1 text-[13px] font-medium text-th-text-secondary hover:text-th-text-primary transition-colors"
          aria-label="Back to Hub"
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
          Back to Hub
        </button>

        <header className="mb-8">
          <h1 className="text-[28px] font-extrabold tracking-tight text-th-text-primary">
            Your classical library
          </h1>
          <p className="mt-1 text-[13px] text-th-text-muted">
            Saved works, recordings, composers, and performers — all read
            from your local stats database.
          </p>
        </header>

        <OverviewBanner overview={overview} loading={loading} />

        {/* Facet tabs */}
        <div
          role="tablist"
          aria-label="Library facets"
          className="my-6 flex flex-wrap gap-1 rounded-full border border-th-border-subtle/60 bg-th-surface/40 p-1"
        >
          {FACETS.map((f) => {
            const active = facet === f.id;
            return (
              <button
                key={f.id}
                type="button"
                role="tab"
                aria-selected={active}
                onClick={() => setFacet(f.id)}
                className={`inline-flex items-center gap-1.5 rounded-full px-4 py-1.5 text-[13px] font-semibold transition-colors ${
                  active
                    ? "bg-th-accent text-black shadow"
                    : "text-th-text-secondary hover:bg-th-surface-hover hover:text-th-text-primary"
                }`}
              >
                {f.icon}
                {f.label}
              </button>
            );
          })}
        </div>

        {error && (
          <div className="rounded-xl border border-red-500/40 bg-red-500/10 p-4 text-[13px] text-red-200">
            Could not load library: {error}
          </div>
        )}

        {!error && (
          <FavoritesGrid
            items={items}
            loading={loading}
            facet={facet}
            onOpen={handleOpen}
            onRemove={handleRemove}
          />
        )}
      </PageContainer>
    </div>
  );
}

interface OverviewBannerProps {
  overview: ClassicalOverview | null;
  loading: boolean;
}

function OverviewBanner({ overview, loading }: OverviewBannerProps) {
  if (loading) {
    return (
      <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
        {Array.from({ length: 4 }).map((_, i) => (
          <div
            key={i}
            className="h-20 animate-pulse rounded-xl bg-th-surface/40"
          />
        ))}
      </div>
    );
  }
  if (!overview) {
    return null;
  }
  const hours = Math.floor(overview.totalListenedSecs / 3600);
  const cells = [
    { label: "Plays", value: overview.totalPlays },
    { label: "Hours listened", value: hours },
    { label: "Distinct works", value: overview.distinctWorks },
    { label: "Distinct composers", value: overview.distinctComposers },
  ];
  return (
    <section
      aria-label="Classical listening footprint"
      className="grid grid-cols-2 gap-3 sm:grid-cols-4"
    >
      {cells.map((c) => (
        <div
          key={c.label}
          className="rounded-xl border border-th-border-subtle/60 bg-th-elevated p-4"
        >
          <p className="text-[11px] uppercase tracking-wider text-th-text-faint">
            {c.label}
          </p>
          <p className="mt-1 text-[22px] font-extrabold tabular-nums text-th-text-primary">
            {c.value.toLocaleString()}
          </p>
        </div>
      ))}
    </section>
  );
}

interface FavoritesGridProps {
  items: ClassicalFavorite[];
  loading: boolean;
  facet: ClassicalFavorite["kind"];
  onOpen: (item: ClassicalFavorite) => void;
  onRemove: (item: ClassicalFavorite) => void;
}

function FavoritesGrid({
  items,
  loading,
  facet,
  onOpen,
  onRemove,
}: FavoritesGridProps) {
  const sorted = useMemo(
    () => [...items].sort((a, b) => b.addedAt - a.addedAt),
    [items],
  );

  if (loading) {
    return (
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
        {Array.from({ length: 6 }).map((_, i) => (
          <div
            key={i}
            className="h-16 animate-pulse rounded-xl bg-th-surface/40"
          />
        ))}
      </div>
    );
  }

  if (sorted.length === 0) {
    return (
      <div className="rounded-xl border border-dashed border-th-border-subtle/60 bg-th-surface/30 p-8 text-center text-[13px] text-th-text-muted">
        Nothing saved yet under "{facet}". Use the heart icon on any
        Work / Composer / Recording / Performer page to add it here.
      </div>
    );
  }

  return (
    <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
      {sorted.map((item) => (
        <article
          key={item.id}
          className="flex items-center justify-between rounded-xl border border-th-border-subtle/60 bg-th-elevated p-4 transition-colors hover:border-th-accent/40"
        >
          <button
            type="button"
            onClick={() => onOpen(item)}
            className="flex-1 text-left"
          >
            <p className="truncate text-[14px] font-bold text-th-text-primary">
              {item.displayName}
            </p>
            <p className="mt-0.5 truncate text-[11px] uppercase tracking-wider text-th-text-faint">
              {item.kind} · added {timeAgo(item.addedAt)}
            </p>
          </button>
          <button
            type="button"
            onClick={() => onRemove(item)}
            aria-label={`Remove ${item.displayName} from library`}
            className="ml-3 rounded-full p-2 text-th-accent hover:bg-th-surface-hover hover:text-red-400 transition-colors"
          >
            <Heart size={16} fill="currentColor" />
          </button>
        </article>
      ))}
    </div>
  );
}

function timeAgo(unixSecs: number): string {
  const now = Date.now() / 1000;
  const diff = now - unixSecs;
  if (diff < 60) {
    return "just now";
  }
  if (diff < 3600) {
    return `${Math.floor(diff / 60)}m ago`;
  }
  if (diff < 86400) {
    return `${Math.floor(diff / 3600)}h ago`;
  }
  return `${Math.floor(diff / 86400)}d ago`;
}
