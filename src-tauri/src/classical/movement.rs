//! Movement boundary detection (Phase 3 / B3.1).
//!
//! Given a Tidal track title and a `Work` (with its parsed `movements`),
//! resolve which movement the track corresponds to. Used by the player
//! to render the "II / IV" indicator and the "Attacca →" hint.
//!
//! Strategy (in order of preference):
//!
//!   1. **Roman numeral prefix** — most movements in classical Tidal
//!      catalog use roman prefixes ("I.", "II.", "IIIa.", "IV: ..."). Parse
//!      the leading roman and match it against `Movement.index`.
//!   2. **Title substring match** — fallback when no roman is present:
//!      check whether `track_title` contains `movement.title` (case +
//!      diacritics insensitive). Useful for Bach BWV where Tidal labels are
//!      "Aria", "Variation 1", etc.
//!   3. **Album position** — last-resort fallback driven externally by the
//!      caller. We expose `resolve_by_position(work, position_zero_based)`.
//!
//! All three are pure functions with deterministic outputs, fully unit
//! testable without network or Tidal auth. The Tauri command wraps them
//! in `get_classical_movement_for_track`.

use serde::Serialize;

use super::types::{Movement, Work};

/// Context returned to the frontend for rendering the player movement
/// indicator. `index` is 1-based (matches `Movement.index`); `total` is
/// the count of movements in the parent `Work`.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MovementContext {
    /// 1-based. Always within `1..=total`.
    pub index: u32,
    pub total: u32,
    /// Display title of this movement (from `Movement.title`).
    pub title: String,
    /// `Some(idx)` when this movement's editorial flag says "attacca to
    /// movement idx". Used to render the "Attacca →" hint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attacca_to: Option<u32>,
    /// How we resolved this match. Useful for QA / debugging in the
    /// frontend (rendered as a hover tooltip).
    pub method: ResolutionMethod,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ResolutionMethod {
    /// Track title started with a roman numeral that maps to a movement
    /// index.
    RomanPrefix,
    /// Track title contains the movement's `title` as a substring (after
    /// normalization).
    TitleSubstring,
    /// External hint: the caller knew the album-level position of the
    /// track, and we trusted it.
    AlbumPosition,
}

/// Parse a roman numeral at the very start of `s`. Recognizes I..XCIX
/// (1..=99) because no real classical work has more movements than that.
/// Tolerates trailing punctuation (`.`, `:`, `-`, `/`, ` `, em-dash),
/// tolerates a single trailing letter suffix (`IIIa`, `IIb`) which
/// collapses to its base index, and is case-insensitive.
///
/// **Refuses** when the roman is followed by another word continuation
/// (`IVAllegro`) — that's a heuristic safeguard against false positives
/// like an Italian word starting with roman letters.
///
/// Returns `Some(index)` on success, `None` if the leading token is not
/// a roman.
pub fn parse_leading_roman(s: &str) -> Option<u32> {
    let trimmed = s.trim_start();
    if trimmed.is_empty() {
        return None;
    }

    // Walk while the character is a roman letter (case-insensitive).
    let mut roman_end = 0usize;
    for (i, ch) in trimmed.char_indices() {
        if matches!(
            ch.to_ascii_uppercase(),
            'I' | 'V' | 'X' | 'L' | 'C' | 'D' | 'M'
        ) {
            // All roman letters are ASCII (1 byte), but use len_utf8 for
            // correctness rather than relying on that assumption.
            roman_end = i + ch.len_utf8();
        } else {
            break;
        }
    }

    if roman_end == 0 {
        return None;
    }

    let roman_part = &trimmed[..roman_end];
    let after_roman = &trimmed[roman_end..];

    // Look at what follows the roman block:
    //   * end-of-string                  → accept
    //   * separator (./:/space/-/etc.)   → accept
    //   * single letter suffix + sep/end → accept ("IIIa.", "IIb")
    //   * letter + letter (a word)       → REJECT ("IVAllegro" is "Allegro"
    //                                      starting with a 4-coincidence)
    let mut chars = after_roman.chars();
    let next = chars.next();
    match next {
        None => parse_roman_strict(roman_part),
        Some(c) if is_separator(c) => parse_roman_strict(roman_part),
        Some(c) if c.is_ascii_alphabetic() => {
            // Look at the char after the candidate suffix.
            let after_suffix_char = chars.next();
            match after_suffix_char {
                None => parse_roman_strict(roman_part),
                Some(c2) if is_separator(c2) => parse_roman_strict(roman_part),
                // Two consecutive letters after the roman → it's a word,
                // not a roman + suffix.
                Some(_) => None,
            }
        }
        _ => None,
    }
}

#[inline]
fn is_separator(c: char) -> bool {
    matches!(
        c,
        '.' | ':' | ' ' | '\t' | '-' | '/' | ',' | '—' | '–' | ';'
    )
}

/// Strict roman → integer. Returns `None` for malformed romans (`IIII`,
/// empty, or non-roman input). Range: 1..=99 (we don't expect movements
/// beyond that).
fn parse_roman_strict(input: &str) -> Option<u32> {
    if input.is_empty() {
        return None;
    }
    let upper = input.to_ascii_uppercase();
    let mut total: u32 = 0;
    let mut prev_value: u32 = 0;
    // Walk right-to-left applying subtraction rule.
    for ch in upper.chars().rev() {
        let v = match ch {
            'I' => 1,
            'V' => 5,
            'X' => 10,
            'L' => 50,
            'C' => 100,
            'D' => 500,
            'M' => 1000,
            _ => return None,
        };
        if v < prev_value {
            total = total.checked_sub(v)?;
        } else {
            total = total.checked_add(v)?;
        }
        prev_value = v;
    }
    if total == 0 || total > 99 {
        return None;
    }
    // Reject malformed forms: "IIII", "VV", "LL", "DD", "MM" use of
    // repeating subtractive characters — `IV` is fine, `IIII` is not.
    // We do this by re-encoding `total` and comparing.
    if to_roman_canonical(total).map(|c| c == upper).unwrap_or(false) {
        Some(total)
    } else {
        None
    }
}

fn to_roman_canonical(mut n: u32) -> Option<String> {
    if !(1..=99).contains(&n) {
        return None;
    }
    let table: [(u32, &str); 8] = [
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut out = String::new();
    for (val, sym) in table {
        while n >= val {
            out.push_str(sym);
            n -= val;
        }
    }
    Some(out)
}

/// Normalize a string for substring comparison: lowercase, ASCII fold
/// of common diacritics, collapse whitespace, drop punctuation. Pure.
///
/// All separator-class characters (whitespace, `-`, `_`, em-dash, en-dash,
/// slash, comma, period) collapse to a single space so word boundaries
/// aren't lost when two words are joined by a punctuation mark.
fn normalize_for_match(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_was_space = true;
    for ch in s.chars() {
        let c = ch.to_ascii_lowercase();
        let folded = match c {
            'á' | 'à' | 'ä' | 'â' | 'ã' | 'å' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' | 'õ' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            _ => c,
        };
        if folded.is_ascii_alphanumeric() {
            out.push(folded);
            last_was_space = false;
        } else if (folded.is_whitespace()
            || matches!(folded, '-' | '_' | '/' | ',' | '.' | ':' | ';' | '—' | '–'))
            && !last_was_space
        {
            out.push(' ');
            last_was_space = true;
        }
        // Other punctuation (parens, quotes, etc.): drop silently.
    }
    out.trim().to_string()
}

/// Resolve the movement that matches `track_title` within `work`.
///
/// Returns `None` if `work.movements` is empty (the work has no movement
/// breakdown — e.g. a single-movement piece) or if no heuristic matches.
pub fn resolve_by_title(work: &Work, track_title: &str) -> Option<MovementContext> {
    if work.movements.is_empty() {
        return None;
    }

    // 1. Roman prefix.
    if let Some(idx) = parse_leading_roman(track_title) {
        if let Some(m) = work.movements.iter().find(|m| m.index == idx) {
            return Some(make_context(work, m, ResolutionMethod::RomanPrefix));
        }
    }

    // 2. Title substring match (normalized).
    let track_norm = normalize_for_match(track_title);
    if !track_norm.is_empty() {
        // Pick the LONGEST matching movement title — avoids "Aria" matching
        // when the track is actually "Aria with Variations".
        let mut best: Option<&Movement> = None;
        let mut best_len: usize = 0;
        for m in &work.movements {
            let m_norm = normalize_for_match(&m.title);
            if m_norm.is_empty() {
                continue;
            }
            if track_norm.contains(&m_norm) && m_norm.len() > best_len {
                best_len = m_norm.len();
                best = Some(m);
            }
        }
        if let Some(m) = best {
            return Some(make_context(work, m, ResolutionMethod::TitleSubstring));
        }
    }

    None
}

/// Resolve the movement at `position_zero_based` within `work.movements`,
/// trusting the caller's external knowledge (Tidal album track-list
/// position). Use only as fallback when `resolve_by_title` failed.
pub fn resolve_by_position(work: &Work, position_zero_based: usize) -> Option<MovementContext> {
    let m = work.movements.get(position_zero_based)?;
    Some(make_context(work, m, ResolutionMethod::AlbumPosition))
}

fn make_context(work: &Work, m: &Movement, method: ResolutionMethod) -> MovementContext {
    MovementContext {
        index: m.index,
        total: work.movements.len() as u32,
        title: m.title.clone(),
        attacca_to: m.attacca_to,
        method,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classical::types::Movement;

    fn mk_movements(specs: &[(u32, &str, Option<u32>)]) -> Vec<Movement> {
        specs
            .iter()
            .map(|(idx, title, attacca)| Movement {
                mbid: format!("movement-{idx}"),
                index: *idx,
                title: title.to_string(),
                duration_approx_secs: None,
                attacca_to: *attacca,
            })
            .collect()
    }

    fn mk_work(movements: Vec<Movement>) -> Work {
        let mut w = Work::skeleton("work-mbid");
        w.title = "Test Symphony".to_string();
        w.movements = movements;
        w
    }

    #[test]
    fn roman_simple_prefix_with_period() {
        assert_eq!(parse_leading_roman("I. Allegro"), Some(1));
        assert_eq!(parse_leading_roman("II. Andante"), Some(2));
        assert_eq!(parse_leading_roman("III. Scherzo"), Some(3));
        assert_eq!(parse_leading_roman("IV. Finale"), Some(4));
        assert_eq!(parse_leading_roman("V. Adagio"), Some(5));
        assert_eq!(parse_leading_roman("VI. Coda"), Some(6));
        assert_eq!(parse_leading_roman("VII"), Some(7));
        assert_eq!(parse_leading_roman("VIII. Pastorale"), Some(8));
        assert_eq!(parse_leading_roman("IX. Choral"), Some(9));
        assert_eq!(parse_leading_roman("X."), Some(10));
    }

    #[test]
    fn roman_with_colon_dash_slash() {
        assert_eq!(parse_leading_roman("II: Molto vivace"), Some(2));
        assert_eq!(parse_leading_roman("III - Adagio"), Some(3));
        assert_eq!(parse_leading_roman("IV/V"), Some(4));
        assert_eq!(parse_leading_roman("V — Coda"), Some(5));
    }

    #[test]
    fn roman_lowercase() {
        assert_eq!(parse_leading_roman("ii. Andante"), Some(2));
        assert_eq!(parse_leading_roman("iv. Allegro"), Some(4));
    }

    #[test]
    fn roman_with_letter_suffix_collapses_to_base() {
        assert_eq!(parse_leading_roman("IIIa. Trio"), Some(3));
        assert_eq!(parse_leading_roman("IIb. Variation"), Some(2));
        assert_eq!(parse_leading_roman("Va. Coda"), Some(5));
    }

    #[test]
    fn roman_rejects_no_separator() {
        // "IVAllegro" — no separator after the roman → could be a word
        // starting with romanic letters. Refuse.
        assert_eq!(parse_leading_roman("IVAllegro"), None);
        // But the strict path catches "IV" alone fine — that's covered by
        // the "no chars after" branch.
        assert_eq!(parse_leading_roman("IV"), Some(4));
    }

    #[test]
    fn roman_rejects_malformed() {
        assert_eq!(parse_leading_roman("IIII. Allegro"), None);
        assert_eq!(parse_leading_roman("VV. Foo"), None);
        assert_eq!(parse_leading_roman("XXXX. Bar"), None);
    }

    #[test]
    fn roman_rejects_non_roman_prefix() {
        assert_eq!(parse_leading_roman("Allegro non troppo"), None);
        assert_eq!(parse_leading_roman("1. Allegro"), None);
        assert_eq!(parse_leading_roman(""), None);
        assert_eq!(parse_leading_roman("   "), None);
    }

    #[test]
    fn roman_handles_leading_whitespace() {
        assert_eq!(parse_leading_roman("  III. Scherzo"), Some(3));
        assert_eq!(parse_leading_roman("\tII"), Some(2));
    }

    #[test]
    fn resolve_by_title_roman_match() {
        let work = mk_work(mk_movements(&[
            (1, "Allegro ma non troppo", None),
            (2, "Andante con moto", None),
            (3, "Scherzo: Allegro", None),
            (4, "Allegro", Some(4)), // self-attacca makes no semantic sense but verifies serde
        ]));
        let ctx = resolve_by_title(&work, "II. Andante con moto").unwrap();
        assert_eq!(ctx.index, 2);
        assert_eq!(ctx.total, 4);
        assert_eq!(ctx.method, ResolutionMethod::RomanPrefix);
        assert_eq!(ctx.title, "Andante con moto");
    }

    #[test]
    fn resolve_by_title_attacca_propagates() {
        let work = mk_work(mk_movements(&[
            (1, "Allegro", None),
            (2, "Andante", None),
            (3, "Allegro vivace — attacca", Some(4)),
            (4, "Finale", None),
        ]));
        let ctx = resolve_by_title(&work, "III. Allegro vivace").unwrap();
        assert_eq!(ctx.index, 3);
        assert_eq!(ctx.attacca_to, Some(4));
    }

    #[test]
    fn resolve_by_title_substring_fallback() {
        // Bach Goldberg-style: no roman, just movement names.
        let work = mk_work(mk_movements(&[
            (1, "Aria", None),
            (2, "Variation 1", None),
            (3, "Variation 2", None),
            (4, "Aria da capo", None),
        ]));
        let ctx = resolve_by_title(&work, "Variation 2 in G major").unwrap();
        assert_eq!(ctx.index, 3);
        assert_eq!(ctx.method, ResolutionMethod::TitleSubstring);
    }

    #[test]
    fn resolve_by_title_substring_picks_longest_match() {
        // Disambiguation: "Aria" and "Aria da capo" both substring-match a
        // track titled "Aria da capo e fine". We must pick the longer.
        let work = mk_work(mk_movements(&[
            (1, "Aria", None),
            (2, "Variation 1", None),
            (3, "Aria da capo", None),
        ]));
        let ctx = resolve_by_title(&work, "Aria da capo e fine").unwrap();
        assert_eq!(ctx.index, 3);
    }

    #[test]
    fn resolve_by_title_returns_none_when_no_movements() {
        let work = mk_work(vec![]);
        assert!(resolve_by_title(&work, "anything").is_none());
    }

    #[test]
    fn resolve_by_title_returns_none_when_no_match() {
        let work = mk_work(mk_movements(&[
            (1, "Allegro", None),
            (2, "Andante", None),
        ]));
        assert!(resolve_by_title(&work, "Random title").is_none());
    }

    #[test]
    fn resolve_by_position_basic() {
        let work = mk_work(mk_movements(&[
            (1, "Allegro", None),
            (2, "Andante", None),
            (3, "Scherzo", None),
        ]));
        let ctx = resolve_by_position(&work, 1).unwrap();
        assert_eq!(ctx.index, 2);
        assert_eq!(ctx.method, ResolutionMethod::AlbumPosition);
        assert_eq!(ctx.title, "Andante");
    }

    #[test]
    fn resolve_by_position_out_of_bounds_is_none() {
        let work = mk_work(mk_movements(&[(1, "Allegro", None)]));
        assert!(resolve_by_position(&work, 5).is_none());
    }

    #[test]
    fn normalize_collapses_whitespace_and_drops_punct() {
        assert_eq!(normalize_for_match("Allegro,  ma non TROPPO!"), "allegro ma non troppo");
        assert_eq!(normalize_for_match("II. Adagio—cantabile"), "ii adagio cantabile");
    }

    #[test]
    fn normalize_folds_diacritics() {
        assert_eq!(normalize_for_match("Mañana à la française"), "manana a la francaise");
    }

    #[test]
    fn beethoven_5_iii_to_iv_attacca_scenario() {
        // Real-world: Beethoven Symphony 5, movement III is marked attacca
        // to IV. Track title from typical Tidal album: "III. Allegro".
        let work = mk_work(mk_movements(&[
            (1, "Allegro con brio", None),
            (2, "Andante con moto", None),
            (3, "Allegro", Some(4)),
            (4, "Allegro - Presto", None),
        ]));
        let ctx = resolve_by_title(&work, "III. Allegro").unwrap();
        assert_eq!(ctx.index, 3);
        assert_eq!(ctx.attacca_to, Some(4));
        assert_eq!(ctx.total, 4);
    }
}
