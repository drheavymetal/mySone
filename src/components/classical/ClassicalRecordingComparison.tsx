import { useEffect, useMemo, useState } from "react";

import PageContainer from "../PageContainer";
import {
  getClassicalWork,
  listClassicalRecordingComparison,
} from "../../api/classical";
import type {
  Recording,
  RecordingComparisonRow,
  Work,
} from "../../types/classical";

interface ClassicalRecordingComparisonProps {
  workMbid: string;
  onBack: () => void;
}

interface CombinedRow extends RecordingComparisonRow {
  recording?: Recording;
}

/**
 * Phase 6 (B6.3 + F6.10) — side-by-side comparison of every recording
 * of a work the user has played. Reached via deep-link
 * `classical://compare/{workMbid}` (typically from "X versions played"
 * link inside a WorkPage).
 *
 * The component fans out two parallel calls: the per-recording stats
 * (`listClassicalRecordingComparison`) and the catalog Work
 * (`getClassicalWork`). The catalog data hydrates the conductor /
 * orchestra / year columns the stats DB doesn't carry by itself.
 */
export default function ClassicalRecordingComparison({
  workMbid,
  onBack,
}: ClassicalRecordingComparisonProps) {
  const [stats, setStats] = useState<RecordingComparisonRow[]>([]);
  const [work, setWork] = useState<Work | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    Promise.all([
      listClassicalRecordingComparison(workMbid).catch((err: unknown) => {
        console.error("[recording-cmp] stats failed:", err);
        return [] as RecordingComparisonRow[];
      }),
      getClassicalWork(workMbid).catch((err: unknown) => {
        console.error("[recording-cmp] work failed:", err);
        return null;
      }),
    ])
      .then(([rows, w]) => {
        if (!cancelled) {
          setStats(rows);
          setWork(w);
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
  }, [workMbid]);

  const combined: CombinedRow[] = useMemo(() => {
    const recIndex = new Map<string, Recording>();
    if (work) {
      for (const r of work.recordings) {
        recIndex.set(r.mbid, r);
      }
    }
    return stats.map((row) => ({
      ...row,
      recording: recIndex.get(row.recordingMbid),
    }));
  }, [stats, work]);

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
            Your recordings of
            {work ? ` ${work.title}` : "…"}
          </h1>
          <p className="mt-1 text-[12px] text-th-text-muted">
            {combined.length} version
            {combined.length === 1 ? "" : "s"} played · ranked by play count
          </p>
        </header>

        {loading && (
          <div className="space-y-2">
            {Array.from({ length: 4 }).map((_, i) => (
              <div
                key={i}
                className="h-16 animate-pulse rounded-md bg-th-surface/40"
              />
            ))}
          </div>
        )}

        {error && !loading && (
          <div className="rounded-xl border border-red-500/40 bg-red-500/10 p-4 text-[13px] text-red-200">
            Could not load comparison: {error}
          </div>
        )}

        {!loading && !error && combined.length === 0 && (
          <div className="rounded-xl border border-dashed border-th-border-subtle/60 bg-th-surface/30 p-6 text-center text-[13px] text-th-text-muted">
            You haven't played any recording of this work yet.
          </div>
        )}

        {!loading && !error && combined.length > 0 && (
          <ComparisonTable rows={combined} />
        )}
      </PageContainer>
    </div>
  );
}

interface ComparisonTableProps {
  rows: CombinedRow[];
}

function ComparisonTable({ rows }: ComparisonTableProps) {
  return (
    <table className="w-full table-auto border-separate border-spacing-y-1">
      <thead>
        <tr className="text-[11px] uppercase tracking-wider text-th-text-faint">
          <th className="px-3 py-2 text-left font-semibold">Recording</th>
          <th className="px-3 py-2 text-right font-semibold">Plays</th>
          <th className="px-3 py-2 text-right font-semibold">Completion</th>
          <th className="px-3 py-2 text-right font-semibold">Listened</th>
          <th className="px-3 py-2 text-right font-semibold">Last played</th>
        </tr>
      </thead>
      <tbody>
        {rows.map((row) => {
          const conductor = row.recording?.conductor?.name;
          const orchestra = row.recording?.orchestras?.[0]?.name;
          const year = row.recording?.recordingYear;
          const completionRate =
            row.plays === 0
              ? 0
              : Math.min(100, Math.round((row.completedCount / row.plays) * 100));
          return (
            <tr
              key={row.recordingMbid}
              className="bg-th-elevated/60 transition-colors hover:bg-th-surface-hover"
            >
              <td className="rounded-l-md px-3 py-2 text-[13px]">
                <p className="truncate font-bold text-th-text-primary">
                  {conductor ?? row.sampleArtist ?? "Unknown conductor"}
                  {orchestra && (
                    <span className="ml-1 font-normal text-th-text-secondary">
                      · {orchestra}
                    </span>
                  )}
                </p>
                <p className="mt-0.5 truncate text-[11px] text-th-text-muted">
                  {row.sampleAlbum ?? "—"}
                  {year && (
                    <span className="ml-2 font-mono text-th-text-faint">
                      {year}
                    </span>
                  )}
                </p>
              </td>
              <td className="px-3 py-2 text-right tabular-nums text-[14px] font-bold text-th-text-primary">
                {row.plays.toLocaleString()}
              </td>
              <td className="px-3 py-2 text-right tabular-nums text-[12px] text-th-text-secondary">
                {completionRate}%
              </td>
              <td className="px-3 py-2 text-right tabular-nums text-[12px] text-th-text-secondary">
                {formatSecs(row.listenedSecs)}
              </td>
              <td className="rounded-r-md px-3 py-2 text-right tabular-nums text-[11px] text-th-text-muted">
                {timeAgo(row.lastStartedAt)}
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}

function formatSecs(secs: number): string {
  if (secs < 60) {
    return `${secs}s`;
  }
  const m = Math.floor(secs / 60);
  if (m < 60) {
    return `${m}m`;
  }
  const h = Math.floor(m / 60);
  const rm = m % 60;
  return rm ? `${h}h ${rm}m` : `${h}h`;
}

function timeAgo(unix: number): string {
  const now = Date.now() / 1000;
  const diff = now - unix;
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
