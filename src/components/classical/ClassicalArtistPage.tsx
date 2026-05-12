import { useEffect, useState } from "react";

import PageContainer from "../PageContainer";
import { useNavigation } from "../../hooks/useNavigation";
import { getClassicalArtistDiscography } from "../../api/classical";
import type {
  ArtistDiscography,
  DiscographyEntry,
  DiscographyGroup,
} from "../../types/classical";

interface ClassicalArtistPageProps {
  mbid: string;
  displayName?: string;
  onBack: () => void;
}

/**
 * Phase 6 (D-022 + F6.9) — landing page for a conductor / orchestra /
 * soloist. Reached by clicking a conductor's name in any Recording row,
 * or by deep-link `classical://artist/{mbid}`.
 *
 * The backend `getClassicalArtistDiscography` returns a flat entries
 * list + a grouped view (`DiscographyGroup`) — the UI prioritises the
 * grouped view because for a popular conductor we typically want
 * "5 versions of Beethoven 9" tiles rather than 60 movements in a row.
 */
export default function ClassicalArtistPage({
  mbid,
  displayName,
  onBack,
}: ClassicalArtistPageProps) {
  const { navigateToClassicalWork } = useNavigation();
  const [data, setData] = useState<ArtistDiscography | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    getClassicalArtistDiscography(mbid, 100)
      .then((d) => {
        if (!cancelled) {
          setData(d);
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
  }, [mbid]);

  const knownGroups: DiscographyGroup[] =
    data?.groups.filter((g) => Boolean(g.workMbid)) ?? [];
  const ungrouped: DiscographyGroup | undefined = data?.groups.find(
    (g) => !g.workMbid,
  );

  return (
    <div className="flex-1 bg-gradient-to-b from-th-surface to-th-base min-h-full overflow-y-auto">
      <PageContainer className="px-8 py-10">
        <button
          type="button"
          onClick={onBack}
          className="mb-6 inline-flex items-center gap-1 text-[13px] font-medium text-th-text-secondary hover:text-th-text-primary transition-colors"
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

        <header className="mb-6">
          <h1 className="text-[28px] font-extrabold tracking-tight text-th-text-primary">
            {displayName ?? "Discography"}
          </h1>
          <p className="mt-1 text-[12px] text-th-text-muted">
            {data
              ? `${data.total.toLocaleString()} recordings · ${knownGroups.length} works`
              : "Loading discography…"}
          </p>
        </header>

        {loading && (
          <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
            {Array.from({ length: 6 }).map((_, i) => (
              <div
                key={i}
                className="h-24 animate-pulse rounded-xl bg-th-surface/40"
              />
            ))}
          </div>
        )}

        {error && !loading && (
          <div className="rounded-xl border border-red-500/40 bg-red-500/10 p-4 text-[13px] text-red-200">
            Could not load discography: {error}
          </div>
        )}

        {!loading && !error && data && (
          <>
            {knownGroups.length > 0 && (
              <section
                aria-labelledby="grouped-heading"
                className="mb-10"
              >
                <h2
                  id="grouped-heading"
                  className="mb-3 text-[18px] font-bold text-th-text-primary"
                >
                  Works performed
                </h2>
                <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
                  {knownGroups.map((g) => (
                    <WorkGroupCard
                      key={g.workMbid ?? "ungrouped"}
                      group={g}
                      entries={data.entries}
                      onOpenWork={(workMbid, sample) =>
                        navigateToClassicalWork(workMbid, sample)
                      }
                    />
                  ))}
                </div>
              </section>
            )}
            {ungrouped && ungrouped.indices.length > 0 && (
              <section aria-labelledby="ungrouped-heading">
                <h2
                  id="ungrouped-heading"
                  className="mb-3 text-[16px] font-bold text-th-text-secondary"
                >
                  Other recordings
                </h2>
                <ul className="space-y-1">
                  {ungrouped.indices.map((i) => {
                    const e = data.entries[i];
                    if (!e) {
                      return null;
                    }
                    return (
                      <li
                        key={e.recordingMbid}
                        className="rounded-md bg-th-surface/30 px-3 py-2 text-[13px] text-th-text-secondary"
                      >
                        <span className="font-semibold text-th-text-primary">
                          {e.title}
                        </span>
                        {e.releaseYear && (
                          <span className="ml-2 font-mono text-[11px] text-th-text-muted">
                            {e.releaseYear}
                          </span>
                        )}
                        <span className="ml-2 text-[11px] text-th-text-faint">
                          {e.artistCredit}
                        </span>
                      </li>
                    );
                  })}
                </ul>
              </section>
            )}
            {data.total === 0 && (
              <div className="rounded-xl border border-dashed border-th-border-subtle/60 bg-th-surface/30 p-6 text-center text-[13px] text-th-text-muted">
                MusicBrainz has no recordings credited to this artist.
              </div>
            )}
          </>
        )}
      </PageContainer>
    </div>
  );
}

interface WorkGroupCardProps {
  group: DiscographyGroup;
  entries: DiscographyEntry[];
  onOpenWork: (workMbid: string, sample?: string) => void;
}

function WorkGroupCard({ group, entries, onOpenWork }: WorkGroupCardProps) {
  if (!group.workMbid) {
    return null;
  }
  const sampleEntry = entries[group.indices[0] ?? 0];
  const sampleTitle = sampleEntry?.title;
  const years = group.indices
    .map((i) => entries[i]?.releaseYear)
    .filter((y): y is number => Boolean(y))
    .sort((a, b) => a - b);
  const yearLabel =
    years.length === 0
      ? null
      : years[0] === years[years.length - 1]
        ? `${years[0]}`
        : `${years[0]}–${years[years.length - 1]}`;
  return (
    <button
      type="button"
      onClick={() => onOpenWork(group.workMbid!, sampleTitle)}
      className="rounded-xl border border-th-border-subtle/60 bg-th-elevated p-4 text-left transition-[background-color,border-color] duration-200 hover:border-th-accent/40 hover:bg-th-surface-hover"
    >
      <p className="truncate text-[14px] font-bold text-th-text-primary">
        {sampleTitle ?? "Untitled work"}
      </p>
      <p className="mt-1 text-[12px] text-th-text-secondary">
        {group.count} recording{group.count > 1 ? "s" : ""}
        {yearLabel && (
          <span className="ml-2 font-mono text-[11px] text-th-text-muted">
            {yearLabel}
          </span>
        )}
      </p>
    </button>
  );
}
