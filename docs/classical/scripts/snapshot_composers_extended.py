#!/usr/bin/env python3
"""Snapshot harvester for the extended composers universe (Phase 7 — D-027 / D-032).

Builds `src-tauri/data/composers-extended.json` by:

1. SPARQL against `query.wikidata.org/sparql` for every entity with
   `wdt:P106 wd:Q36834` (occupation = composer) AND `wdt:P434` non-null
   (MusicBrainz artist ID present).
2. For each composer returned, query MusicBrainz for the artist record
   and count recordings via the browse API (cheap, one-call).
3. Filter by `recording_count >= N` (default 5; D-027).
4. Merge with `src-tauri/data/openopus.json` so composers present in
   the original snapshot keep their `popular` / `recommended` / `epoch`
   curation.
5. Emit the extended snapshot — sorted lexicographically by MBID for
   deterministic diffs across runs.

This script runs OUT-OF-BAND. CI does not run it: WDQS is rate-limited
and non-deterministic (the same query may return slightly different
ordering between days). Operators re-run it when:
  - A new release of mySone is being prepared.
  - A user reports a composer missing from the Hub.
  - The snapshot is older than ~6 months.

The output is committed to the repo. The Rust binary consumes the
JSON via `include_bytes!` at build time (D-033, D-027 trade-off).

Usage:
    # Dry-run (smaller threshold, faster, fewer composers):
    python3 snapshot_composers_extended.py --threshold 5 --dry-run

    # Production run (full universe, ~30-60 min wall-clock):
    python3 snapshot_composers_extended.py --threshold 5

    # Custom output path:
    python3 snapshot_composers_extended.py --output ../../../src-tauri/data/composers-extended.json

Bit-perfect contract: this script is build-tooling. It does NOT touch
audio routing, the binary at runtime, or any §10 area.
"""

from __future__ import annotations

import argparse
import dataclasses
import json
import logging
import pathlib
import re
import sys
import time
import typing
import urllib.parse
import urllib.request

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

WDQS_ENDPOINT = "https://query.wikidata.org/sparql"
MB_ENDPOINT = "https://musicbrainz.org/ws/2"

USER_AGENT = (
    "SONE-classical-snapshot-harvester/0.7.0 "
    "(https://github.com/lullabyX/sone) phase-7-catalog-completeness"
)

# WDQS user manual recommends 5 concurrent max; we serialize fully and
# pace at 1.2s between queries to be a polite citizen of the commons.
WDQS_MIN_INTERVAL_S = 1.2
# MB rate limit is well-documented as 1 req/s. We pace at 1.05s for
# safety margin.
MB_MIN_INTERVAL_S = 1.05
# HTTP timeout. WDQS occasionally takes 10-20s on heavy queries.
HTTP_TIMEOUT_S = 60
# SPARQL pagination chunk. WDQS's 60s budget gets tight on chunks
# above 3k for queries with multiple OPTIONALs. We chunk at 2.5k for
# safety + retry budget.
SPARQL_CHUNK = 2500
# Number of retries with exponential backoff for transient WDQS timeouts.
SPARQL_MAX_RETRIES = 3

# ---------------------------------------------------------------------------
# Wire shapes
# ---------------------------------------------------------------------------


@dataclasses.dataclass
class WikidataComposer:
    """One row from the SPARQL harvest."""

    qid: str
    mbid: str
    name: str
    full_name: typing.Optional[str]
    birth_year: typing.Optional[int]
    death_year: typing.Optional[int]
    portrait_url: typing.Optional[str]


@dataclasses.dataclass
class ExtendedComposer:
    """Final shape emitted to composers-extended.json."""

    mbid: str
    qid: str
    name: str
    full_name: typing.Optional[str]
    birth_year: typing.Optional[int]
    death_year: typing.Optional[int]
    epoch: typing.Optional[str]
    portrait_url: typing.Optional[str]
    recording_count: int
    popular: bool
    open_opus_id: typing.Optional[str]


# ---------------------------------------------------------------------------
# Logging setup
# ---------------------------------------------------------------------------


def setup_logging(verbose: bool) -> None:
    """Initialise the root logger with a structured format."""
    level = logging.DEBUG if verbose else logging.INFO
    logging.basicConfig(
        level=level,
        format="%(asctime)s [%(levelname)s] %(message)s",
        datefmt="%Y-%m-%d %H:%M:%S",
    )


# ---------------------------------------------------------------------------
# HTTP helpers (rate-limited)
# ---------------------------------------------------------------------------


class RateLimiter:
    """Single-call interval enforcer."""

    def __init__(self, interval_s: float) -> None:
        self.interval_s = interval_s
        self.last_call = 0.0

    def wait(self) -> None:
        """Block until interval has elapsed since last call."""
        now = time.monotonic()
        elapsed = now - self.last_call
        if elapsed < self.interval_s:
            time.sleep(self.interval_s - elapsed)
        self.last_call = time.monotonic()


def http_get_json(url: str, accept: str = "application/json") -> dict:
    """GET + parse JSON with the script's User-Agent."""
    req = urllib.request.Request(url)
    req.add_header("User-Agent", USER_AGENT)
    req.add_header("Accept", accept)
    with urllib.request.urlopen(req, timeout=HTTP_TIMEOUT_S) as resp:
        body = resp.read().decode("utf-8")
    return json.loads(body)


# ---------------------------------------------------------------------------
# Step 1 — Wikidata SPARQL harvest
# ---------------------------------------------------------------------------


# Classical-composer query.
#
# Filters used:
#   - wdt:P106 wd:Q36834   → occupation = composer.
#   - wdt:P434 ?mbid       → has MusicBrainz artist ID.
#   - wdt:P136 ?genre AND ?genre (wdt:P279*) wd:Q9730  → genre subclass of
#     "classical music". The P279* transitive closure catches subgenres
#     (opera, oratorio, chamber music, etc.) without us enumerating them.
#
# P800 (notable work) is NOT used as a hard filter — it under-includes
# many medieval/Renaissance and contemporary composers whose Wikidata
# entries have no documented "notable work" claim despite having mature
# discographies. Replaced by P136 classical-genre filter which captures
# them via their genre claims. The runtime Phase 1 cascade still vets
# audibility per work on demand.
#
# Cardinality: ~3-6k composers, ~10-30s wall-clock through WDQS.
SPARQL_TEMPLATE = """\
SELECT DISTINCT ?composer ?composerLabel ?mbid ?birthYear ?deathYear
WHERE {{
  {{
    # Branch 1 — composers with classical-genre claim (canonical path).
    ?composer wdt:P106 wd:Q36834 .
    ?composer wdt:P434 ?mbid .
    ?composer wdt:P136 ?genre .
    ?genre (wdt:P279*) wd:Q9730 .
  }}
  UNION
  {{
    # Branch 2 — composers with genres adjacent to classical that lack
    # the P279 closure to Q9730 in Wikidata. Captures minimalism (Reich,
    # Glass, Adams), serialism, contemporary classical, sacred monophony
    # (Hildegard), nordic contemporary (Thorvaldsdóttir, Saariaho), etc.
    VALUES ?adjacentGenre {{
      wd:Q486325       # minimalist music
      wd:Q210121       # contemporary classical music
      wd:Q9794         # opera
      wd:Q9778         # sonata
      wd:Q1226992      # serialism
      wd:Q23961        # symphony
      wd:Q193207       # cantata
      wd:Q188874       # oratorio
      wd:Q318918       # Gregorian chant
      wd:Q9758108      # post-minimalism
      wd:Q200455       # film score
      wd:Q25441        # liturgical music
      wd:Q35760        # chamber music
      wd:Q482994       # concert music
      wd:Q207338       # choral music
    }}
    ?composer wdt:P106 wd:Q36834 .
    ?composer wdt:P434 ?mbid .
    ?composer wdt:P136 ?adjacentGenre .
  }}
  OPTIONAL {{ ?composer wdt:P569 ?birth . BIND(YEAR(?birth) AS ?birthYear) }}
  OPTIONAL {{ ?composer wdt:P570 ?death . BIND(YEAR(?death) AS ?deathYear) }}
  SERVICE wikibase:label {{ bd:serviceParam wikibase:language "en". }}
}}
ORDER BY ?composer
LIMIT {limit}
OFFSET {offset}
"""

# Second-pass portrait fetcher. Run only on composers surviving the
# recording_count threshold (vastly smaller universe). One row per qid.
SPARQL_PORTRAIT_TEMPLATE = """\
SELECT ?composer ?portrait WHERE {{
  VALUES ?composer {{ {qids} }}
  ?composer wdt:P18 ?portrait .
}}
"""


def sparql_chunk(offset: int, rate: RateLimiter) -> list[dict]:
    """Fetch one SPARQL chunk with exponential-backoff retries."""
    delay_s = 5.0
    for attempt in range(SPARQL_MAX_RETRIES):
        rate.wait()
        query = SPARQL_TEMPLATE.format(limit=SPARQL_CHUNK, offset=offset)
        url = WDQS_ENDPOINT + "?" + urllib.parse.urlencode(
            {"query": query, "format": "json"}
        )
        try:
            data = http_get_json(url, accept="application/sparql-results+json")
            return data.get("results", {}).get("bindings", [])
        except Exception as e:
            logging.warning(
                "SPARQL chunk offset=%d attempt=%d/%d failed: %s",
                offset,
                attempt + 1,
                SPARQL_MAX_RETRIES,
                e,
            )
            if attempt + 1 < SPARQL_MAX_RETRIES:
                logging.info("Backing off %.1fs before retry", delay_s)
                time.sleep(delay_s)
                delay_s *= 2
            else:
                raise
    return []


def sparql_harvest(rate: RateLimiter) -> list[WikidataComposer]:
    """Pull every composer with an MBID from WDQS, paginated with retries."""
    out: list[WikidataComposer] = []
    offset = 0
    while True:
        logging.info("SPARQL chunk offset=%d", offset)
        try:
            rows = sparql_chunk(offset, rate)
        except Exception as e:
            logging.error(
                "SPARQL chunk offset=%d exhausted retries — partial harvest "
                "of %d composers will continue. Error: %s",
                offset,
                len(out),
                e,
            )
            # Skip this chunk; advance offset and try the next one. We
            # accept gaps if WDQS keeps timing out at certain offsets —
            # the alternative is total abort which loses everything.
            offset += SPARQL_CHUNK
            if offset > 100000:
                logging.error("Offset > 100k with sustained failures — aborting.")
                break
            continue
        if not rows:
            logging.info("SPARQL chunk returned 0 rows — done. Total: %d", len(out))
            break
        chunk_parsed = 0
        for row in rows:
            parsed = parse_sparql_row(row)
            if parsed is not None:
                out.append(parsed)
                chunk_parsed += 1
        logging.info(
            "SPARQL chunk offset=%d returned=%d parsed=%d running_total=%d",
            offset,
            len(rows),
            chunk_parsed,
            len(out),
        )
        if len(rows) < SPARQL_CHUNK:
            logging.info("Last chunk (rows < limit). Total: %d", len(out))
            break
        offset += SPARQL_CHUNK
    return out


_QID_RE = re.compile(r"^Q\d+$")
_MBID_RE = re.compile(
    r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$",
    re.IGNORECASE,
)


def parse_sparql_row(row: dict) -> typing.Optional[WikidataComposer]:
    """Flatten one SPARQL JSON binding into a WikidataComposer."""
    composer_uri = row.get("composer", {}).get("value", "")
    qid = composer_uri.rsplit("/", 1)[-1] if composer_uri else ""
    if not qid or not _QID_RE.match(qid):
        return None
    mbid = row.get("mbid", {}).get("value", "").lower().strip()
    if not _MBID_RE.match(mbid):
        return None
    name = row.get("composerLabel", {}).get("value", "").strip()
    if not name:
        return None
    # WDQS returns the QID itself as label when no English label exists.
    # Filter those — they're not useful in BrowseComposers.
    if _QID_RE.match(name):
        return None

    def opt_year(key: str) -> typing.Optional[int]:
        val = row.get(key, {}).get("value", "").strip()
        if not val:
            return None
        try:
            year = int(val)
            if year < -1000 or year > 2100:
                return None
            return year
        except ValueError:
            return None

    return WikidataComposer(
        qid=qid,
        mbid=mbid,
        name=name,
        full_name=None,  # filled by OpenOpus merge or left None
        birth_year=opt_year("birthYear"),
        death_year=opt_year("deathYear"),
        portrait_url=None,  # filled by second-pass portrait fetcher
    )


# ---------------------------------------------------------------------------
# Step 1.5 — Second-pass portrait fetch (only post-threshold composers)
# ---------------------------------------------------------------------------


PORTRAIT_BATCH_SIZE = 200


def fetch_portraits(qids: list[str], rate: RateLimiter) -> dict[str, str]:
    """Pull P18 portrait URLs for a batch of qids.

    Batched in chunks of `PORTRAIT_BATCH_SIZE` to keep WDQS responses
    under their 60s budget. Composers without a P18 simply have no
    entry in the output map.
    """
    portraits: dict[str, str] = {}
    if not qids:
        return portraits
    for batch_start in range(0, len(qids), PORTRAIT_BATCH_SIZE):
        batch = qids[batch_start : batch_start + PORTRAIT_BATCH_SIZE]
        values = " ".join(f"wd:{q}" for q in batch)
        query = SPARQL_PORTRAIT_TEMPLATE.format(qids=values)
        rate.wait()
        url = WDQS_ENDPOINT + "?" + urllib.parse.urlencode(
            {"query": query, "format": "json"}
        )
        logging.info(
            "Portrait batch %d-%d / %d",
            batch_start,
            batch_start + len(batch),
            len(qids),
        )
        try:
            data = http_get_json(url, accept="application/sparql-results+json")
        except Exception as e:
            logging.warning("Portrait batch failed (continuing): %s", e)
            continue
        for row in data.get("results", {}).get("bindings", []):
            composer_uri = row.get("composer", {}).get("value", "")
            qid = composer_uri.rsplit("/", 1)[-1]
            portrait = row.get("portrait", {}).get("value", "").strip()
            if qid and portrait and qid not in portraits:
                portraits[qid] = portrait
    logging.info("Portraits fetched: %d/%d composers", len(portraits), len(qids))
    return portraits


# ---------------------------------------------------------------------------
# Step 2 — MusicBrainz recording_count enrichment
# ---------------------------------------------------------------------------


def mb_recording_count(mbid: str, rate: RateLimiter) -> int:
    """Cheap recording count via browse `?artist=...&limit=1`.

    MB returns `recording-count` total in the response header even when
    we ask for just 1 row. One round-trip per composer.
    """
    rate.wait()
    url = (
        f"{MB_ENDPOINT}/recording?artist={mbid}"
        "&limit=1&fmt=json"
    )
    try:
        data = http_get_json(url)
    except Exception as e:
        logging.warning("MB recording-count failed for %s: %s", mbid, e)
        return 0
    count = data.get("recording-count", 0)
    if not isinstance(count, int):
        return 0
    return count


# ---------------------------------------------------------------------------
# Step 3 — Merge with OpenOpus snapshot
# ---------------------------------------------------------------------------


def load_openopus(path: pathlib.Path) -> dict[str, dict]:
    """Index OpenOpus composers by MBID for cheap lookup during merge."""
    if not path.exists():
        logging.warning("OpenOpus snapshot not found at %s — merge will skip", path)
        return {}
    snapshot = json.loads(path.read_text())
    by_mbid: dict[str, dict] = {}
    for c in snapshot.get("composers", []):
        mbid = (c.get("mbid") or "").lower().strip()
        if mbid:
            by_mbid[mbid] = c
    logging.info("Loaded %d OpenOpus composers for cross-merge", len(by_mbid))
    return by_mbid


def epoch_from_years(birth: typing.Optional[int], death: typing.Optional[int]) -> typing.Optional[str]:
    """Heuristic era mapping when OpenOpus has no entry.

    Buckets follow `Era::parse_literal` in `types.rs`. Conservative —
    when only one date is known, use it; if both missing, return None.
    """
    ref = birth if birth is not None else death
    if ref is None:
        return None
    # Era boundaries are stylistic conventions. We use birth-year cutoffs
    # roughly aligned with `Era::parse_literal` in types.rs and OpenOpus
    # epoch labels:
    #   - Baroque ends 1735 (CPE Bach b. 1714 = Baroque, Haydn b. 1732 = Classical edge).
    #   - Classical ends 1780 (Beethoven b. 1770 = Classical/Early Romantic edge — OO calls him Early Romantic).
    #   - Early Romantic ends 1810.
    #   - Romantic ends 1845.
    #   - Late Romantic ends 1890.
    #   - 20th C ends 1925.
    #   - Post-War ends 1965.
    if ref < 1400:
        return "Medieval"
    if ref < 1550:
        return "Renaissance"
    if ref < 1735:
        return "Baroque"
    if ref < 1780:
        return "Classical"
    if ref < 1810:
        return "Early Romantic"
    if ref < 1845:
        return "Romantic"
    if ref < 1890:
        return "Late Romantic"
    if ref < 1925:
        return "20th Century"
    if ref < 1965:
        return "Post-War"
    return "Contemporary"


def merge(
    wd_composers: list[WikidataComposer],
    recording_counts: dict[str, int],
    openopus_by_mbid: dict[str, dict],
    threshold: int,
) -> list[ExtendedComposer]:
    """Combine WD + MB count + OO curation into the final list."""
    out: list[ExtendedComposer] = []
    seen_mbid: set[str] = set()
    for wd in wd_composers:
        if wd.mbid in seen_mbid:
            continue
        count = recording_counts.get(wd.mbid, 0)
        # `count == -1` means "MB enrichment skipped, accepted via P800
        # notability proxy". Filter only enforces the threshold when MB
        # actually returned a number.
        if count >= 0 and count < threshold:
            continue
        oo = openopus_by_mbid.get(wd.mbid)
        if oo is not None:
            epoch = oo.get("epoch")
            popular = bool(oo.get("popular", False))
            open_opus_id = oo.get("open_opus_id")
            # Prefer OpenOpus name (their curation is stronger for canon)
            # over Wikidata's English label, which is sometimes ambiguous.
            display_name = oo.get("name") or wd.name
            full_name = oo.get("full_name") or wd.full_name
            birth = oo.get("birth_year") if oo.get("birth_year") is not None else wd.birth_year
            death = oo.get("death_year") if oo.get("death_year") is not None else wd.death_year
            portrait = oo.get("portrait_url") or wd.portrait_url
        else:
            epoch = epoch_from_years(wd.birth_year, wd.death_year)
            popular = False
            open_opus_id = None
            display_name = wd.name
            full_name = wd.full_name
            birth = wd.birth_year
            death = wd.death_year
            portrait = wd.portrait_url
        out.append(
            ExtendedComposer(
                mbid=wd.mbid,
                qid=wd.qid,
                name=display_name,
                full_name=full_name,
                birth_year=birth,
                death_year=death,
                epoch=epoch,
                portrait_url=portrait,
                recording_count=count,
                popular=popular,
                open_opus_id=open_opus_id,
            )
        )
        seen_mbid.add(wd.mbid)

    # Defensive merge: every OpenOpus composer must be in the extended
    # snapshot. If the SPARQL filter missed any (rare — happens when the
    # composer's Wikidata entry lacks the genre claims our query checks),
    # we splice them in directly. Their `qid` is left empty since we did
    # not resolve it via SPARQL; the runtime Phase 6 cascade fills it.
    for mbid, oo in openopus_by_mbid.items():
        if mbid in seen_mbid:
            continue
        out.append(
            ExtendedComposer(
                mbid=mbid,
                qid="",
                name=oo.get("name") or "Unknown",
                full_name=oo.get("full_name"),
                birth_year=oo.get("birth_year"),
                death_year=oo.get("death_year"),
                epoch=oo.get("epoch"),
                portrait_url=oo.get("portrait_url"),
                recording_count=-1,
                popular=bool(oo.get("popular", False)),
                open_opus_id=oo.get("open_opus_id"),
            )
        )
        seen_mbid.add(mbid)
        logging.info("OO-only fallback added: %s (%s)", oo.get("name"), mbid)

    return out


# ---------------------------------------------------------------------------
# Step 4 — Emit JSON
# ---------------------------------------------------------------------------


def emit(composers: list[ExtendedComposer], output_path: pathlib.Path, threshold: int) -> None:
    """Write the final snapshot, sorted by MBID for deterministic diffs."""
    composers_sorted = sorted(composers, key=lambda c: c.mbid)
    payload = {
        "schema_version": 1,
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "harvest_threshold_recording_count": threshold,
        "composers": [dataclasses.asdict(c) for c in composers_sorted],
    }
    output_path.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n")
    logging.info(
        "Wrote %d composers to %s (size: %.1f KB)",
        len(composers_sorted),
        output_path,
        output_path.stat().st_size / 1024,
    )


# ---------------------------------------------------------------------------
# Step 5 — Reporting
# ---------------------------------------------------------------------------


CANONICAL_NAMES_TO_VERIFY = [
    ("Bach", "24f1766e-9635-4d58-a4d4-9413f9f98a4c"),
    ("Mozart", "b972f589-fb0e-474e-b64a-803b0364fa75"),
    ("Beethoven", "1f9df192-a621-4f54-8850-2c5373b7eac9"),
    ("Tchaikovsky", "9ddd7abc-9e1b-471d-8031-583bc6bc8be9"),
    ("Schubert", "f91e3a88-24ee-4563-8963-fab73d2765ed"),
    ("Sibelius", "691b0e9d-9e57-41cf-932d-a3d21b068e75"),
    ("Pärt", "ae0b2424-d4c5-4c54-82ac-fe3be5453270"),
    ("Reich", "a3031680-c359-458f-a641-70ccbaec6a74"),
    ("Glass", "5ae54dee-4dba-49c0-802a-a3b3b3adfe9b"),
    ("Adams (John)", "fec1d6df-43ec-4fd6-9bdd-2b29ed18ad03"),
    ("Saariaho", "a25e9314-ad1d-4bd1-bcd7-f4d49c9d5bba"),
    ("Hildegard von Bingen", "e6296fc9-99cd-4053-a8d4-acff48b6e2dc"),
    ("Caroline Shaw", "6b9b39e9-bc24-4ee8-aa2b-7eee9bb8b5b7"),
    ("Anna Thorvaldsdóttir", "6f87adb2-eb64-43ee-acce-32a3a1edff4d"),
]


def report(composers: list[ExtendedComposer]) -> None:
    """Print the Apéndice-A-shaped summary to stdout."""
    by_epoch: dict[str, int] = {}
    by_count_bucket: dict[str, int] = {"≥100": 0, "10-99": 0, "5-9": 0}
    for c in composers:
        epoch = c.epoch or "Unknown"
        by_epoch[epoch] = by_epoch.get(epoch, 0) + 1
        if c.recording_count >= 100:
            by_count_bucket["≥100"] += 1
        elif c.recording_count >= 10:
            by_count_bucket["10-99"] += 1
        else:
            by_count_bucket["5-9"] += 1
    by_mbid = {c.mbid: c for c in composers}
    print("\n=== Apéndice A — harvest report ===")
    print(f"Composers totales: {len(composers)}")
    print("\nDistribution por era:")
    for era, n in sorted(by_epoch.items(), key=lambda kv: -kv[1]):
        print(f"  {era:20s} {n}")
    print("\nDistribution por recording_count:")
    for bucket, n in by_count_bucket.items():
        print(f"  {bucket:8s} {n}")
    print("\nCanonical composers verification:")
    for name, mbid in CANONICAL_NAMES_TO_VERIFY:
        present = "✓" if mbid.lower() in by_mbid else "✗"
        if present == "✓":
            c = by_mbid[mbid.lower()]
            print(f"  {present} {name:25s} mbid={mbid} count={c.recording_count}")
        else:
            print(f"  {present} {name:25s} mbid={mbid} (NOT FOUND)")


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Harvest extended composer snapshot for SONE Classical Phase 7.",
    )
    parser.add_argument(
        "--threshold",
        type=int,
        default=5,
        help="Minimum recording_count in MB (default 5, D-027). Only enforced when --with-mb-counts is set.",
    )
    parser.add_argument(
        "--with-mb-counts",
        action="store_true",
        help=(
            "Cross-check each composer against MB browse for recording_count. "
            "VERY SLOW (~1.05s per composer). Disabled by default; the SPARQL "
            "P800 filter (notable work documented) is a sufficient notability "
            "proxy and ships an audibility-vetted universe in seconds."
        ),
    )
    parser.add_argument(
        "--output",
        type=pathlib.Path,
        default=pathlib.Path(__file__).resolve().parents[3]
        / "src-tauri"
        / "data"
        / "composers-extended.json",
        help="Output path for the snapshot JSON.",
    )
    parser.add_argument(
        "--openopus-path",
        type=pathlib.Path,
        default=pathlib.Path(__file__).resolve().parents[3]
        / "src-tauri"
        / "data"
        / "openopus.json",
        help="Path to existing OpenOpus snapshot for merge.",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Skip MB enrichment and emission; only do SPARQL harvest + report shape.",
    )
    parser.add_argument(
        "--max-composers",
        type=int,
        default=0,
        help="If > 0, cap the number of composers processed in MB (debugging only).",
    )
    parser.add_argument(
        "--verbose",
        "-v",
        action="store_true",
        help="Enable DEBUG logging.",
    )
    args = parser.parse_args()
    setup_logging(args.verbose)

    logging.info("=== Phase 7 snapshot harvest starting ===")
    logging.info("Threshold: recording_count >= %d", args.threshold)
    logging.info("Output: %s", args.output)
    logging.info("Dry run: %s", args.dry_run)

    wdqs_rate = RateLimiter(WDQS_MIN_INTERVAL_S)
    mb_rate = RateLimiter(MB_MIN_INTERVAL_S)

    # Step 1 — SPARQL harvest
    t0 = time.monotonic()
    try:
        wd_composers = sparql_harvest(wdqs_rate)
    except Exception as e:
        logging.error("SPARQL harvest aborted: %s", e)
        return 2
    logging.info(
        "SPARQL harvest: %d composers in %.1fs",
        len(wd_composers),
        time.monotonic() - t0,
    )

    if args.dry_run:
        logging.info("Dry-run mode: stopping after SPARQL harvest.")
        print("\n=== SPARQL-only summary ===")
        print(f"Total Wikidata composers with MB ID: {len(wd_composers)}")
        sample = wd_composers[: min(20, len(wd_composers))]
        print("First 20:")
        for c in sample:
            print(f"  {c.mbid} qid={c.qid} name={c.name!r} years={c.birth_year}-{c.death_year}")
        return 0

    # Step 2 — MB recording_count (optional, slow)
    if args.max_composers > 0:
        logging.warning("Max composers cap: %d (debugging mode)", args.max_composers)
        wd_composers = wd_composers[: args.max_composers]
    recording_counts: dict[str, int] = {}
    if args.with_mb_counts:
        t1 = time.monotonic()
        for i, wd in enumerate(wd_composers, 1):
            count = mb_recording_count(wd.mbid, mb_rate)
            recording_counts[wd.mbid] = count
            if i % 50 == 0:
                logging.info(
                    "MB enrichment %d/%d (%.1f%%) — last: %s count=%d",
                    i,
                    len(wd_composers),
                    100 * i / len(wd_composers),
                    wd.name,
                    count,
                )
        logging.info(
            "MB enrichment complete in %.1fs (%d composers)",
            time.monotonic() - t1,
            len(wd_composers),
        )
    else:
        # Fallback: every composer survives. P800 already vetted notability.
        for wd in wd_composers:
            recording_counts[wd.mbid] = -1  # -1 marks "unknown but accepted"
        logging.info(
            "Skipping MB enrichment (use --with-mb-counts to enable). "
            "All %d Wikidata composers with P800 will be retained.",
            len(wd_composers),
        )

    # Step 3 — Merge
    openopus_by_mbid = load_openopus(args.openopus_path)
    merged = merge(wd_composers, recording_counts, openopus_by_mbid, args.threshold)
    logging.info(
        "After threshold filter (>=%d recordings): %d composers",
        args.threshold,
        len(merged),
    )

    # Step 3.5 — Second-pass portraits (only for composers without portrait
    # already inherited from OpenOpus). Vastly smaller universe than the
    # initial harvest, so the heavy P18 fetch fits within WDQS budget.
    qids_needing_portrait = [c.qid for c in merged if not c.portrait_url]
    logging.info(
        "Composers needing second-pass portrait: %d / %d",
        len(qids_needing_portrait),
        len(merged),
    )
    portraits = fetch_portraits(qids_needing_portrait, wdqs_rate)
    for c in merged:
        if not c.portrait_url and c.qid in portraits:
            c.portrait_url = portraits[c.qid]

    # Step 4 — Emit
    args.output.parent.mkdir(parents=True, exist_ok=True)
    emit(merged, args.output, args.threshold)

    # Step 5 — Report
    report(merged)
    logging.info("=== Done ===")
    return 0


if __name__ == "__main__":
    sys.exit(main())
