/**
 * StatsPage — local-only listening statistics.
 *
 * Reads from the SQLite stats DB the backend writes to on every play.
 * No network, no telemetry — privacy-first by design. Tabs: Overview,
 * Top Tracks, Top Artists, Heatmap. Window selector lets the user pivot
 * between Week / Month / Year / All time.
 */

import { useEffect, useState, useMemo } from "react";
import {
  getStatsOverview,
  getTopTracks,
  getTopArtists,
  getTopAlbums,
  getListeningHeatmap,
  type StatsWindow,
  type StatsOverview,
  type TopTrack,
  type TopArtist,
  type TopAlbum,
  type HeatmapCell,
} from "../api/stats";
import PageContainer from "./PageContainer";

type Tab = "overview" | "tracks" | "artists" | "albums" | "heatmap";

const TABS: { id: Tab; label: string }[] = [
  { id: "overview", label: "Overview" },
  { id: "tracks", label: "Top Tracks" },
  { id: "artists", label: "Top Artists" },
  { id: "albums", label: "Top Albums" },
  { id: "heatmap", label: "Heatmap" },
];

const WINDOWS: { id: StatsWindow; label: string }[] = [
  { id: "week", label: "Week" },
  { id: "month", label: "Month" },
  { id: "year", label: "Year" },
  { id: "all", label: "All time" },
];

function formatDuration(secs: number): string {
  if (secs < 60) return `${secs}s`;
  const m = Math.floor(secs / 60);
  if (m < 60) return `${m}m`;
  const h = Math.floor(m / 60);
  const rm = m % 60;
  if (h < 24) return rm ? `${h}h ${rm}m` : `${h}h`;
  const d = Math.floor(h / 24);
  const rh = h % 24;
  return rh ? `${d}d ${rh}h` : `${d}d`;
}

function formatNumber(n: number): string {
  return n.toLocaleString();
}

export default function StatsPage() {
  const [tab, setTab] = useState<Tab>("overview");
  const [window, setWindow] = useState<StatsWindow>("month");

  return (
    <PageContainer className="px-6 pt-6 pb-8">
      <header className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-[26px] font-bold text-th-text-primary">Stats</h1>
          <p className="text-[12px] text-th-text-muted mt-0.5">
            Your listening history, on your machine. No telemetry, no upload.
          </p>
        </div>
        <div className="flex gap-1 rounded-lg bg-th-surface p-1">
          {WINDOWS.map((w) => (
            <button
              key={w.id}
              onClick={() => setWindow(w.id)}
              className={`px-3 py-1.5 text-[12px] font-medium rounded-md transition-colors ${
                window === w.id
                  ? "bg-th-accent text-black"
                  : "text-th-text-muted hover:text-th-text-primary"
              }`}
            >
              {w.label}
            </button>
          ))}
        </div>
      </header>

      <nav className="flex gap-1 mb-5 border-b border-th-border-subtle">
        {TABS.map((t) => (
          <button
            key={t.id}
            onClick={() => setTab(t.id)}
            className={`px-3 py-2 text-[13px] font-medium border-b-2 transition-colors -mb-px ${
              tab === t.id
                ? "border-th-accent text-th-text-primary"
                : "border-transparent text-th-text-muted hover:text-th-text-primary"
            }`}
          >
            {t.label}
          </button>
        ))}
      </nav>

      {tab === "overview" && <OverviewTab window={window} />}
      {tab === "tracks" && <TopTracksTab window={window} />}
      {tab === "artists" && <TopArtistsTab window={window} />}
      {tab === "albums" && <TopAlbumsTab window={window} />}
      {tab === "heatmap" && <HeatmapTab window={window} />}
    </PageContainer>
  );
}

// ─── Overview ─────────────────────────────────────────────────────────────

function OverviewTab({ window }: { window: StatsWindow }) {
  const [data, setData] = useState<StatsOverview | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    setLoading(true);
    getStatsOverview(window)
      .then((d) => setData(d))
      .finally(() => setLoading(false));
  }, [window]);

  if (loading) return <Loader />;
  if (!data) return null;

  const cards = [
    {
      label: "Total plays",
      value: formatNumber(data.totalPlays),
      sub: `${formatNumber(data.completedPlays)} completed`,
    },
    {
      label: "Time listened",
      value: formatDuration(data.totalListenedSecs),
      sub: `${formatNumber(Math.round(data.totalListenedSecs / 60))} min`,
    },
    {
      label: "Tracks",
      value: formatNumber(data.distinctTracks),
      sub: "distinct",
    },
    {
      label: "Artists",
      value: formatNumber(data.distinctArtists),
      sub: "distinct",
    },
    {
      label: "Albums",
      value: formatNumber(data.distinctAlbums),
      sub: "distinct",
    },
  ];

  if (data.totalPlays === 0) {
    return (
      <div className="text-center py-16 text-th-text-muted text-[14px]">
        No listening data yet for this window. Play something!
      </div>
    );
  }

  return (
    <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-5 gap-3">
      {cards.map((c) => (
        <div
          key={c.label}
          className="rounded-lg bg-th-surface p-4 flex flex-col"
        >
          <div className="text-[10px] font-bold tracking-wider text-th-text-faint">
            {c.label.toUpperCase()}
          </div>
          <div className="text-[22px] font-bold text-th-text-primary mt-1">
            {c.value}
          </div>
          <div className="text-[11px] text-th-text-muted mt-0.5">{c.sub}</div>
        </div>
      ))}
    </div>
  );
}

// ─── Top Tracks ────────────────────────────────────────────────────────────

function TopTracksTab({ window }: { window: StatsWindow }) {
  const [items, setItems] = useState<TopTrack[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    setLoading(true);
    getTopTracks(window, 50)
      .then(setItems)
      .finally(() => setLoading(false));
  }, [window]);

  if (loading) return <Loader />;
  return (
    <RankedList
      empty="No top tracks yet for this window."
      items={items.map((t, i) => ({
        key: `${t.trackId ?? `${t.title}|${t.artist}`}-${i}`,
        primary: t.title,
        secondary: t.artist + (t.album ? ` — ${t.album}` : ""),
        count: t.plays,
        countLabel: t.plays === 1 ? "play" : "plays",
        time: t.listenedSecs,
      }))}
    />
  );
}

// ─── Top Artists ──────────────────────────────────────────────────────────

function TopArtistsTab({ window }: { window: StatsWindow }) {
  const [items, setItems] = useState<TopArtist[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    setLoading(true);
    getTopArtists(window, 50)
      .then(setItems)
      .finally(() => setLoading(false));
  }, [window]);

  if (loading) return <Loader />;
  return (
    <RankedList
      empty="No top artists yet for this window."
      items={items.map((a, i) => ({
        key: `${a.artist}-${i}`,
        primary: a.artist,
        secondary: `${a.distinctTracks} ${a.distinctTracks === 1 ? "track" : "tracks"}`,
        count: a.plays,
        countLabel: a.plays === 1 ? "play" : "plays",
        time: a.listenedSecs,
      }))}
    />
  );
}

// ─── Top Albums ───────────────────────────────────────────────────────────

function TopAlbumsTab({ window }: { window: StatsWindow }) {
  const [items, setItems] = useState<TopAlbum[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    setLoading(true);
    getTopAlbums(window, 50)
      .then(setItems)
      .finally(() => setLoading(false));
  }, [window]);

  if (loading) return <Loader />;
  return (
    <RankedList
      empty="No top albums yet for this window."
      items={items.map((a, i) => ({
        key: `${a.album}|${a.artist}-${i}`,
        primary: a.album,
        secondary: a.artist,
        count: a.plays,
        countLabel: a.plays === 1 ? "play" : "plays",
        time: a.listenedSecs,
      }))}
    />
  );
}

// ─── Heatmap ──────────────────────────────────────────────────────────────

const DOW_LABELS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

function HeatmapTab({ window }: { window: StatsWindow }) {
  const [cells, setCells] = useState<HeatmapCell[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    setLoading(true);
    getListeningHeatmap(window)
      .then(setCells)
      .finally(() => setLoading(false));
  }, [window]);

  // Build a 7×24 grid keyed by dow, hour
  const { grid, max } = useMemo(() => {
    const g: number[][] = Array.from({ length: 7 }, () => Array(24).fill(0));
    let mx = 0;
    for (const c of cells) {
      if (c.dow >= 0 && c.dow < 7 && c.hour >= 0 && c.hour < 24) {
        g[c.dow][c.hour] = c.listenedSecs;
        if (c.listenedSecs > mx) mx = c.listenedSecs;
      }
    }
    return { grid: g, max: mx };
  }, [cells]);

  if (loading) return <Loader />;
  if (max === 0) {
    return (
      <div className="text-center py-16 text-th-text-muted text-[14px]">
        Not enough data for a heatmap yet.
      </div>
    );
  }

  return (
    <div>
      <div className="text-[12px] text-th-text-muted mb-3">
        Listening intensity by day of week and hour of day. Darker cells = more
        listening.
      </div>
      <div className="overflow-x-auto">
        <div className="inline-grid grid-cols-[auto_repeat(24,1fr)] gap-[2px] text-[10px]">
          <div></div>
          {Array.from({ length: 24 }).map((_, h) => (
            <div
              key={h}
              className="text-center text-th-text-faint font-medium"
              style={{ minWidth: 22 }}
            >
              {h % 3 === 0 ? h : ""}
            </div>
          ))}
          {DOW_LABELS.map((label, dow) => (
            <Row key={dow} label={label} cells={grid[dow]} max={max} />
          ))}
        </div>
      </div>
    </div>
  );
}

function Row({
  label,
  cells,
  max,
}: {
  label: string;
  cells: number[];
  max: number;
}) {
  return (
    <>
      <div className="pr-2 self-center text-th-text-muted font-medium">
        {label}
      </div>
      {cells.map((v, h) => {
        const intensity = v === 0 ? 0 : 0.15 + 0.85 * (v / max);
        const title = v
          ? `${label} ${h}:00 — ${formatDuration(v)}`
          : `${label} ${h}:00 — silent`;
        return (
          <div
            key={h}
            title={title}
            className="aspect-square rounded-sm bg-th-accent transition-colors"
            style={{ opacity: intensity || 0.06 }}
          />
        );
      })}
    </>
  );
}

// ─── Shared UI ─────────────────────────────────────────────────────────────

function Loader() {
  return (
    <div className="flex items-center justify-center py-16">
      <div className="h-6 w-6 animate-spin rounded-full border-2 border-th-accent border-t-transparent" />
    </div>
  );
}

interface RankedItem {
  key: string;
  primary: string;
  secondary: string;
  count: number;
  countLabel: string;
  time: number;
}

function RankedList({ items, empty }: { items: RankedItem[]; empty: string }) {
  if (items.length === 0) {
    return (
      <div className="text-center py-16 text-th-text-muted text-[14px]">
        {empty}
      </div>
    );
  }
  const max = items[0]?.count ?? 1;
  return (
    <ol className="space-y-1.5">
      {items.map((it, i) => {
        const pct = (it.count / max) * 100;
        return (
          <li
            key={it.key}
            className="relative flex items-center gap-3 rounded-md bg-th-surface px-3 py-2 overflow-hidden"
          >
            <div
              className="absolute inset-y-0 left-0 bg-th-accent/15"
              style={{ width: `${pct}%` }}
            />
            <span className="relative w-7 text-right text-[12px] font-bold text-th-text-faint tabular-nums">
              {i + 1}
            </span>
            <div className="relative flex-1 min-w-0">
              <div className="text-[13px] font-medium text-th-text-primary truncate">
                {it.primary}
              </div>
              <div className="text-[11px] text-th-text-muted truncate">
                {it.secondary}
              </div>
            </div>
            <div className="relative text-right shrink-0">
              <div className="text-[13px] font-bold text-th-text-primary tabular-nums">
                {formatNumber(it.count)}
              </div>
              <div className="text-[10px] text-th-text-muted">
                {it.countLabel} · {formatDuration(it.time)}
              </div>
            </div>
          </li>
        );
      })}
    </ol>
  );
}
