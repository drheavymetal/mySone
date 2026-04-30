// LRC parser. Lines look like `[mm:ss.xx]Text` or `[mm:ss]Text`. Enhanced LRC
// can have multiple timestamps per line for repeated lyrics like choruses:
// `[00:12.34][01:34.56]Repeating line`. Header lines like `[ar:Artist]` are
// metadata and skipped.

export interface LyricLine {
  /** seconds since track start when this line should be active */
  time: number;
  text: string;
}

export interface ParsedLyrics {
  /** sorted by time, ready to render */
  synced: LyricLine[];
  /** plain text fallback when no synced lyrics available */
  plain: string | null;
}

// Match `[mm:ss]` or `[mm:ss.xx]` or `[mm:ss.xxx]`
const TIMESTAMP_RE = /\[(\d{1,3}):(\d{2})(?:[.:](\d{1,3}))?\]/g;

function parseLrc(lrc: string): LyricLine[] {
  const out: LyricLine[] = [];
  for (const raw of lrc.split(/\r?\n/)) {
    const line = raw.trimEnd();
    if (!line) continue;
    // Capture all leading timestamps then the trailing text.
    const timestamps: number[] = [];
    TIMESTAMP_RE.lastIndex = 0;
    let lastEnd = 0;
    let m: RegExpExecArray | null;
    while ((m = TIMESTAMP_RE.exec(line))) {
      // Reject non-numeric "[ar:...]" header tags by checking groups.
      const minutes = parseInt(m[1], 10);
      const seconds = parseInt(m[2], 10);
      if (Number.isNaN(minutes) || Number.isNaN(seconds)) continue;
      let frac = 0;
      if (m[3] != null) {
        const fracStr = m[3];
        frac = parseInt(fracStr, 10) / 10 ** fracStr.length;
      }
      timestamps.push(minutes * 60 + seconds + frac);
      lastEnd = m.index + m[0].length;
    }
    if (timestamps.length === 0) continue;
    const text = line.slice(lastEnd).trim();
    for (const t of timestamps) {
      out.push({ time: t, text });
    }
  }
  out.sort((a, b) => a.time - b.time);
  return out;
}

export function parseLyrics(input: {
  subtitles?: string | null;
  lyrics?: string | null;
}): ParsedLyrics {
  const synced = input.subtitles ? parseLrc(input.subtitles) : [];
  // Sometimes TIDAL returns plain lyrics in the `subtitles` field too (no
  // timestamps). Detect by absence of any successfully parsed line.
  const plain =
    input.lyrics && input.lyrics.trim().length > 0
      ? input.lyrics
      : synced.length === 0 && input.subtitles
      ? input.subtitles
      : null;
  return { synced, plain };
}

/**
 * Binary-search the index of the line that should be highlighted at `time`.
 * Returns -1 when `time` is before the first line.
 */
export function findActiveLineIndex(lines: LyricLine[], time: number): number {
  if (lines.length === 0 || time < lines[0].time) return -1;
  let lo = 0;
  let hi = lines.length - 1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >>> 1;
    if (lines[mid].time <= time) lo = mid;
    else hi = mid - 1;
  }
  return lo;
}
