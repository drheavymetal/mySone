/**
 * StatsPage — local-only listening statistics.
 *
 * Reads from the SQLite stats DB the backend writes to on every play.
 * No network, no telemetry — privacy-first by design.
 */

import { useEffect, useMemo, useState } from "react";
import {
  Music,
  Users,
  Disc3,
  Clock,
  Flame,
  Activity,
  Crown,
  Headphones,
  ExternalLink,
} from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  getStatsOverview,
  getTopTracks,
  getTopArtists,
  getTopAlbums,
  getListeningHeatmap,
  getDailyMinutes,
  type StatsWindow,
  type StatsOverview,
  type TopTrack,
  type TopArtist,
  type TopAlbum,
  type HeatmapCell,
  type DailyMinutes,
} from "../api/stats";
import {
  getTrackCover,
  getAlbumCover,
  getArtistPicture,
} from "../api/coverLookup";
import { getTidalImageUrl } from "../types";
import PageContainer from "./PageContainer";

type CoverKind =
  | { kind: "track"; trackId: number | null; title: string; artist: string }
  | { kind: "album"; album: string; artist: string }
  | { kind: "artist"; artist: string };

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

// ─── Helpers ───────────────────────────────────────────────────────────────

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

/**
 * Build a MusicBrainz search URL for a given source. We search by name
 * because we don't carry MBIDs in the stats DB; the user lands on the
 * disambiguation page on MB and can click through.
 */
function musicBrainzUrlFor(source: CoverKind): string {
  const base = "https://musicbrainz.org/search";
  if (source.kind === "artist") {
    return `${base}?type=artist&query=${encodeURIComponent(source.artist)}`;
  }
  if (source.kind === "album") {
    return `${base}?type=release-group&query=${encodeURIComponent(
      `${source.album} AND artist:${source.artist}`,
    )}`;
  }
  return `${base}?type=recording&query=${encodeURIComponent(
    `${source.title} AND artist:${source.artist}`,
  )}`;
}

function MbLink({
  source,
  className,
}: {
  source: CoverKind;
  className?: string;
}) {
  const url = musicBrainzUrlFor(source);
  return (
    <button
      onClick={(e) => {
        e.stopPropagation();
        void openUrl(url);
      }}
      title="Open on MusicBrainz"
      className={
        "inline-flex items-center gap-0.5 rounded-md px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-wider text-th-text-faint hover:bg-th-bg-inset hover:text-th-text-primary transition-colors " +
        (className ?? "")
      }
    >
      MB
      <ExternalLink size={9} />
    </button>
  );
}

function hueFromString(s: string): number {
  let h = 0;
  for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) | 0;
  return (h >>> 0) % 360;
}

/** Generated linear-gradient for a name — used as avatar fill. */
function gradientFor(seed: string): string {
  const h1 = hueFromString(seed);
  const h2 = (h1 + 60) % 360;
  return `linear-gradient(135deg, hsl(${h1} 70% 55%) 0%, hsl(${h2} 75% 38%) 100%)`;
}

/** Initials for the avatar. Up to 2 letters from the first two words. */
function initialsFor(name: string): string {
  const parts = name
    .replace(/[^\p{L}\p{N}\s]/gu, " ")
    .trim()
    .split(/\s+/)
    .slice(0, 2);
  return parts.map((p) => p[0]?.toUpperCase() ?? "").join("") || "?";
}

// ─── Page ──────────────────────────────────────────────────────────────────

export default function StatsPage() {
  const [tab, setTab] = useState<Tab>("overview");
  const [window, setWindow] = useState<StatsWindow>("month");

  return (
    <PageContainer className="px-6 pt-6 pb-8">
      <header className="mb-6 flex flex-wrap items-end justify-between gap-4">
        <div>
          <div className="flex items-center gap-2 text-[11px] font-bold uppercase tracking-[0.2em] text-th-accent/80">
            <Activity size={12} strokeWidth={2.5} />
            <span>Local · Private</span>
          </div>
          <h1 className="mt-1 bg-gradient-to-br from-th-text-primary to-th-text-muted bg-clip-text text-[34px] font-extrabold leading-none text-transparent">
            Your listening
          </h1>
          <p className="mt-2 text-[12px] text-th-text-muted">
            Built from plays on this machine. Nothing leaves your laptop.
          </p>
        </div>
        <div className="flex gap-1 rounded-full border border-th-border-subtle bg-th-surface/60 p-1 backdrop-blur">
          {WINDOWS.map((w) => (
            <button
              key={w.id}
              onClick={() => setWindow(w.id)}
              className={`rounded-full px-4 py-1.5 text-[12px] font-semibold transition-all ${
                window === w.id
                  ? "bg-th-accent text-black shadow-[0_0_24px_-6px_var(--th-accent)]"
                  : "text-th-text-muted hover:text-th-text-primary"
              }`}
            >
              {w.label}
            </button>
          ))}
        </div>
      </header>

      <nav className="mb-6 flex gap-1 border-b border-th-border-subtle">
        {TABS.map((t) => (
          <button
            key={t.id}
            onClick={() => setTab(t.id)}
            className={`relative -mb-px px-3 py-2 text-[13px] font-semibold transition-colors ${
              tab === t.id
                ? "text-th-text-primary"
                : "text-th-text-muted hover:text-th-text-primary"
            }`}
          >
            {t.label}
            {tab === t.id && (
              <span className="absolute inset-x-3 -bottom-px h-[2px] rounded-full bg-th-accent shadow-[0_0_12px_var(--th-accent)]" />
            )}
          </button>
        ))}
      </nav>

      <div key={`${tab}-${window}`} className="stats-fade-in">
        {tab === "overview" && <OverviewTab window={window} />}
        {tab === "tracks" && <TopTracksTab window={window} />}
        {tab === "artists" && <TopArtistsTab window={window} />}
        {tab === "albums" && <TopAlbumsTab window={window} />}
        {tab === "heatmap" && <HeatmapTab window={window} />}
      </div>
    </PageContainer>
  );
}

// ─── Overview ──────────────────────────────────────────────────────────────

function OverviewTab({ window }: { window: StatsWindow }) {
  const [overview, setOverview] = useState<StatsOverview | null>(null);
  const [daily, setDaily] = useState<DailyMinutes[]>([]);
  const [topTrack, setTopTrack] = useState<TopTrack | null>(null);
  const [topArtist, setTopArtist] = useState<TopArtist | null>(null);
  const [topAlbum, setTopAlbum] = useState<TopAlbum | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    setLoading(true);
    Promise.all([
      getStatsOverview(window),
      getDailyMinutes(window),
      getTopTracks(window, 1),
      getTopArtists(window, 1),
      getTopAlbums(window, 1),
    ])
      .then(([ov, dm, tt, ta, tal]) => {
        setOverview(ov);
        setDaily(dm);
        setTopTrack(tt[0] ?? null);
        setTopArtist(ta[0] ?? null);
        setTopAlbum(tal[0] ?? null);
      })
      .finally(() => setLoading(false));
  }, [window]);

  if (loading) return <Loader />;
  if (!overview) return null;
  if (overview.totalPlays === 0) {
    return (
      <EmptyState
        title="Nothing to chart yet"
        body="Play something — your stats will start filling in instantly."
      />
    );
  }

  const completion = overview.totalPlays
    ? Math.round((overview.completedPlays / overview.totalPlays) * 100)
    : 0;

  return (
    <div className="space-y-6">
      <HeroCard
        listenedSecs={overview.totalListenedSecs}
        daily={daily}
        completion={completion}
        totalPlays={overview.totalPlays}
      />

      <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
        <StatTile
          icon={<Headphones size={16} />}
          label="Plays"
          value={formatNumber(overview.totalPlays)}
          sub={`${formatNumber(overview.completedPlays)} completed`}
          accent="hsl(180 70% 55%)"
        />
        <StatTile
          icon={<Music size={16} />}
          label="Tracks"
          value={formatNumber(overview.distinctTracks)}
          sub="distinct"
          accent="hsl(280 70% 65%)"
        />
        <StatTile
          icon={<Users size={16} />}
          label="Artists"
          value={formatNumber(overview.distinctArtists)}
          sub="distinct"
          accent="hsl(330 75% 60%)"
        />
        <StatTile
          icon={<Disc3 size={16} />}
          label="Albums"
          value={formatNumber(overview.distinctAlbums)}
          sub="distinct"
          accent="hsl(40 85% 60%)"
        />
      </div>

      {(topTrack || topArtist || topAlbum) && (
        <div>
          <SectionHeading
            icon={<Crown size={14} />}
            title="Crowned this window"
          />
          <div className="grid grid-cols-1 gap-3 md:grid-cols-3">
            {topTrack && (
              <PodiumCard
                rank="Top track"
                primary={topTrack.title}
                secondary={topTrack.artist}
                count={topTrack.plays}
                seed={`${topTrack.title}|${topTrack.artist}`}
                source={{
                  kind: "track",
                  trackId: topTrack.trackId,
                  title: topTrack.title,
                  artist: topTrack.artist,
                }}
              />
            )}
            {topArtist && (
              <PodiumCard
                rank="Top artist"
                primary={topArtist.artist}
                secondary={`${topArtist.distinctTracks} tracks`}
                count={topArtist.plays}
                seed={topArtist.artist}
                source={{ kind: "artist", artist: topArtist.artist }}
              />
            )}
            {topAlbum && (
              <PodiumCard
                rank="Top album"
                primary={topAlbum.album}
                secondary={topAlbum.artist}
                count={topAlbum.plays}
                seed={`${topAlbum.album}|${topAlbum.artist}`}
                source={{
                  kind: "album",
                  album: topAlbum.album,
                  artist: topAlbum.artist,
                }}
              />
            )}
          </div>
        </div>
      )}
    </div>
  );
}

function HeroCard({
  listenedSecs,
  daily,
  completion,
  totalPlays,
}: {
  listenedSecs: number;
  daily: DailyMinutes[];
  completion: number;
  totalPlays: number;
}) {
  const totalMinutes = Math.round(listenedSecs / 60);
  const peak = daily.reduce(
    (best, d) => (d.minutes > best.minutes ? d : best),
    daily[0] ?? { date: "", minutes: 0 },
  );
  return (
    <div className="relative overflow-hidden rounded-2xl border border-th-border-subtle bg-gradient-to-br from-th-surface to-th-bg-base p-6">
      <div
        className="pointer-events-none absolute -top-24 -right-16 h-64 w-64 rounded-full opacity-30 blur-3xl"
        style={{ background: "var(--th-accent)" }}
      />
      <div className="relative grid grid-cols-1 gap-6 md:grid-cols-[1fr_1.4fr]">
        <div className="flex flex-col justify-between gap-4">
          <div>
            <div className="flex items-center gap-2 text-[11px] font-bold uppercase tracking-[0.18em] text-th-text-muted">
              <Clock size={12} />
              <span>Time listened</span>
            </div>
            <div className="mt-2 flex items-baseline gap-2">
              <span className="text-[56px] font-black leading-none tracking-tight text-th-text-primary">
                {formatDuration(listenedSecs)}
              </span>
            </div>
            <div className="mt-1 text-[12px] text-th-text-muted">
              {formatNumber(totalMinutes)} minutes ·{" "}
              {formatNumber(totalPlays)} plays
            </div>
          </div>
          <div className="flex flex-wrap gap-3">
            <Pill label="Completion" value={`${completion}%`} />
            {peak.minutes > 0 && (
              <Pill
                label="Peak day"
                value={`${peak.minutes}m`}
                hint={peak.date}
              />
            )}
          </div>
        </div>
        <div>
          <div className="mb-2 flex items-center justify-between text-[10px] font-bold uppercase tracking-[0.18em] text-th-text-faint">
            <span>Daily minutes</span>
            <span>{daily.length} days</span>
          </div>
          <DailyAreaChart daily={daily} />
        </div>
      </div>
    </div>
  );
}

function Pill({
  label,
  value,
  hint,
}: {
  label: string;
  value: string;
  hint?: string;
}) {
  return (
    <div className="rounded-xl border border-th-border-subtle bg-th-bg-base/60 px-3 py-2">
      <div className="text-[9px] font-bold uppercase tracking-[0.2em] text-th-text-faint">
        {label}
      </div>
      <div className="text-[15px] font-bold text-th-text-primary">{value}</div>
      {hint && (
        <div className="text-[10px] text-th-text-muted tabular-nums">
          {hint}
        </div>
      )}
    </div>
  );
}

function DailyAreaChart({ daily }: { daily: DailyMinutes[] }) {
  const W = 600;
  const H = 140;
  const PAD_X = 6;
  const PAD_Y = 10;

  const { path, area, dotX, dotY, max } = useMemo(() => {
    if (daily.length === 0) {
      return { path: "", area: "", dotX: 0, dotY: 0, max: 0 };
    }
    const mx = daily.reduce((m, d) => Math.max(m, d.minutes), 0) || 1;
    const innerW = W - PAD_X * 2;
    const innerH = H - PAD_Y * 2;
    const stepX = daily.length > 1 ? innerW / (daily.length - 1) : 0;
    const points = daily.map((d, i) => {
      const x = PAD_X + i * stepX;
      const y = PAD_Y + innerH * (1 - d.minutes / mx);
      return [x, y] as const;
    });

    // Smooth path via Catmull-Rom → cubic Bézier.
    const d0 = points
      .map(([x, y], i) => {
        if (i === 0) return `M ${x.toFixed(2)} ${y.toFixed(2)}`;
        const [x0, y0] = points[i - 1];
        const [x1, y1] = points[i];
        const [x2, y2] = points[Math.min(i + 1, points.length - 1)];
        const [xm1, ym1] = points[Math.max(i - 2, 0)];
        const c1x = x0 + (x1 - xm1) / 6;
        const c1y = y0 + (y1 - ym1) / 6;
        const c2x = x1 - (x2 - x0) / 6;
        const c2y = y1 - (y2 - y0) / 6;
        return `C ${c1x.toFixed(2)} ${c1y.toFixed(2)}, ${c2x.toFixed(2)} ${c2y.toFixed(2)}, ${x.toFixed(2)} ${y.toFixed(2)}`;
      })
      .join(" ");
    const last = points[points.length - 1];
    const first = points[0];
    const a = `${d0} L ${last[0].toFixed(2)} ${(H - PAD_Y).toFixed(2)} L ${first[0].toFixed(2)} ${(H - PAD_Y).toFixed(2)} Z`;

    // Highlight latest non-zero day.
    let dotIdx = points.length - 1;
    for (let i = points.length - 1; i >= 0; i--) {
      if (daily[i].minutes > 0) {
        dotIdx = i;
        break;
      }
    }
    return {
      path: d0,
      area: a,
      dotX: points[dotIdx][0],
      dotY: points[dotIdx][1],
      max: mx,
    };
  }, [daily]);

  if (daily.length === 0 || max === 0) {
    return (
      <div className="flex h-[140px] items-center justify-center rounded-xl border border-th-border-subtle bg-th-bg-base/40 text-[11px] text-th-text-faint">
        No daily activity yet
      </div>
    );
  }

  return (
    <svg
      viewBox={`0 0 ${W} ${H}`}
      preserveAspectRatio="none"
      className="h-[140px] w-full"
    >
      <defs>
        <linearGradient id="stats-area" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor="var(--th-accent)" stopOpacity="0.55" />
          <stop offset="100%" stopColor="var(--th-accent)" stopOpacity="0" />
        </linearGradient>
        <linearGradient id="stats-line" x1="0" y1="0" x2="1" y2="0">
          <stop offset="0%" stopColor="var(--th-accent)" stopOpacity="0.85" />
          <stop offset="100%" stopColor="var(--th-accent)" stopOpacity="1" />
        </linearGradient>
      </defs>
      <path d={area} fill="url(#stats-area)" />
      <path
        d={path}
        fill="none"
        stroke="url(#stats-line)"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <circle cx={dotX} cy={dotY} r="6" fill="var(--th-accent)" opacity="0.2" />
      <circle cx={dotX} cy={dotY} r="3" fill="var(--th-accent)" />
    </svg>
  );
}

function StatTile({
  icon,
  label,
  value,
  sub,
  accent,
}: {
  icon: React.ReactNode;
  label: string;
  value: string;
  sub: string;
  accent: string;
}) {
  return (
    <div className="group relative overflow-hidden rounded-xl border border-th-border-subtle bg-th-surface/80 p-4 transition-colors hover:border-th-accent/40">
      <div
        className="pointer-events-none absolute -top-8 -right-8 h-24 w-24 rounded-full opacity-20 blur-2xl transition-opacity group-hover:opacity-40"
        style={{ background: accent }}
      />
      <div
        className="mb-3 inline-flex h-7 w-7 items-center justify-center rounded-lg"
        style={{ background: `${accent}20`, color: accent }}
      >
        {icon}
      </div>
      <div className="text-[10px] font-bold uppercase tracking-[0.18em] text-th-text-faint">
        {label}
      </div>
      <div className="mt-1 text-[24px] font-extrabold leading-none text-th-text-primary tabular-nums">
        {value}
      </div>
      <div className="mt-1 text-[11px] text-th-text-muted">{sub}</div>
    </div>
  );
}

function SectionHeading({
  icon,
  title,
}: {
  icon: React.ReactNode;
  title: string;
}) {
  return (
    <div className="mb-3 flex items-center gap-2">
      <span className="text-th-accent">{icon}</span>
      <h2 className="text-[12px] font-bold uppercase tracking-[0.2em] text-th-text-muted">
        {title}
      </h2>
      <span className="ml-2 h-px flex-1 bg-gradient-to-r from-th-border-subtle to-transparent" />
    </div>
  );
}

function PodiumCard({
  rank,
  primary,
  secondary,
  count,
  seed,
  source,
}: {
  rank: string;
  primary: string;
  secondary: string;
  count: number;
  seed: string;
  source: CoverKind;
}) {
  return (
    <div className="group relative flex items-center gap-3 overflow-hidden rounded-xl border border-th-border-subtle bg-th-surface/80 p-3 transition-colors hover:border-th-accent/40">
      <CoverArt
        source={source}
        seed={seed}
        label={primary}
        size={56}
        rounded={source.kind === "artist" ? "rounded-full" : "rounded-lg"}
      />
      <div className="min-w-0 flex-1">
        <div className="text-[9px] font-bold uppercase tracking-[0.2em] text-th-accent/80">
          {rank}
        </div>
        <div className="truncate text-[14px] font-bold text-th-text-primary">
          {primary}
        </div>
        <div className="truncate text-[11px] text-th-text-muted">
          {secondary}
        </div>
      </div>
      <div className="flex flex-col items-end gap-1">
        <div className="text-[18px] font-extrabold leading-none text-th-text-primary tabular-nums">
          {formatNumber(count)}
        </div>
        <div className="text-[9px] font-semibold uppercase tracking-wider text-th-text-faint">
          plays
        </div>
        <MbLink source={source} className="opacity-0 group-hover:opacity-100" />
      </div>
    </div>
  );
}

/**
 * Renders an album cover or artist photo with a name-derived gradient
 * placeholder underneath. The gradient (with initials) shows immediately
 * so the layout is stable; the real image streams in once the lookup
 * resolves and fades in on top.
 */
function CoverArt({
  source,
  seed,
  label,
  size,
  rounded = "rounded-lg",
}: {
  source: CoverKind;
  seed: string;
  label: string;
  size: number;
  rounded?: string;
}) {
  const [url, setUrl] = useState<string | null>(null);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    let cancelled = false;
    setUrl(null);
    setLoaded(false);
    let p: Promise<string | null>;
    if (source.kind === "artist") {
      p = getArtistPicture(source.artist);
    } else if (source.kind === "album") {
      p = getAlbumCover(source.album, source.artist);
    } else {
      p = getTrackCover(source.trackId, source.title, source.artist);
    }
    p.then((uuid) => {
      if (cancelled) return;
      if (uuid) setUrl(getTidalImageUrl(uuid, size <= 80 ? 80 : 160));
    }).catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [source, size]);

  return (
    <div
      className={`relative flex shrink-0 items-center justify-center overflow-hidden text-white shadow-[inset_0_1px_0_rgba(255,255,255,0.15),0_4px_12px_-4px_rgba(0,0,0,0.5)] ${rounded}`}
      style={{
        width: size,
        height: size,
        background: gradientFor(seed),
        fontSize: size * 0.3,
        fontWeight: 800,
        letterSpacing: "-0.02em",
      }}
    >
      <span className="opacity-90">{initialsFor(label)}</span>
      {url && (
        <img
          src={url}
          alt=""
          loading="lazy"
          decoding="async"
          onLoad={() => setLoaded(true)}
          className="absolute inset-0 h-full w-full object-cover transition-opacity duration-300"
          style={{ opacity: loaded ? 1 : 0 }}
        />
      )}
    </div>
  );
}

// ─── Top X tabs ────────────────────────────────────────────────────────────

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
        seed: `${t.title}|${t.artist}`,
        source: {
          kind: "track" as const,
          trackId: t.trackId,
          title: t.title,
          artist: t.artist,
        },
        count: t.plays,
        countLabel: t.plays === 1 ? "play" : "plays",
        time: t.listenedSecs,
      }))}
    />
  );
}

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
        seed: a.artist,
        source: { kind: "artist" as const, artist: a.artist },
        count: a.plays,
        countLabel: a.plays === 1 ? "play" : "plays",
        time: a.listenedSecs,
      }))}
    />
  );
}

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
        seed: `${a.album}|${a.artist}`,
        source: { kind: "album" as const, album: a.album, artist: a.artist },
        count: a.plays,
        countLabel: a.plays === 1 ? "play" : "plays",
        time: a.listenedSecs,
      }))}
    />
  );
}

// ─── Heatmap ──────────────────────────────────────────────────────────────

const DOW_LABELS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

/** Color ramp from cool (low) → hot (high). Returns CSS color. */
function heatColor(t: number): string {
  // t in [0,1]. Hue from 215 (cool blue) → 320 (magenta) → 25 (orange/red).
  if (t <= 0) return "rgba(255,255,255,0.04)";
  // Two-stop interpolation through magenta for a more "rompedor" gradient.
  let hue: number;
  if (t < 0.5) {
    const k = t / 0.5;
    hue = 215 + (320 - 215) * k;
  } else {
    const k = (t - 0.5) / 0.5;
    hue = 320 + (25 - 320) * k;
    if (hue < 0) hue += 360;
  }
  const sat = 70 + 20 * t;
  const light = 38 + 22 * t;
  return `hsl(${hue.toFixed(0)} ${sat.toFixed(0)}% ${light.toFixed(0)}%)`;
}

function HeatmapTab({ window }: { window: StatsWindow }) {
  const [cells, setCells] = useState<HeatmapCell[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    setLoading(true);
    getListeningHeatmap(window)
      .then(setCells)
      .finally(() => setLoading(false));
  }, [window]);

  const { grid, max, peak, byDow, byHour, totalSecs } = useMemo(() => {
    const g: number[][] = Array.from({ length: 7 }, () => Array(24).fill(0));
    let mx = 0;
    let pk = { dow: 0, hour: 0, secs: 0 };
    const dowSum = Array(7).fill(0);
    const hourSum = Array(24).fill(0);
    let total = 0;
    for (const c of cells) {
      if (c.dow >= 0 && c.dow < 7 && c.hour >= 0 && c.hour < 24) {
        g[c.dow][c.hour] = c.listenedSecs;
        dowSum[c.dow] += c.listenedSecs;
        hourSum[c.hour] += c.listenedSecs;
        total += c.listenedSecs;
        if (c.listenedSecs > mx) mx = c.listenedSecs;
        if (c.listenedSecs > pk.secs) {
          pk = { dow: c.dow, hour: c.hour, secs: c.listenedSecs };
        }
      }
    }
    return {
      grid: g,
      max: mx,
      peak: pk,
      byDow: dowSum,
      byHour: hourSum,
      totalSecs: total,
    };
  }, [cells]);

  if (loading) return <Loader />;
  if (max === 0) {
    return (
      <EmptyState
        title="No heatmap yet"
        body="Once you have a few sessions, your week starts lighting up here."
      />
    );
  }

  const peakLabel = `${DOW_LABELS[peak.dow]} ${peak.hour
    .toString()
    .padStart(2, "0")}:00`;

  return (
    <div className="space-y-5">
      <div className="grid grid-cols-1 gap-3 md:grid-cols-3">
        <StatTile
          icon={<Flame size={16} />}
          label="Peak slot"
          value={peakLabel}
          sub={formatDuration(peak.secs)}
          accent="hsl(20 90% 60%)"
        />
        <StatTile
          icon={<Activity size={16} />}
          label="Total"
          value={formatDuration(totalSecs)}
          sub={`across ${DOW_LABELS.length} days × 24 hours`}
          accent="hsl(180 70% 55%)"
        />
        <StatTile
          icon={<Clock size={16} />}
          label="Top hour"
          value={`${argmax(byHour).toString().padStart(2, "0")}:00`}
          sub="most listened hour of day"
          accent="hsl(280 70% 65%)"
        />
      </div>

      <div className="rounded-2xl border border-th-border-subtle bg-th-surface/60 p-4">
        <div className="mb-3 flex items-center justify-between text-[11px] text-th-text-muted">
          <span>Listening intensity by day × hour</span>
          <Legend />
        </div>
        <div className="overflow-x-auto">
          <div className="inline-grid min-w-full grid-cols-[auto_repeat(24,1fr)_auto] gap-[3px] text-[10px]">
            <div />
            {Array.from({ length: 24 }).map((_, h) => (
              <div
                key={h}
                className="text-center font-medium text-th-text-faint"
                style={{ minWidth: 22 }}
              >
                {h % 3 === 0 ? h : ""}
              </div>
            ))}
            <div />
            {DOW_LABELS.map((label, dow) => (
              <HeatRow
                key={dow}
                label={label}
                cells={grid[dow]}
                max={max}
                rowTotal={byDow[dow]}
                rowMax={Math.max(...byDow)}
                isPeakRow={dow === peak.dow}
              />
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}

function argmax(arr: number[]): number {
  let best = 0;
  for (let i = 1; i < arr.length; i++) if (arr[i] > arr[best]) best = i;
  return best;
}

function Legend() {
  return (
    <div className="flex items-center gap-2">
      <span className="text-[10px] text-th-text-faint">less</span>
      <div className="flex h-2 w-32 overflow-hidden rounded-full">
        {Array.from({ length: 24 }).map((_, i) => (
          <div
            key={i}
            className="h-full flex-1"
            style={{ background: heatColor((i + 1) / 24) }}
          />
        ))}
      </div>
      <span className="text-[10px] text-th-text-faint">more</span>
    </div>
  );
}

function HeatRow({
  label,
  cells,
  max,
  rowTotal,
  rowMax,
  isPeakRow,
}: {
  label: string;
  cells: number[];
  max: number;
  rowTotal: number;
  rowMax: number;
  isPeakRow: boolean;
}) {
  const rowPct = rowMax ? (rowTotal / rowMax) * 100 : 0;
  return (
    <>
      <div
        className={`self-center pr-2 font-semibold ${
          isPeakRow ? "text-th-accent" : "text-th-text-muted"
        }`}
      >
        {label}
      </div>
      {cells.map((v, h) => {
        const t = v === 0 ? 0 : v / max;
        const title = v
          ? `${label} ${h}:00 — ${formatDuration(v)}`
          : `${label} ${h}:00 — silent`;
        return (
          <div
            key={h}
            title={title}
            className="aspect-square rounded-[3px] transition-transform hover:scale-110"
            style={{
              background: heatColor(t),
              boxShadow: t > 0.85 ? `0 0 10px ${heatColor(t)}` : undefined,
            }}
          />
        );
      })}
      <div className="self-center pl-2">
        <div
          className="h-1 w-12 overflow-hidden rounded-full bg-th-border-subtle/40"
          title={formatDuration(rowTotal)}
        >
          <div
            className="h-full rounded-full bg-gradient-to-r from-th-accent/60 to-th-accent"
            style={{ width: `${rowPct}%` }}
          />
        </div>
      </div>
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

function EmptyState({ title, body }: { title: string; body: string }) {
  return (
    <div className="rounded-2xl border border-dashed border-th-border-subtle py-16 text-center">
      <div className="text-[14px] font-bold text-th-text-primary">{title}</div>
      <div className="mt-1 text-[12px] text-th-text-muted">{body}</div>
    </div>
  );
}

interface RankedItem {
  key: string;
  primary: string;
  secondary: string;
  seed: string;
  source: CoverKind;
  count: number;
  countLabel: string;
  time: number;
}

function RankedList({ items, empty }: { items: RankedItem[]; empty: string }) {
  if (items.length === 0) {
    return <EmptyState title={empty} body="Try a wider time window." />;
  }
  const max = items[0]?.count ?? 1;
  return (
    <ol className="space-y-1.5">
      {items.map((it, i) => (
        <RankedRow key={it.key} item={it} index={i} max={max} />
      ))}
    </ol>
  );
}

const MEDAL_COLORS: Record<number, string> = {
  0: "linear-gradient(135deg, #FFD46B 0%, #C68A14 100%)",
  1: "linear-gradient(135deg, #E8E8EC 0%, #9CA0AB 100%)",
  2: "linear-gradient(135deg, #D69060 0%, #8B5320 100%)",
};

function RankedRow({
  item,
  index,
  max,
}: {
  item: RankedItem;
  index: number;
  max: number;
}) {
  const pct = (item.count / max) * 100;
  const medal = MEDAL_COLORS[index];
  return (
    <li className="group relative flex items-center gap-3 overflow-hidden rounded-xl border border-th-border-subtle bg-th-surface/70 px-3 py-2.5 transition-all hover:border-th-accent/40 hover:bg-th-surface">
      <div
        className="absolute inset-y-0 left-0 bg-gradient-to-r from-th-accent/15 via-th-accent/5 to-transparent transition-opacity group-hover:opacity-80"
        style={{ width: `${pct}%`, opacity: 0.6 }}
      />
      <div
        className="relative flex h-7 w-7 shrink-0 items-center justify-center rounded-full text-[12px] font-extrabold tabular-nums shadow-sm"
        style={
          medal
            ? { background: medal, color: "#1a1408" }
            : {
                background: "var(--th-bg-inset)",
                color: "var(--th-text-faint)",
              }
        }
      >
        {index + 1}
      </div>
      <CoverArt
        source={item.source}
        seed={item.seed}
        label={item.primary}
        size={42}
        rounded={item.source.kind === "artist" ? "rounded-full" : "rounded-lg"}
      />
      <div className="relative min-w-0 flex-1">
        <div className="truncate text-[13px] font-bold text-th-text-primary">
          {item.primary}
        </div>
        <div className="truncate text-[11px] text-th-text-muted">
          {item.secondary}
        </div>
      </div>
      <div className="relative shrink-0 flex flex-col items-end gap-1">
        <div className="text-[14px] font-extrabold tabular-nums text-th-text-primary">
          {formatNumber(item.count)}
        </div>
        <div className="text-[10px] text-th-text-muted">
          {item.countLabel} · {formatDuration(item.time)}
        </div>
        <MbLink
          source={item.source}
          className="opacity-0 group-hover:opacity-100"
        />
      </div>
    </li>
  );
}
