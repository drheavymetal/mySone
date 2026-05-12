//! Phase 5 (B5.1) — classical search: tokenizer + planner + executor.
//!
//! Reference: D-019. CLASSICAL_DESIGN.md §4.1 + §11 (Phase 5 gate).
//!
//! Strategy in three stages so each is testable in isolation:
//!
//!   1. `tokenize(query)` — purely lexical. Recognises composer surname
//!      (against an OpenOpus snapshot index), catalogue numbers
//!      (BWV/K/D/RV/Hob/HWV/Op + variants), key signatures, years, and
//!      free-text remainder. No I/O.
//!
//!   2. `plan(tokens, openopus)` — turns a token list into a
//!      `SearchPlan`: composer MBID (resolved if a surname matched),
//!      catalogue, keywords, year, key. No I/O.
//!
//!   3. `execute(plan, deps)` — fan-out:
//!      - Composer + catalogue/keywords → `list_works_by_composer` then
//!        score titles. The OpenOpus + MB cache already paid the cost.
//!      - Catalogue or keywords without composer → MB Lucene search on
//!        works (rate-limited).
//!
//!      Each result becomes a `SearchHit { work_mbid, title, score, ... }`.
//!
//! Bit-perfect contract: ZERO modification to audio routing. This module
//! is read-only over the catalog data the rest of the app already owns,
//! and never touches the writer thread, hw_volume, or signal_path.
//!
//! Code-style §1: braces always.

use serde::{Deserialize, Serialize};

use super::providers::openopus::OpenOpusProvider;
use super::types::{ComposerSummary, WorkSummary};

// ---------------------------------------------------------------------------
// Token model
// ---------------------------------------------------------------------------

/// Token recognised in a free-text search query. Lossless: the original
/// substring is preserved so the UI can show "we matched X here" chips.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum Token {
    /// Catalogue number with system letter + number, e.g. "BWV 1052".
    Catalogue {
        system: String,
        number: String,
        display: String,
    },
    /// Year between 1500 and 2100 inclusive.
    Year(i32),
    /// Key signature, e.g. "D minor".
    Key(String),
    /// Composer surname matched against the OpenOpus snapshot. Carries
    /// the surname text plus the resolved composer MBID.
    Composer { surname: String, mbid: String },
    /// Free-text token (anything left over).
    Keyword(String),
}

// ---------------------------------------------------------------------------
// Plan model
// ---------------------------------------------------------------------------

/// Resolved search plan — what the executor will actually use.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPlan {
    /// Composer MBID resolved from a surname token, if any.
    pub composer_mbid: Option<String>,
    /// Composer name to display (resolved from snapshot). Empty when not
    /// matched.
    pub composer_name: Option<String>,
    /// Catalogue number, e.g. "BWV 1052" or "Op. 125".
    pub catalogue: Option<CataloguePlan>,
    /// Year filter, e.g. 1962.
    pub year: Option<i32>,
    /// Key, e.g. "D minor".
    pub key: Option<String>,
    /// Free-text keywords joined by spaces. Used as the fallback search.
    pub keywords: String,
    /// Tokens we recognised, surfaced to the UI as "Detected: ..." chips.
    pub tokens: Vec<Token>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CataloguePlan {
    pub system: String,
    pub number: String,
    pub display: String,
}

// ---------------------------------------------------------------------------
// Search hit
// ---------------------------------------------------------------------------

/// A single result row returned to the frontend. Lightweight — the UI
/// navigates to the WorkPage on click and that page hydrates everything
/// else via `getClassicalWork(mbid)`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub work_mbid: String,
    pub title: String,
    /// Composer name when known. `None` for free-text matches that
    /// MB returned without a composer-rel.
    pub composer_name: Option<String>,
    pub composer_mbid: Option<String>,
    pub catalogue_display: Option<String>,
    /// Score in [0, 1]. Higher is better. The frontend sorts by it.
    pub score: f64,
    /// Source of the hit, surfaced for debug/UX:
    /// `"snapshot"` | `"mb-lucene"` | `"composer-list"`.
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResults {
    pub plan: SearchPlan,
    pub hits: Vec<SearchHit>,
}

// ---------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------

/// Recognised catalogue systems, in priority order so "Op. 125" beats a
/// loose "Op" alone. Each entry is the system letter as displayed.
const CATALOGUE_SYSTEMS: &[&str] =
    &["BWV", "HWV", "Hob", "RV", "K", "D", "WoO", "Op"];

/// Recognised tonal keys. Both ASCII (`Db`, `F#`) and unicode (`D♭`,
/// `F♯`) variants are accepted by the regex below.
const KEY_LETTERS: &[char] = &['A', 'B', 'C', 'D', 'E', 'F', 'G'];
const KEY_MODIFIERS: &[&str] = &["♭", "♯", "b", "#"];
const KEY_QUALIFIERS: &[&str] = &["minor", "major", "min", "maj", "m"];

/// Tokenize a free-text query. Pure: no I/O. The composer index comes
/// from `OpenOpusProvider::top_composers` — we accept it as a parameter
/// so tests can inject a fixture without touching the embedded snapshot.
pub fn tokenize(
    query: &str,
    composer_index: &[ComposerSummary],
) -> Vec<Token> {
    let mut tokens: Vec<Token> = Vec::new();
    // Track which char positions we've already consumed so later passes
    // don't re-tokenize them. This avoids double-counting "1962" both as
    // a year and as part of free text.
    let mut consumed: Vec<bool> = vec![false; query.len()];

    // Pass 1: catalogue numbers. Greedy-longest system letter so "Op. 125"
    // matches before "OpusXX".
    for system in CATALOGUE_SYSTEMS {
        let mut start_search = 0usize;
        while let Some(found) = find_catalogue_at(query, system, start_search, &consumed) {
            let (start, end, number) = found;
            let display = format!("{system} {number}");
            tokens.push(Token::Catalogue {
                system: (*system).to_string(),
                number,
                display,
            });
            for c in consumed.iter_mut().take(end).skip(start) {
                *c = true;
            }
            start_search = end;
        }
    }

    // Pass 2: years (4 digits in 1500..=2100).
    let bytes = query.as_bytes();
    let mut i = 0usize;
    while i + 4 <= bytes.len() {
        if !consumed[i] && is_year_at(bytes, i) {
            // Check word boundary on both ends.
            let before = if i == 0 {
                true
            } else {
                !bytes[i - 1].is_ascii_alphanumeric()
            };
            let after = if i + 4 == bytes.len() {
                true
            } else {
                !bytes[i + 4].is_ascii_alphanumeric()
            };
            if before && after {
                let s = &query[i..i + 4];
                if let Ok(year) = s.parse::<i32>() {
                    tokens.push(Token::Year(year));
                    for c in consumed.iter_mut().take(i + 4).skip(i) {
                        *c = true;
                    }
                    i += 4;
                    continue;
                }
            }
        }
        i += 1;
    }

    // Pass 3: keys ("D minor", "C# major", "E♭ major", "Bb m").
    if let Some((start, end, key_display)) = find_key(query, &consumed) {
        tokens.push(Token::Key(key_display));
        for c in consumed.iter_mut().take(end).skip(start) {
            *c = true;
        }
    }

    // Pass 4: composer surnames. Try each top-N composer surname against
    // remaining text (lowercase substring on word-boundary). First match
    // wins per query — handles "Beethoven 9 Karajan" without conflating
    // "Karajan" as composer.
    let lower = query.to_lowercase();
    for c in composer_index.iter() {
        let surname = surname_of(&c.name);
        if surname.is_empty() {
            continue;
        }
        let target = surname.to_lowercase();
        if let Some(pos) = find_word_at(&lower, &target, &consumed) {
            tokens.push(Token::Composer {
                surname: surname.to_string(),
                mbid: c.mbid.clone(),
            });
            for cell in consumed.iter_mut().take(pos + target.len()).skip(pos) {
                *cell = true;
            }
            break;
        }
    }

    // Pass 5: keywords — split remaining text on whitespace + punctuation.
    let mut buf = String::new();
    for (idx, ch) in query.char_indices() {
        let in_consumed = idx < consumed.len() && consumed[idx];
        let is_word = ch.is_alphanumeric();
        if in_consumed || !is_word {
            if !buf.is_empty() {
                tokens.push(Token::Keyword(buf.clone()));
                buf.clear();
            }
        } else {
            buf.push(ch);
        }
    }
    if !buf.is_empty() {
        tokens.push(Token::Keyword(buf));
    }

    tokens
}

/// Find a catalogue substring like `"Op. 125"` starting at or after
/// `from`. Returns `(start, end, number)`.
fn find_catalogue_at(
    query: &str,
    system: &str,
    from: usize,
    consumed: &[bool],
) -> Option<(usize, usize, String)> {
    let q = query.as_bytes();
    let s_lower = system.to_lowercase();
    let s_lower_b = s_lower.as_bytes();
    let qlen = q.len();
    let slen = s_lower_b.len();

    let mut i = from;
    while i + slen <= qlen {
        // Skip if any byte in the candidate window is consumed.
        let mut window_consumed = false;
        for c in consumed.iter().take(i + slen).skip(i) {
            if *c {
                window_consumed = true;
                break;
            }
        }
        if window_consumed {
            i += 1;
            continue;
        }

        // Case-insensitive match on the system letter(s).
        let mut hit = true;
        for j in 0..slen {
            if q[i + j].to_ascii_lowercase() != s_lower_b[j] {
                hit = false;
                break;
            }
        }
        if !hit {
            i += 1;
            continue;
        }

        // Word boundary before.
        if i > 0 && q[i - 1].is_ascii_alphanumeric() {
            i += 1;
            continue;
        }

        // After the system letter we expect optional dot + whitespace +
        // digits. If the next char is alphanumeric (and not a digit) it's
        // not a catalogue (e.g. "Operatic" should not match "Op").
        let mut k = i + slen;
        if k < qlen && q[k] == b'.' {
            k += 1;
        }
        // Whitespace tolerated.
        while k < qlen && (q[k] == b' ' || q[k] == b'\t') {
            k += 1;
        }
        // Must hit a digit.
        if k >= qlen || !q[k].is_ascii_digit() {
            i += 1;
            continue;
        }
        // Read digits + optional `a/b` suffix common in MB.
        let num_start = k;
        while k < qlen && q[k].is_ascii_digit() {
            k += 1;
        }
        if k < qlen && (q[k] == b'a' || q[k] == b'b') {
            // Only accept the suffix if followed by a non-letter.
            if k + 1 == qlen || !q[k + 1].is_ascii_alphabetic() {
                k += 1;
            }
        }
        // Word boundary after.
        if k < qlen && q[k].is_ascii_alphabetic() {
            i += 1;
            continue;
        }
        let number = query[num_start..k].to_string();
        return Some((i, k, number));
    }
    None
}

fn is_year_at(bytes: &[u8], i: usize) -> bool {
    if i + 4 > bytes.len() {
        return false;
    }
    let first = bytes[i];
    if !(first == b'1' || first == b'2') {
        return false;
    }
    for k in 0..4 {
        if !bytes[i + k].is_ascii_digit() {
            return false;
        }
    }
    let s = std::str::from_utf8(&bytes[i..i + 4]).unwrap_or("0000");
    if let Ok(y) = s.parse::<u32>() {
        return (1500..=2100).contains(&y);
    }
    false
}

/// Find a key substring like "D minor", "C# major", "Eb m". Returns
/// `(start, end, normalized_display)`.
fn find_key(query: &str, consumed: &[bool]) -> Option<(usize, usize, String)> {
    let chars: Vec<char> = query.chars().collect();
    let n = chars.len();
    let mut byte_offsets: Vec<usize> = Vec::with_capacity(n + 1);
    let mut acc = 0usize;
    for ch in chars.iter() {
        byte_offsets.push(acc);
        acc += ch.len_utf8();
    }
    byte_offsets.push(acc);

    let mut i = 0usize;
    while i < n {
        if i < byte_offsets.len() && byte_offsets[i] < consumed.len() && consumed[byte_offsets[i]] {
            i += 1;
            continue;
        }
        let letter = chars[i];
        if !KEY_LETTERS.contains(&letter) {
            i += 1;
            continue;
        }
        // Must be at a word boundary on the left.
        if i > 0 && chars[i - 1].is_alphanumeric() {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        // Optional accidental.
        let mut accidental = String::new();
        if j < n {
            let next = chars[j].to_string();
            for m in KEY_MODIFIERS.iter() {
                if next == *m {
                    accidental = m.to_string();
                    j += 1;
                    break;
                }
            }
        }
        // Whitespace separator.
        if j >= n || !chars[j].is_whitespace() {
            i += 1;
            continue;
        }
        while j < n && chars[j].is_whitespace() {
            j += 1;
        }
        // Qualifier (minor/major/m/maj/min).
        let mut matched_qual: Option<&'static str> = None;
        let remaining: String = chars[j..].iter().collect();
        let lower = remaining.to_lowercase();
        for q in KEY_QUALIFIERS.iter() {
            if lower.starts_with(q) {
                let after_idx = j + q.chars().count();
                let valid_after = after_idx == n
                    || !chars[after_idx].is_alphanumeric();
                if valid_after {
                    matched_qual = Some(canonical_qualifier(q));
                    j = after_idx;
                    break;
                }
            }
        }
        let qualifier = match matched_qual {
            Some(q) => q,
            None => {
                i += 1;
                continue;
            }
        };
        let display = format!("{}{} {}", letter, accidental, qualifier);
        return Some((byte_offsets[i], byte_offsets[j], display));
    }
    None
}

fn canonical_qualifier(q: &str) -> &'static str {
    match q {
        "major" | "maj" => "major",
        "minor" | "min" | "m" => "minor",
        _ => "minor",
    }
}

/// Take the last whitespace-separated token of `name` as the surname.
/// "Ludwig van Beethoven" → "Beethoven", "J. S. Bach" → "Bach".
fn surname_of(name: &str) -> &str {
    name.split_whitespace().last().unwrap_or("")
}

/// Find `target` inside `haystack` at a word boundary, skipping
/// already-consumed positions. `haystack` is expected lowercase.
fn find_word_at(haystack: &str, target: &str, consumed: &[bool]) -> Option<usize> {
    if target.is_empty() {
        return None;
    }
    let mut from = 0usize;
    while let Some(pos) = haystack[from..].find(target) {
        let abs = from + pos;
        // Word boundary check.
        let before_ok = abs == 0
            || !haystack.as_bytes()[abs - 1].is_ascii_alphanumeric();
        let after_idx = abs + target.len();
        let after_ok = after_idx == haystack.len()
            || !haystack.as_bytes()[after_idx].is_ascii_alphanumeric();
        // Consumed check.
        let mut not_consumed = true;
        for c in consumed.iter().take(after_idx).skip(abs) {
            if *c {
                not_consumed = false;
                break;
            }
        }
        if before_ok && after_ok && not_consumed {
            return Some(abs);
        }
        from = abs + 1;
    }
    None
}

// ---------------------------------------------------------------------------
// Planner
// ---------------------------------------------------------------------------

/// Build a `SearchPlan` from a list of tokens. Pure.
pub fn plan(tokens: Vec<Token>, openopus: &OpenOpusProvider) -> SearchPlan {
    let mut composer_mbid: Option<String> = None;
    let mut composer_name: Option<String> = None;
    let mut catalogue: Option<CataloguePlan> = None;
    let mut year: Option<i32> = None;
    let mut key: Option<String> = None;
    let mut keywords: Vec<String> = Vec::new();

    for tok in tokens.iter() {
        match tok {
            Token::Composer { mbid, surname } => {
                if composer_mbid.is_none() {
                    composer_mbid = Some(mbid.clone());
                    composer_name = openopus
                        .lookup_composer_summary(mbid)
                        .map(|s| s.full_name.unwrap_or(s.name))
                        .or_else(|| Some(surname.clone()));
                }
            }
            Token::Catalogue {
                system,
                number,
                display,
            } => {
                if catalogue.is_none() {
                    catalogue = Some(CataloguePlan {
                        system: system.clone(),
                        number: number.clone(),
                        display: display.clone(),
                    });
                }
            }
            Token::Year(y) => {
                if year.is_none() {
                    year = Some(*y);
                }
            }
            Token::Key(k) => {
                if key.is_none() {
                    key = Some(k.clone());
                }
            }
            Token::Keyword(k) => {
                keywords.push(k.clone());
            }
        }
    }

    SearchPlan {
        composer_mbid,
        composer_name,
        catalogue,
        year,
        key,
        keywords: keywords.join(" "),
        tokens,
    }
}

// ---------------------------------------------------------------------------
// Scoring
// ---------------------------------------------------------------------------

/// Score a `WorkSummary` against a `SearchPlan` in [0, 1]. Higher is
/// better. Components:
///   * catalogue_match (0..=0.5)
///   * title_match     (0..=0.3)
///   * year_match      (0..=0.1) — only when both plan and work have a year
///   * composer_match  (0..=0.1)
pub fn score_work(work: &WorkSummary, plan: &SearchPlan) -> f64 {
    let mut score: f64 = 0.0;

    // Catalogue: exact (system+number) → 0.5; system-only → 0.1.
    if let (Some(plan_cat), Some(work_cat)) = (&plan.catalogue, &work.catalogue_number) {
        if normalize_system(&plan_cat.system) == normalize_system(&work_cat.system) {
            if normalize_number(&plan_cat.number) == normalize_number(&work_cat.number) {
                score += 0.5;
            } else {
                score += 0.1;
            }
        }
    }

    // Title: substring scan over plan keywords. Single-digit tokens
    // ("9", "5") are kept because classical UX is "Symphony 9" — those
    // are critical disambiguators between Sym 5 and Sym 9. We do filter
    // out single-letter alphabetic noise.
    let work_title_lower = work.title.to_lowercase();
    let kws = plan.keywords.to_lowercase();
    if !kws.is_empty() {
        let kws_tokens: Vec<&str> = kws
            .split_whitespace()
            .filter(|t| t.len() > 1 || t.chars().all(|c| c.is_ascii_digit()))
            .collect();
        if !kws_tokens.is_empty() {
            let mut matched: usize = 0;
            for k in kws_tokens.iter() {
                if title_contains_token(&work_title_lower, k) {
                    matched += 1;
                }
            }
            score += 0.3 * (matched as f64 / kws_tokens.len() as f64);
        }
    }

    // Year: only contributes when both sides know the year.
    if let (Some(py), Some(wy)) = (plan.year, work.composition_year) {
        let diff = (py - wy).abs();
        if diff <= 2 {
            score += 0.1;
        } else if diff <= 10 {
            score += 0.05;
        }
    }

    // Composer match: if plan has a composer and the work belongs to it,
    // 0.1. The composer-list executor pre-filters by composer so this is
    // mostly a tie-breaker for the MB Lucene path.
    if let (Some(pmbid), Some(wmbid)) = (&plan.composer_mbid, &work.composer_mbid) {
        if pmbid == wmbid {
            score += 0.1;
        }
    }

    // Hard cap.
    if score > 1.0 {
        score = 1.0;
    }
    score
}

/// Substring check that requires word-boundaries when the token is
/// purely numeric — so "9" matches "Symphony No. 9" but not "1962" or
/// "1990s". Alphabetic / mixed tokens fall back to plain substring.
fn title_contains_token(title_lower: &str, token: &str) -> bool {
    if token.is_empty() {
        return false;
    }
    let needs_boundary = token.chars().all(|c| c.is_ascii_digit());
    if !needs_boundary {
        return title_lower.contains(token);
    }
    let bytes = title_lower.as_bytes();
    let tlen = token.len();
    let mut from = 0usize;
    while let Some(rel) = title_lower[from..].find(token) {
        let abs = from + rel;
        let before_ok = abs == 0 || !bytes[abs - 1].is_ascii_alphanumeric();
        let after_idx = abs + tlen;
        let after_ok = after_idx == bytes.len()
            || !bytes[after_idx].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
        from = abs + 1;
    }
    false
}

/// Normalise catalogue system letter for comparison: "Op." → "OP",
/// "BWV" → "BWV", "WoO" → "WOO".
fn normalize_system(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphabetic())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

fn normalize_number(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_lowercase()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classical::types::{CatalogueNumber, Era};

    fn fake_composers() -> Vec<ComposerSummary> {
        // A small in-test composer index. Real OpenOpus is exercised
        // separately in providers::openopus tests.
        vec![
            ComposerSummary {
                mbid: "1f9df192-a621-4f54-8850-2c5373b7eac9".to_string(),
                open_opus_id: None,
                name: "Beethoven".to_string(),
                full_name: Some("Ludwig van Beethoven".to_string()),
                birth_year: Some(1770),
                death_year: Some(1827),
                era: Era::EarlyRomantic,
                portrait_url: None,
                popular: true,
            },
            ComposerSummary {
                mbid: "24f1766e-9635-4d58-a4d4-9413f9f98a4c".to_string(),
                open_opus_id: None,
                name: "Bach".to_string(),
                full_name: Some("Johann Sebastian Bach".to_string()),
                birth_year: Some(1685),
                death_year: Some(1750),
                era: Era::Baroque,
                portrait_url: None,
                popular: true,
            },
            ComposerSummary {
                mbid: "b972f589-fb0e-474e-b64a-803b0364fa75".to_string(),
                open_opus_id: None,
                name: "Mozart".to_string(),
                full_name: Some("Wolfgang Amadeus Mozart".to_string()),
                birth_year: Some(1756),
                death_year: Some(1791),
                era: Era::Classical,
                portrait_url: None,
                popular: true,
            },
        ]
    }

    fn make_work(
        title: &str,
        composer_mbid: Option<&str>,
        catalogue: Option<(&str, &str)>,
        year: Option<i32>,
    ) -> WorkSummary {
        let mbid = format!("work-{}", title.replace(' ', "-").to_lowercase());
        let catalogue_number = catalogue.map(|(sys, num)| CatalogueNumber {
            system: sys.to_string(),
            number: num.to_string(),
            display: format!("{sys} {num}"),
        });
        WorkSummary {
            mbid,
            title: title.to_string(),
            composer_mbid: composer_mbid.map(String::from),
            composer_name: None,
            catalogue_number,
            key: None,
            work_type: None,
            genre: None,
            bucket: None,
            composition_year: year,
            popular: false,
        }
    }

    // -----------------------------------------------------------------
    // Tokenizer tests
    // -----------------------------------------------------------------

    #[test]
    fn tokenize_catalogue_op() {
        let toks = tokenize("Op. 125", &[]);
        assert_eq!(toks.len(), 1);
        match &toks[0] {
            Token::Catalogue { system, number, display } => {
                assert_eq!(system, "Op");
                assert_eq!(number, "125");
                assert_eq!(display, "Op 125");
            }
            t => panic!("unexpected token: {t:?}"),
        }
    }

    #[test]
    fn tokenize_catalogue_bwv() {
        let toks = tokenize("BWV 1052", &[]);
        let cat = toks.iter().find_map(|t| match t {
            Token::Catalogue { system, number, .. } => Some((system.clone(), number.clone())),
            _ => None,
        });
        assert_eq!(cat, Some(("BWV".to_string(), "1052".to_string())));
    }

    #[test]
    fn tokenize_catalogue_k_dotted() {
        let toks = tokenize("K. 466", &[]);
        let cat = toks.iter().find_map(|t| match t {
            Token::Catalogue { system, number, .. } => Some((system.clone(), number.clone())),
            _ => None,
        });
        assert_eq!(cat, Some(("K".to_string(), "466".to_string())));
    }

    #[test]
    fn tokenize_year_only() {
        let toks = tokenize("1962", &[]);
        assert_eq!(toks.len(), 1);
        assert!(matches!(toks[0], Token::Year(1962)));
    }

    #[test]
    fn tokenize_year_inside_text() {
        let toks = tokenize("Karajan 1962 Berlin", &[]);
        let years: Vec<i32> = toks
            .iter()
            .filter_map(|t| match t {
                Token::Year(y) => Some(*y),
                _ => None,
            })
            .collect();
        assert_eq!(years, vec![1962]);
        // "Berlin" should remain as keyword.
        let kws: Vec<String> = toks
            .iter()
            .filter_map(|t| match t {
                Token::Keyword(k) => Some(k.clone()),
                _ => None,
            })
            .collect();
        assert!(kws.contains(&"Karajan".to_string()));
        assert!(kws.contains(&"Berlin".to_string()));
    }

    #[test]
    fn tokenize_year_out_of_range_is_keyword() {
        let toks = tokenize("Track 9999", &[]);
        let has_year = toks.iter().any(|t| matches!(t, Token::Year(_)));
        assert!(!has_year, "9999 must not be classified as a year");
    }

    #[test]
    fn tokenize_key_d_minor() {
        let toks = tokenize("Symphony D minor", &[]);
        let key = toks.iter().find_map(|t| match t {
            Token::Key(k) => Some(k.clone()),
            _ => None,
        });
        assert_eq!(key, Some("D minor".to_string()));
    }

    #[test]
    fn tokenize_key_with_sharp() {
        let toks = tokenize("Piano Sonata in C# minor", &[]);
        let key = toks.iter().find_map(|t| match t {
            Token::Key(k) => Some(k.clone()),
            _ => None,
        });
        assert_eq!(key, Some("C# minor".to_string()));
    }

    #[test]
    fn tokenize_composer_surname_resolves_mbid() {
        let composers = fake_composers();
        let toks = tokenize("Beethoven 9", &composers);
        let composer = toks.iter().find_map(|t| match t {
            Token::Composer { surname, mbid } => Some((surname.clone(), mbid.clone())),
            _ => None,
        });
        assert_eq!(
            composer,
            Some((
                "Beethoven".to_string(),
                "1f9df192-a621-4f54-8850-2c5373b7eac9".to_string(),
            ))
        );
    }

    #[test]
    fn tokenize_first_composer_match_wins() {
        // "Karajan" is not a composer in the fixture, so no Composer token.
        // But "Beethoven" is, and it must be the first one hit.
        let composers = fake_composers();
        let toks = tokenize("Beethoven 9 Karajan 1962", &composers);
        let composers_in_query: Vec<String> = toks
            .iter()
            .filter_map(|t| match t {
                Token::Composer { surname, .. } => Some(surname.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(composers_in_query, vec!["Beethoven".to_string()]);
    }

    #[test]
    fn tokenize_full_query_phase_5_acceptance() {
        // The CLASSICAL_DESIGN.md §11 acceptance query for Phase 5.
        let composers = fake_composers();
        let toks = tokenize("Beethoven 9 Karajan 1962", &composers);

        // Should produce: Composer(Beethoven), Year(1962), Keyword(9), Keyword(Karajan).
        let has_composer = toks.iter().any(|t| {
            matches!(t, Token::Composer { surname, .. } if surname == "Beethoven")
        });
        let has_year = toks.iter().any(|t| matches!(t, Token::Year(1962)));
        let kws: Vec<String> = toks
            .iter()
            .filter_map(|t| match t {
                Token::Keyword(k) => Some(k.clone()),
                _ => None,
            })
            .collect();
        assert!(has_composer, "Beethoven must resolve to a Composer token");
        assert!(has_year, "1962 must resolve to a Year token");
        assert!(kws.contains(&"9".to_string()), "9 stays as keyword");
        assert!(
            kws.contains(&"Karajan".to_string()),
            "Karajan stays as keyword (no composer match)"
        );
    }

    #[test]
    fn tokenize_does_not_match_op_inside_word() {
        // "Operatic" should NOT match "Op" as catalogue.
        let toks = tokenize("Operatic showpiece", &[]);
        let has_cat = toks.iter().any(|t| matches!(t, Token::Catalogue { .. }));
        assert!(!has_cat, "Operatic must not be tokenised as Op catalogue");
    }

    #[test]
    fn tokenize_empty_query() {
        let toks = tokenize("", &fake_composers());
        assert!(toks.is_empty());
    }

    #[test]
    fn tokenize_all_whitespace() {
        let toks = tokenize("   ", &fake_composers());
        assert!(toks.is_empty());
    }

    #[test]
    fn tokenize_catalogue_with_year_and_keyword() {
        let toks = tokenize("Symphony No. 9 Op. 125 Karajan", &[]);
        let has_op_125 = toks.iter().any(|t| {
            matches!(
                t,
                Token::Catalogue { system, number, .. }
                    if system == "Op" && number == "125"
            )
        });
        assert!(has_op_125, "Op. 125 must be detected");
    }

    // -----------------------------------------------------------------
    // Planner tests
    // -----------------------------------------------------------------

    #[test]
    fn plan_builds_from_full_query() {
        let composers = fake_composers();
        let toks = tokenize("Beethoven Op. 125 1962", &composers);
        let oo = OpenOpusProvider::new();
        let p = plan(toks, &oo);

        assert!(p.composer_mbid.is_some());
        assert_eq!(p.catalogue.as_ref().map(|c| c.display.clone()),
                   Some("Op 125".to_string()));
        assert_eq!(p.year, Some(1962));
    }

    #[test]
    fn plan_keywords_collected_in_order() {
        let toks = tokenize("Symphony 5 Karajan", &fake_composers());
        let oo = OpenOpusProvider::new();
        let p = plan(toks, &oo);
        // "5" stays as keyword (not catalogue without system letter), and
        // Karajan + Symphony are keywords.
        assert!(p.keywords.contains("Symphony"));
        assert!(p.keywords.contains("Karajan"));
    }

    // -----------------------------------------------------------------
    // Scoring tests
    // -----------------------------------------------------------------

    #[test]
    fn score_work_exact_catalogue_dominates() {
        let beet_mbid = "1f9df192-a621-4f54-8850-2c5373b7eac9";
        let sym9 = make_work(
            "Symphony No. 9 in D minor",
            Some(beet_mbid),
            Some(("Op", "125")),
            Some(1824),
        );
        let sym1 = make_work(
            "Symphony No. 1 in C major",
            Some(beet_mbid),
            Some(("Op", "21")),
            Some(1800),
        );
        let composers = fake_composers();
        let toks = tokenize("Beethoven Op. 125", &composers);
        let oo = OpenOpusProvider::new();
        let p = plan(toks, &oo);

        let s9 = score_work(&sym9, &p);
        let s1 = score_work(&sym1, &p);
        assert!(
            s9 > s1,
            "Op. 125 must outscore Op. 21 (got s9={s9}, s1={s1})"
        );
        // sym1's score = 0.1 (system match Op alone) + 0.1 (composer)
        // sym9's score = 0.5 (exact Op+125) + 0.1 (composer) = 0.6+
        assert!(s9 >= 0.5, "exact catalogue match must contribute ≥ 0.5");
    }

    #[test]
    fn score_work_keyword_only() {
        let work = make_work("Symphony No. 9 in D minor", None, None, None);
        let composers = fake_composers();
        let toks = tokenize("Symphony 9", &composers);
        let oo = OpenOpusProvider::new();
        let p = plan(toks, &oo);
        let s = score_work(&work, &p);
        // Both "symphony" and "9" should match → 0.3 * 1.0 = 0.3.
        assert!(s >= 0.25, "keyword match must contribute (got {s})");
    }

    #[test]
    fn score_work_year_within_two_years() {
        let work = make_work(
            "Symphony No. 9 in D minor",
            None,
            Some(("Op", "125")),
            Some(1824),
        );
        let composers = fake_composers();
        let toks = tokenize("Op. 125 1825", &composers);
        let oo = OpenOpusProvider::new();
        let p = plan(toks, &oo);
        let s = score_work(&work, &p);
        // 0.5 (exact catalogue) + 0.1 (year ±2) = 0.6.
        assert!(s >= 0.6, "year within 2 years must contribute 0.1 (got {s})");
    }

    #[test]
    fn score_work_caps_at_one() {
        let beet_mbid = "1f9df192-a621-4f54-8850-2c5373b7eac9";
        let work = make_work(
            "Symphony No. 9 in D minor",
            Some(beet_mbid),
            Some(("Op", "125")),
            Some(1824),
        );
        let composers = fake_composers();
        let toks = tokenize("Beethoven Symphony 9 D minor Op. 125 1824", &composers);
        let oo = OpenOpusProvider::new();
        let p = plan(toks, &oo);
        let s = score_work(&work, &p);
        assert!(s <= 1.0, "score must cap at 1.0 (got {s})");
        assert!(s > 0.85, "this case should approach 1.0 (got {s})");
    }

    // -----------------------------------------------------------------
    // Phase 5 acceptance — D-019 contract
    // -----------------------------------------------------------------

    #[test]
    fn phase5_acceptance_op_125_resolves_to_beethoven_9() {
        // The acceptance criteria of Phase 5 says that "Op. 125" must
        // surface Beethoven's Symphony No. 9 as the top hit. We model the
        // executor's input — a list of candidate works — and verify the
        // scoring picks the right one. This is the deterministic half of
        // the gate (the executor stage is exercised in catalog tests).
        let beet_mbid = "1f9df192-a621-4f54-8850-2c5373b7eac9";
        let candidates = vec![
            make_work(
                "Symphony No. 9 in D minor",
                Some(beet_mbid),
                Some(("Op", "125")),
                Some(1824),
            ),
            make_work(
                "Symphony No. 5 in C minor",
                Some(beet_mbid),
                Some(("Op", "67")),
                Some(1808),
            ),
            make_work(
                "Piano Sonata No. 14 'Moonlight'",
                Some(beet_mbid),
                Some(("Op", "27")),
                Some(1801),
            ),
        ];
        let composers = fake_composers();
        let toks = tokenize("Op. 125", &composers);
        let oo = OpenOpusProvider::new();
        let p = plan(toks, &oo);

        let mut scored: Vec<(f64, &WorkSummary)> = candidates
            .iter()
            .map(|w| (score_work(w, &p), w))
            .collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        assert_eq!(scored[0].1.title, "Symphony No. 9 in D minor");
        // It must clear the threshold; we use 0.5 because Op. 125 alone
        // gives the catalogue match without composer help.
        assert!(scored[0].0 >= 0.5, "top score must be ≥ 0.5 (got {})", scored[0].0);
    }

    #[test]
    fn phase5_acceptance_beethoven_9_karajan_1962_resolves_top_match() {
        // The full §11 query: "Beethoven 9 Karajan 1962" must rank
        // Symphony No. 9 above Symphony No. 5. We don't model recordings
        // here — Karajan/1962 disambiguates *recordings*, not works —
        // but the work-level rank is the necessary precondition.
        let beet_mbid = "1f9df192-a621-4f54-8850-2c5373b7eac9";
        let candidates = vec![
            make_work(
                "Symphony No. 9 in D minor",
                Some(beet_mbid),
                Some(("Op", "125")),
                Some(1824),
            ),
            make_work(
                "Symphony No. 5 in C minor",
                Some(beet_mbid),
                Some(("Op", "67")),
                Some(1808),
            ),
        ];
        let composers = fake_composers();
        let toks = tokenize("Beethoven 9 Karajan 1962", &composers);
        let oo = OpenOpusProvider::new();
        let p = plan(toks, &oo);

        let s9 = score_work(&candidates[0], &p);
        let s5 = score_work(&candidates[1], &p);
        assert!(
            s9 > s5,
            "Symphony 9 must outrank Symphony 5 with 'Beethoven 9 ...' query (s9={s9}, s5={s5})"
        );
    }

    #[test]
    fn normalize_helpers() {
        assert_eq!(normalize_system("Op."), "OP");
        assert_eq!(normalize_system("op"), "OP");
        assert_eq!(normalize_system("BWV"), "BWV");
        assert_eq!(normalize_number("125"), "125");
        assert_eq!(normalize_number("125a"), "125a");
    }

    // ---------------------------------------------------------------
    // Phase 7 (D-031) — extended composer index tests.
    //
    // Verifies that composers outside the OpenOpus canon (e.g. fed in
    // via `ExtendedComposersProvider`) tokenize correctly when the
    // caller passes a wider `composer_index`.
    // ---------------------------------------------------------------

    fn extended_fake_composers() -> Vec<ComposerSummary> {
        // A small fixture standing in for the extended snapshot. Includes
        // composers that are NOT in the OpenOpus canon-33.
        let mut out = fake_composers();
        out.extend(vec![
            ComposerSummary {
                mbid: "456596a9-1d4f-4b47-b4e0-ac402ca672b0".to_string(),
                open_opus_id: None,
                name: "Saariaho".to_string(),
                full_name: Some("Kaija Saariaho".to_string()),
                birth_year: Some(1952),
                death_year: Some(2023),
                era: Era::Contemporary,
                portrait_url: None,
                popular: false,
            },
            ComposerSummary {
                mbid: "b37b4bed-a7ec-4d2f-940b-7fad6d308c6e".to_string(),
                open_opus_id: None,
                name: "Shaw".to_string(),
                full_name: Some("Caroline Shaw".to_string()),
                birth_year: Some(1982),
                death_year: None,
                era: Era::Contemporary,
                portrait_url: None,
                popular: false,
            },
            ComposerSummary {
                mbid: "00000000-0000-0000-0000-000000000bin".to_string(),
                open_opus_id: None,
                // Mononame composer — test that surname extraction
                // handles single-word names gracefully.
                name: "Hildegard".to_string(),
                full_name: Some("Hildegard von Bingen".to_string()),
                birth_year: Some(1098),
                death_year: Some(1179),
                era: Era::Medieval,
                portrait_url: None,
                popular: false,
            },
        ]);
        out
    }

    #[test]
    fn tokenize_extended_composer_saariaho_resolves() {
        let composers = extended_fake_composers();
        let toks = tokenize("Saariaho", &composers);
        let composer = toks.iter().find_map(|t| match t {
            Token::Composer { surname, mbid } => Some((surname.clone(), mbid.clone())),
            _ => None,
        });
        assert_eq!(
            composer,
            Some((
                "Saariaho".to_string(),
                "456596a9-1d4f-4b47-b4e0-ac402ca672b0".to_string(),
            ))
        );
    }

    #[test]
    fn tokenize_extended_composer_caroline_shaw_resolves() {
        // Multi-token name. Tokenizer matches the surname.
        let composers = extended_fake_composers();
        let toks = tokenize("Caroline Shaw partita", &composers);
        let has_shaw = toks.iter().any(|t| matches!(
            t,
            Token::Composer { surname, .. } if surname == "Shaw"
        ));
        assert!(
            has_shaw,
            "Caroline Shaw should tokenize as composer when extended index is provided"
        );
    }

    #[test]
    fn tokenize_extended_composer_hildegard_resolves() {
        let composers = extended_fake_composers();
        let toks = tokenize("Hildegard", &composers);
        let has_hildegard = toks.iter().any(|t| matches!(
            t,
            Token::Composer { surname, .. } if surname.contains("Hildegard")
        ));
        assert!(
            has_hildegard,
            "Hildegard von Bingen should tokenize when extended index is provided"
        );
    }

    #[test]
    fn tokenize_unknown_composer_outside_index_falls_through_to_keyword() {
        // A composer not in either canon nor extended fixtures should
        // surface as keywords, not as a Composer token. This validates
        // the tokenizer doesn't hallucinate Composer tokens.
        let composers = extended_fake_composers();
        let toks = tokenize("Stockhausen Hymnen", &composers);
        let has_composer = toks.iter().any(|t| matches!(t, Token::Composer { .. }));
        assert!(
            !has_composer,
            "Stockhausen is not in our test fixtures so should not be tokenized as composer"
        );
    }
}
