//! Phase 9 (D-040) — `WorkBucket` mapping rules.
//!
//! `bucket_for(work_type, genre, p136, title)` is the canonical
//! cascade. Inputs come from MusicBrainz (`work-type`), Wikidata
//! (`P136` instance-of keywords), the editorial snapshot
//! (`editorial.json`/`editorial-extended.json` `bucket` override) and
//! a title-fallback regex layer.
//!
//! This module also hosts the album-title → `WorkBucket` inference
//! used by the matcher's genre penalty (formerly `AlbumKindHint` in
//! `matching.rs`, removed during the Phase 9.B refactor) and the
//! pairwise compatibility lattice consumed by the matcher.
//!
//! Reference: `CLASSICAL_DESIGN.md` §7.2, `DECISIONS.md` D-039 + D-040.

use std::sync::OnceLock;

use regex::Regex;

use crate::tidal_api::TidalTrack;

use super::types::{Genre, WorkBucket, WorkType};

// ---------------------------------------------------------------------------
// Cascade: `bucket_for(work_type, genre, p136, title)`
// ---------------------------------------------------------------------------

/// Phase 9 (D-040) — compute the canonical `WorkBucket` for a work.
///
/// Cascade:
/// 1. Editorial override (callers thread it through directly when set).
///    Not represented here — `catalog::build_work_fresh` consults
///    `editorial.lookup_bucket(work_mbid)` first and only falls
///    through to this function when there's no override.
/// 2. Wikidata P136 keywords (passed as a slice of lowercase strings).
/// 3. MusicBrainz `work-type` mapping.
/// 4. Title-regex fallback.
/// 5. `WorkBucket::Other` if nothing fires.
///
/// `genre` is the Wikidata-derived `Genre` enum; we currently use it
/// as a tiebreaker only (e.g. `Sonata` + `Genre::Chamber` → Chamber).
/// `title` is the work title verbatim — case-insensitive matching is
/// applied internally.
pub fn bucket_for(
    work_type: Option<WorkType>,
    genre: Option<Genre>,
    p136: &[String],
    title: &str,
) -> WorkBucket {
    // (2) P136 keywords — strongest external signal after editorial.
    if let Some(b) = bucket_from_p136(p136) {
        return b;
    }

    // (3) MusicBrainz work-type mapping.
    if let Some(wt) = work_type {
        if let Some(b) = bucket_from_work_type(wt, genre, title) {
            return b;
        }
    }

    // (4) Title-regex fallback.
    if let Some(b) = bucket_from_title(title) {
        return b;
    }

    // (5) Default to Other.
    WorkBucket::Other
}

/// Wikidata P136 instance-of keywords → bucket. Inputs are lowercased
/// labels of P136 claims attached to the work in Wikidata
/// (e.g. ["symphony"], ["opera", "music drama"]).
fn bucket_from_p136(keywords: &[String]) -> Option<WorkBucket> {
    let bag: Vec<&str> = keywords.iter().map(|s| s.as_str()).collect();
    if bag.iter().any(|k| {
        *k == "opera"
            || *k == "operetta"
            || *k == "ballet"
            || *k == "music drama"
            || *k == "musical"
            || *k == "zarzuela"
            || *k == "incidental music"
    }) {
        return Some(WorkBucket::Stage);
    }
    if bag.iter().any(|k| {
        *k == "mass"
            || *k == "requiem"
            || *k == "oratorio"
            || *k == "motet"
            || *k == "passion"
            || *k == "sacred music"
            || *k == "te deum"
            || *k == "magnificat"
    }) {
        return Some(WorkBucket::ChoralSacred);
    }
    if bag.iter().any(|k| {
        *k == "art song"
            || *k == "lied"
            || *k == "song cycle"
            || *k == "mélodie"
            || *k == "melodie"
            || *k == "madrigal"
    }) {
        return Some(WorkBucket::Vocal);
    }
    if bag.iter().any(|k| *k == "symphony" || *k == "sinfonia") {
        return Some(WorkBucket::Symphonies);
    }
    if bag
        .iter()
        .any(|k| *k == "concerto" || *k == "concerto grosso" || *k == "sinfonia concertante")
    {
        return Some(WorkBucket::Concertos);
    }
    if bag.iter().any(|k| {
        *k == "overture"
            || *k == "tone poem"
            || *k == "symphonic poem"
            || *k == "orchestral suite"
    }) {
        return Some(WorkBucket::Orchestral);
    }
    if bag.iter().any(|k| {
        *k == "string quartet"
            || *k == "piano trio"
            || *k == "string trio"
            || *k == "piano quintet"
            || *k == "chamber music"
    }) {
        return Some(WorkBucket::Chamber);
    }
    if bag.iter().any(|k| {
        *k == "piano sonata"
            || *k == "organ work"
            || *k == "harpsichord work"
            || *k == "piano work"
    }) {
        return Some(WorkBucket::Keyboard);
    }
    if bag
        .iter()
        .any(|k| *k == "solo violin" || *k == "solo cello" || *k == "solo guitar")
    {
        return Some(WorkBucket::SoloInstrumental);
    }
    if bag
        .iter()
        .any(|k| *k == "film score" || *k == "soundtrack")
    {
        return Some(WorkBucket::FilmTheatre);
    }
    None
}

/// MusicBrainz `work-type` → bucket. The mapping handles the easy
/// cases directly and consults `genre` + `title` for the genuinely
/// ambiguous types (Sonata, Suite, Cantata).
fn bucket_from_work_type(
    work_type: WorkType,
    genre: Option<Genre>,
    title: &str,
) -> Option<WorkBucket> {
    match work_type {
        WorkType::Symphony => Some(WorkBucket::Symphonies),
        WorkType::Concerto => Some(WorkBucket::Concertos),
        WorkType::Opera => Some(WorkBucket::Stage),
        WorkType::Mass => Some(WorkBucket::ChoralSacred),
        WorkType::StringQuartet => Some(WorkBucket::Chamber),
        WorkType::Lieder => Some(WorkBucket::Vocal),

        // Cantata: BWV 1-200 are sacred (Bach's church cantatas);
        // outside that range or when title carries a sacred token,
        // ChoralSacred. Otherwise Vocal (secular cantata).
        WorkType::Cantata => {
            let lower = title.to_lowercase();
            if title_looks_sacred(&lower) || is_bach_sacred_cantata_range(&lower) {
                Some(WorkBucket::ChoralSacred)
            } else {
                Some(WorkBucket::Vocal)
            }
        }

        // Sonata: keyboard if title says so; solo instrumental if a
        // single non-keyboard instrument; chamber otherwise.
        WorkType::Sonata => {
            let lower = title.to_lowercase();
            if title_says_keyboard_instrument(&lower) {
                Some(WorkBucket::Keyboard)
            } else if title_says_solo_string_or_wind(&lower)
                && !title_mentions_piano_accompaniment(&lower)
            {
                Some(WorkBucket::SoloInstrumental)
            } else if let Some(Genre::Chamber) = genre {
                Some(WorkBucket::Chamber)
            } else {
                Some(WorkBucket::Chamber)
            }
        }

        // Suite: orchestral if explicit, solo instrumental for cello
        // / violin suites, keyboard for harpsichord/piano suites;
        // default orchestral.
        WorkType::Suite => {
            let lower = title.to_lowercase();
            if title_says_solo_string_or_wind(&lower) {
                Some(WorkBucket::SoloInstrumental)
            } else if title_says_keyboard_instrument(&lower) {
                Some(WorkBucket::Keyboard)
            } else {
                Some(WorkBucket::Orchestral)
            }
        }

        // Étude: keyboard unless title says guitar / violin.
        WorkType::Etude => {
            let lower = title.to_lowercase();
            if title_says_solo_string_or_wind(&lower) {
                Some(WorkBucket::SoloInstrumental)
            } else {
                Some(WorkBucket::Keyboard)
            }
        }

        // Other → fall through to title regex.
        WorkType::Other => None,
    }
}

/// Title regex fallback — last gate before `Other`. Conservative on
/// purpose: misclassifying into Symphonies is much worse than landing
/// in Other. Patterns require word boundaries to avoid matching e.g.
/// "Overtures and Other Pieces" → "Overtures" → Orchestral.
fn bucket_from_title(title: &str) -> Option<WorkBucket> {
    static RE_SACRED: OnceLock<Regex> = OnceLock::new();
    static RE_OVERTURE: OnceLock<Regex> = OnceLock::new();
    static RE_TONE_POEM: OnceLock<Regex> = OnceLock::new();
    static RE_CONCERTO: OnceLock<Regex> = OnceLock::new();
    static RE_KEYBOARD_FORMS: OnceLock<Regex> = OnceLock::new();
    static RE_SYMPHONY: OnceLock<Regex> = OnceLock::new();
    static RE_OPERA: OnceLock<Regex> = OnceLock::new();

    let re_sacred = RE_SACRED.get_or_init(|| {
        Regex::new(r"(?i)^\s*(missa|requiem|te deum|stabat mater|magnificat)\b").unwrap()
    });
    if re_sacred.is_match(title) {
        return Some(WorkBucket::ChoralSacred);
    }

    let re_symphony = RE_SYMPHONY.get_or_init(|| {
        Regex::new(r"(?i)^\s*(symphony|sinfonia|sinfonía)\s+(no\.?|num\.?|n\.?|\d|in\b)").unwrap()
    });
    if re_symphony.is_match(title) {
        return Some(WorkBucket::Symphonies);
    }

    let re_concerto = RE_CONCERTO.get_or_init(|| {
        Regex::new(r"(?i)\b(concerto|concertos|concerti|concert(?:o|i) grosso)\b").unwrap()
    });
    if re_concerto.is_match(title) {
        return Some(WorkBucket::Concertos);
    }

    let re_overture = RE_OVERTURE
        .get_or_init(|| Regex::new(r"(?i)\b(overture|prelude to\s+\w)\b").unwrap());
    if re_overture.is_match(title) {
        return Some(WorkBucket::Orchestral);
    }

    let re_tone_poem = RE_TONE_POEM
        .get_or_init(|| Regex::new(r"(?i)\b(symphonic poem|tone poem|poema sinfónico)\b").unwrap());
    if re_tone_poem.is_match(title) {
        return Some(WorkBucket::Orchestral);
    }

    let re_opera = RE_OPERA.get_or_init(|| {
        Regex::new(r"(?i)\b(opera|operetta|ballet|libretto)\b").unwrap()
    });
    if re_opera.is_match(title) {
        return Some(WorkBucket::Stage);
    }

    let re_keyboard_forms = RE_KEYBOARD_FORMS.get_or_init(|| {
        Regex::new(
            r"(?i)\b(nocturne|nocturnes|mazurka|mazurkas|polonaise|polonaises|ballade|ballades|impromptu|impromptus|prelude|preludes|prélude|préludes|fugue|fugues|étude|études|etude|etudes|variations|partita|invention|inventions)\b",
        )
        .unwrap()
    });
    if re_keyboard_forms.is_match(title) {
        return Some(WorkBucket::Keyboard);
    }

    None
}

fn title_looks_sacred(lower: &str) -> bool {
    lower.contains("sacred") || lower.contains("sacra") || lower.contains("kirchen")
}

fn is_bach_sacred_cantata_range(lower: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"\bbwv\s*0*([1-9]\d{0,2})\b").unwrap());
    if let Some(caps) = re.captures(lower) {
        if let Some(num) = caps.get(1) {
            if let Ok(n) = num.as_str().parse::<u32>() {
                return (1..=200).contains(&n);
            }
        }
    }
    false
}

fn title_says_keyboard_instrument(lower: &str) -> bool {
    lower.contains("for piano")
        || lower.contains("for harpsichord")
        || lower.contains("for organ")
        || lower.contains("for clavier")
        || lower.contains("piano sonata")
        || lower.contains("harpsichord sonata")
        || lower.contains("organ sonata")
}

fn title_says_solo_string_or_wind(lower: &str) -> bool {
    // Order: prefer "for cello" over "cello sonata" when both could
    // match — this function only checks one level of specificity.
    lower.contains("for solo violin")
        || lower.contains("for solo cello")
        || lower.contains("for solo flute")
        || lower.contains("for solo guitar")
        || lower.contains("for solo viola")
        || lower.contains("for violin solo")
        || lower.contains("for cello solo")
        || lower.contains("cello suite")
        || lower.contains("violin partita")
}

fn title_mentions_piano_accompaniment(lower: &str) -> bool {
    lower.contains("violin and piano")
        || lower.contains("cello and piano")
        || lower.contains("flute and piano")
        || lower.contains("for violin and piano")
        || lower.contains("for cello and piano")
}

// ---------------------------------------------------------------------------
// Album-title → `WorkBucket` (used by the matcher's genre penalty)
// ---------------------------------------------------------------------------

/// Phase 9 (B9.1 refactor) — derive a `WorkBucket` hint from the
/// candidate's Tidal album title. Replaces the legacy
/// `infer_album_kind` from `matching.rs`. Returns `None` when no
/// confident signal is present (the matcher reads "no penalty").
///
/// The function is conservative on purpose: ambiguous compound
/// titles ("Concertos and Symphonies") return `None`. Order matters
/// inside — more specific buckets first.
pub fn bucket_from_album_title(candidate: &TidalTrack) -> Option<WorkBucket> {
    let album_title = candidate.album.as_ref().map(|a| a.title.to_lowercase())?;

    let has_symphony = album_title.contains("symphony")
        || album_title.contains("symphonies")
        || album_title.contains("sinfonia");
    let has_concerto = album_title.contains("concerto")
        || album_title.contains("concertos")
        || album_title.contains("concerti");
    let has_opera = album_title.contains("opera")
        || album_title.contains(" act ")
        || album_title.starts_with("act ");
    let has_choral = album_title.contains("requiem")
        || album_title.contains("mass")
        || album_title.contains("oratorio")
        || album_title.contains("passion")
        || album_title.contains("cantata");
    let has_lieder = album_title.contains("lieder")
        || album_title.contains("songs")
        || album_title.contains("gesänge")
        || album_title.contains("gesange");
    let has_quartet = album_title.contains("quartet")
        || album_title.contains("quintet")
        || album_title.contains("trio")
        || album_title.contains("chamber");
    let has_keyboard = album_title.contains("piano sonata")
        || album_title.contains("piano sonatas")
        || album_title.contains("variations")
        || album_title.contains("preludes")
        || album_title.contains("nocturnes")
        || album_title.contains("études")
        || album_title.contains("etudes")
        || album_title.contains("ballades");

    // Ambiguous compounds → no signal.
    let signals = [
        has_symphony,
        has_concerto,
        has_opera,
        has_choral,
        has_lieder,
        has_quartet,
        has_keyboard,
    ];
    let count = signals.iter().filter(|b| **b).count();
    if count >= 2 {
        return None;
    }

    if has_symphony {
        return Some(WorkBucket::Symphonies);
    }
    if has_concerto {
        return Some(WorkBucket::Concertos);
    }
    if has_opera {
        return Some(WorkBucket::Stage);
    }
    if has_choral {
        return Some(WorkBucket::ChoralSacred);
    }
    if has_lieder {
        return Some(WorkBucket::Vocal);
    }
    if has_quartet {
        return Some(WorkBucket::Chamber);
    }
    if has_keyboard {
        return Some(WorkBucket::Keyboard);
    }
    None
}

/// Phase 9 (B9.1 refactor) — pairwise compatibility lattice. The
/// matcher penalises a candidate by `GENRE_BUCKET_PENALTY` when the
/// work's bucket and the candidate album's bucket are explicitly
/// incompatible per this matrix.
///
/// Symmetric in spirit (Vocal ⊥ Symphonic both ways) but encoded as a
/// directed lookup because the goal is "did the candidate match a
/// completely wrong bucket?" — the work's bucket is authoritative.
///
/// `Other` and `FilmTheatre` always return `true` (can't tell, no
/// penalty). When `album_kind` is `None` the matcher never calls this.
pub fn buckets_compatible(work_bucket: WorkBucket, album_kind: WorkBucket) -> bool {
    use WorkBucket::*;

    // Authoritative pass-throughs.
    if matches!(work_bucket, Other | FilmTheatre) {
        return true;
    }
    if matches!(album_kind, Other | FilmTheatre) {
        return true;
    }

    match (work_bucket, album_kind) {
        // Symmetric exact match.
        (a, b) if a == b => true,

        // Stage works frequently appear on choral & sacred albums
        // (oratorios share covers with operas in some catalogues) and
        // vice versa.
        (Stage, ChoralSacred) | (ChoralSacred, Stage) => true,

        // Vocal works can ride on choral albums (lied in a sacred
        // recital).
        (Vocal, ChoralSacred) | (ChoralSacred, Vocal) => true,

        // Symphonies frequently share albums with concertos
        // ("Brahms: Symphony No. 4 / Violin Concerto").
        (Symphonies, Concertos) | (Concertos, Symphonies) => true,
        (Symphonies, Orchestral) | (Orchestral, Symphonies) => true,
        (Concertos, Orchestral) | (Orchestral, Concertos) => true,

        // Chamber + keyboard share albums in piano-trio compilations.
        (Chamber, Keyboard) | (Keyboard, Chamber) => true,

        // Solo instrumental + chamber share albums in mixed
        // recitals.
        (SoloInstrumental, Chamber) | (Chamber, SoloInstrumental) => true,

        // Solo instrumental + keyboard share albums in piano-and-
        // companion-instrument programmes.
        (SoloInstrumental, Keyboard) | (Keyboard, SoloInstrumental) => true,

        // Everything else explicitly: incompatible.
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Tests — D-040 deterministic canon
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classical::types::{Genre, WorkType};

    fn b(work_type: Option<WorkType>, title: &str) -> WorkBucket {
        bucket_for(work_type, None, &[], title)
    }

    fn bg(work_type: Option<WorkType>, genre: Option<Genre>, title: &str) -> WorkBucket {
        bucket_for(work_type, genre, &[], title)
    }

    // ---- 15 canon cases (deterministic) ----

    #[test]
    fn beethoven_symphony_9_choral() {
        // Beethoven 9 has a choral finale but is structurally a
        // symphony. Bucket: Symphonies.
        assert_eq!(
            b(
                Some(WorkType::Symphony),
                "Symphony No. 9 in D minor, Op. 125 (Choral)",
            ),
            WorkBucket::Symphonies
        );
    }

    #[test]
    fn bach_st_matthew_passion_falls_to_other_without_p136() {
        // Without P136 ["passion"] or work_type=Mass the title
        // regex doesn't catch the bare "Passion" token. This is a
        // documented V1 limitation: Bach Passion entries lean on
        // either Wikidata P136 or an editorial snapshot bucket
        // override. The test characterises the fallback path
        // explicitly so the regression is visible if someone tries
        // to extend `bucket_from_title` later.
        assert_eq!(
            b(Some(WorkType::Other), "St Matthew Passion, BWV 244"),
            WorkBucket::Other
        );
    }

    #[test]
    fn bach_st_matthew_passion_with_p136() {
        let p136 = vec!["passion".to_string()];
        assert_eq!(
            bucket_for(
                Some(WorkType::Other),
                None,
                &p136,
                "St Matthew Passion, BWV 244",
            ),
            WorkBucket::ChoralSacred
        );
    }

    #[test]
    fn schubert_winterreise() {
        assert_eq!(
            b(Some(WorkType::Lieder), "Winterreise, D. 911"),
            WorkBucket::Vocal
        );
    }

    #[test]
    fn chopin_etude_op_10_no_1() {
        assert_eq!(
            b(Some(WorkType::Etude), "Étude in C major, Op. 10 No. 1"),
            WorkBucket::Keyboard
        );
    }

    #[test]
    fn bach_cello_suite_bwv_1007() {
        assert_eq!(
            b(Some(WorkType::Suite), "Cello Suite No. 1 in G major, BWV 1007"),
            WorkBucket::SoloInstrumental
        );
    }

    #[test]
    fn stravinsky_petrushka_suite() {
        assert_eq!(
            b(Some(WorkType::Suite), "Petrushka Suite (1947 version)"),
            WorkBucket::Orchestral
        );
    }

    #[test]
    fn stravinsky_petrushka_ballet_via_p136() {
        let p136 = vec!["ballet".to_string()];
        assert_eq!(
            bucket_for(None, None, &p136, "Petrushka"),
            WorkBucket::Stage
        );
    }

    #[test]
    fn mozart_requiem() {
        assert_eq!(
            b(Some(WorkType::Mass), "Requiem in D minor, K. 626"),
            WorkBucket::ChoralSacred
        );
    }

    #[test]
    fn bach_goldberg_variations() {
        assert_eq!(
            b(Some(WorkType::Other), "Goldberg Variations, BWV 988"),
            WorkBucket::Keyboard
        );
    }

    #[test]
    fn brahms_violin_concerto() {
        assert_eq!(
            b(Some(WorkType::Concerto), "Violin Concerto in D major, Op. 77"),
            WorkBucket::Concertos
        );
    }

    #[test]
    fn beethoven_string_quartet_op_131() {
        assert_eq!(
            b(
                Some(WorkType::StringQuartet),
                "String Quartet No. 14 in C-sharp minor, Op. 131",
            ),
            WorkBucket::Chamber
        );
    }

    #[test]
    fn wagner_tristan_und_isolde() {
        assert_eq!(
            b(Some(WorkType::Opera), "Tristan und Isolde"),
            WorkBucket::Stage
        );
    }

    #[test]
    fn debussy_la_mer_via_title_regex() {
        // No work-type, no P136 — title regex should NOT misfire.
        // La Mer is a tone poem; without a regex hit it falls to Other
        // unless P136 ["symphonic poem"] is present.
        assert_eq!(b(None, "La Mer"), WorkBucket::Other);
        let p136 = vec!["symphonic poem".to_string()];
        assert_eq!(
            bucket_for(None, None, &p136, "La Mer"),
            WorkBucket::Orchestral
        );
    }

    #[test]
    fn bach_cantata_bwv_140_sacred_via_range() {
        assert_eq!(
            b(
                Some(WorkType::Cantata),
                "Wachet auf, ruft uns die Stimme, BWV 140",
            ),
            WorkBucket::ChoralSacred
        );
    }

    #[test]
    fn bach_secular_cantata_bwv_211_coffee() {
        // BWV 211 is the Coffee Cantata — outside the sacred range
        // 1-200. Falls to Vocal.
        assert_eq!(
            b(
                Some(WorkType::Cantata),
                "Schweigt stille, plaudert nicht, BWV 211",
            ),
            WorkBucket::Vocal
        );
    }

    #[test]
    fn beethoven_3_gesange_op_83_lieder() {
        // The work that triggered D-041 (Op. 83 → Eroica false
        // positive). Confirm bucketing assigns Vocal regardless.
        assert_eq!(
            b(
                Some(WorkType::Lieder),
                "3 Gesänge, Op. 83",
            ),
            WorkBucket::Vocal
        );
    }

    #[test]
    fn glass_einstein_on_the_beach_via_p136() {
        let p136 = vec!["opera".to_string()];
        assert_eq!(
            bucket_for(None, None, &p136, "Einstein on the Beach"),
            WorkBucket::Stage
        );
    }

    #[test]
    fn fallback_other() {
        assert_eq!(b(None, "Three Untitled Pieces"), WorkBucket::Other);
    }

    // ---- Sonata ambiguity ----

    #[test]
    fn beethoven_piano_sonata_op_111() {
        assert_eq!(
            b(Some(WorkType::Sonata), "Piano Sonata No. 32 in C minor, Op. 111"),
            WorkBucket::Keyboard
        );
    }

    #[test]
    fn brahms_violin_sonata_chamber_default() {
        // No "for piano" hint, no "for solo violin" hint — falls
        // through to Chamber (a violin sonata is implicitly
        // violin + piano).
        assert_eq!(
            b(Some(WorkType::Sonata), "Violin Sonata No. 1, Op. 78"),
            WorkBucket::Chamber
        );
    }

    #[test]
    fn bach_violin_partita_solo() {
        assert_eq!(
            b(
                Some(WorkType::Sonata),
                "Partita for solo violin No. 2 in D minor, BWV 1004",
            ),
            WorkBucket::SoloInstrumental
        );
    }

    // ---- Suite ambiguity ----

    #[test]
    fn handel_water_music_suite() {
        assert_eq!(
            b(Some(WorkType::Suite), "Water Music Suite No. 1, HWV 348"),
            WorkBucket::Orchestral
        );
    }

    #[test]
    fn bach_french_suite_keyboard() {
        assert_eq!(
            b(
                Some(WorkType::Suite),
                "French Suite No. 5 in G major for harpsichord, BWV 816",
            ),
            WorkBucket::Keyboard
        );
    }

    // ---- Title-regex coverage ----

    #[test]
    fn rachmaninoff_symphonic_poem_via_title() {
        assert_eq!(
            b(None, "Isle of the Dead, symphonic poem, Op. 29"),
            WorkBucket::Orchestral
        );
    }

    #[test]
    fn beethoven_egmont_overture_via_title() {
        assert_eq!(
            b(None, "Egmont Overture, Op. 84"),
            WorkBucket::Orchestral
        );
    }

    #[test]
    fn brahms_ballade_via_title_keyboard() {
        assert_eq!(
            b(None, "Ballade in D minor, Op. 10 No. 1"),
            WorkBucket::Keyboard
        );
    }

    // ---- Genre tiebreaker ----

    #[test]
    fn sonata_with_chamber_genre_resolves_chamber() {
        assert_eq!(
            bg(
                Some(WorkType::Sonata),
                Some(Genre::Chamber),
                "Sonata in B-flat",
            ),
            WorkBucket::Chamber
        );
    }

    // ---- buckets_compatible lattice ----

    #[test]
    fn vocal_incompatible_with_symphonies() {
        assert!(!buckets_compatible(WorkBucket::Vocal, WorkBucket::Symphonies));
        assert!(!buckets_compatible(WorkBucket::Symphonies, WorkBucket::Vocal));
    }

    #[test]
    fn symphonies_compatible_with_concertos() {
        assert!(buckets_compatible(WorkBucket::Symphonies, WorkBucket::Concertos));
        assert!(buckets_compatible(WorkBucket::Concertos, WorkBucket::Symphonies));
    }

    #[test]
    fn chamber_compatible_with_keyboard() {
        assert!(buckets_compatible(WorkBucket::Chamber, WorkBucket::Keyboard));
    }

    #[test]
    fn other_always_compatible() {
        assert!(buckets_compatible(WorkBucket::Other, WorkBucket::Symphonies));
        assert!(buckets_compatible(WorkBucket::Symphonies, WorkBucket::Other));
    }

    #[test]
    fn film_always_compatible() {
        assert!(buckets_compatible(WorkBucket::FilmTheatre, WorkBucket::Symphonies));
    }

    #[test]
    fn stage_compatible_with_choral_sacred() {
        // Don Giovanni / oratorio adjacency.
        assert!(buckets_compatible(WorkBucket::Stage, WorkBucket::ChoralSacred));
    }
}
