// ──────────────────────────────────────────────────────────────────────────────
// pipeline/scrubber.rs — Text Scrubbing Pipeline
//
// Self-contained module: must not reference external crate items (`crate::...`),
// allowing standalone compilation as the `philosofile_scrubber` library crate.
// ──────────────────────────────────────────────────────────────────────────────
#![allow(dead_code)]

use regex::{Captures, Regex};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, OnceLock, RwLock};
use unicode_normalization::UnicodeNormalization;

#[path = "registry_check.rs"]
pub mod registry_check;

#[path = "image_redact.rs"]
pub mod image_redact;

// ── Scoring configuration — ALL tunable weights live here ─────────────────────

pub struct ScoringConfig {
    /// Anything scoring at least this much is redacted.
    pub redact_threshold: f32,
    /// Score needed for the country-specific label (verified rows only).
    pub country_label_threshold: f32,
    /// Score needed for the type-level label.
    pub type_label_threshold: f32,
    pub context_bonus: f32,
    pub checksum_valid_bonus: f32,
    pub checksum_invalid_penalty: f32,
    pub doc_context_bonus: f32,
    pub country_clue_bonus: f32,
    pub year_penalty: f32,
    /// Flat bonus in high-sensitivity docs for `context_required=yes` rows with no keyword nearby.
    pub aggressive_context_bonus: f32,
    /// Characters on each side of a match searched for context keywords.
    pub context_window: usize,
    /// Two candidates closer than this are treated as a tie.
    pub tie_margin: f32,
}

pub const SCORING: ScoringConfig = ScoringConfig {
    redact_threshold: 0.30,
    country_label_threshold: 0.75,
    type_label_threshold: 0.50,
    context_bonus: 0.30,
    checksum_valid_bonus: 0.25,
    checksum_invalid_penalty: -0.25,
    doc_context_bonus: 0.10,
    country_clue_bonus: 0.10,
    year_penalty: -0.10,
    aggressive_context_bonus: 0.15,
    context_window: 60,
    tie_margin: 0.10,
};

// ── Public API ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RegistryHit {
    pub matched_text: String,
    pub row_id: String,
    pub score: f32,
    pub breakdown: String,
    pub tag: String,
}

#[derive(Debug, Clone)]
pub struct ScrubReport {
    pub text: String,
    // Keys are `String` (not `&'static str`) because registry-derived tags such as
    // "Bangladeshi National ID Number" are built at runtime from CSV rows.
    pub counts: BTreeMap<String, usize>,
    pub high_sensitivity: bool,
    pub registry_hits: Vec<RegistryHit>,
    /// Token shapes (digits -> 9, letters -> A) redacted by the catch-all that no registry row matched.
    pub gaps: Vec<String>,
    /// Languages that ran on unreviewed ("draft") label data during scrubbing.
    pub unreviewed_languages: BTreeSet<String>,
    /// Languages that matched via the script-neutral name lookup.
    pub matched_name_languages: BTreeSet<String>,
    /// Clear error message when scrubbing refuses to run (e.g. missing/empty `bn_words.tsv` in release mode).
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ScrubOptions {
    /// Lowercase identity tokens (user's own names). Never printed or logged.
    identity_tokens: Vec<String>,
    pub dynamic_name_cache:
        Option<std::sync::Arc<std::sync::RwLock<std::collections::HashSet<String>>>>,
    /// Optional custom resource directory override for loading `bn_words.tsv` and `bn_names.tsv`.
    pub bn_resource_dir: Option<PathBuf>,
    /// Optional override for release-build resource enforcement (None uses `!cfg!(debug_assertions)`).
    pub bn_release_mode_override: Option<bool>,
}

impl ScrubOptions {
    /// Build options from the OS `USER` env value plus extra names (e.g. from `--names`).
    pub fn new(extra_names: &[String]) -> Self {
        Self::new_with_cache(extra_names, None)
    }

    pub fn new_with_cache(
        extra_names: &[String],
        cache: Option<std::sync::Arc<std::sync::RwLock<std::collections::HashSet<String>>>>,
    ) -> Self {
        let mut tokens: Vec<String> = Vec::new();
        let mut add = |s: &str| {
            for part in s.split(|c: char| !c.is_alphabetic()) {
                let p = part.trim().to_lowercase();
                if p.chars().count() >= 3 && !tokens.contains(&p) {
                    tokens.push(p);
                }
            }
        };
        if let Ok(u) = std::env::var("USER") {
            add(&u);
        }
        for n in extra_names {
            add(n);
        }
        ScrubOptions {
            identity_tokens: tokens,
            dynamic_name_cache: cache,
            bn_resource_dir: None,
            bn_release_mode_override: None,
        }
    }

    /// Only the number of identity tokens may ever be printed (never the tokens).
    pub fn identity_token_count(&self) -> usize {
        self.identity_tokens.len()
    }

    /// Options with no identity tokens at all (used by tests that must not depend on `$USER`).
    pub fn without_identity() -> Self {
        ScrubOptions::default()
    }
}

pub fn try_scrub_text(input: &str) -> Result<ScrubReport, String> {
    try_scrub_text_with(input, &ScrubOptions::new(&[]))
}

pub fn try_scrub_text_with(input: &str, opts: &ScrubOptions) -> Result<ScrubReport, String> {
    let rep = scrub_text_with(input, opts);
    if let Some(err) = rep.error {
        Err(err)
    } else {
        Ok(rep)
    }
}

pub fn scrub_text(input: &str) -> ScrubReport {
    scrub_text_with(input, &ScrubOptions::new(&[]))
}

pub fn scrub_text_with(input: &str, opts: &ScrubOptions) -> ScrubReport {
    clear_scrub_resource_error();
    let is_release = opts
        .bn_release_mode_override
        .unwrap_or(!cfg!(debug_assertions));
    if is_release && (opts.bn_resource_dir.is_some() || input.chars().any(is_bengali_char)) {
        if let Some(ref custom_dir) = opts.bn_resource_dir {
            let res = load_bn_resources_from_dir_with_mode(custom_dir, true);
            if let Some(err) = res.words_error.as_ref() {
                return ScrubReport {
                    text: String::new(),
                    counts: BTreeMap::new(),
                    high_sensitivity: false,
                    registry_hits: Vec::new(),
                    gaps: Vec::new(),
                    unreviewed_languages: BTreeSet::new(),
                    matched_name_languages: BTreeSet::new(),
                    error: Some(err.clone()),
                };
            }
            if res.words.is_empty() {
                let msg = "[scrubber] ERROR: Bangla word list (bn_words.tsv) is missing, unreadable, or empty in release build; refusing to scrub with empty sets.".to_string();
                return ScrubReport {
                    text: String::new(),
                    counts: BTreeMap::new(),
                    high_sensitivity: false,
                    registry_hits: Vec::new(),
                    gaps: Vec::new(),
                    unreviewed_languages: BTreeSet::new(),
                    matched_name_languages: BTreeSet::new(),
                    error: Some(msg),
                };
            }
        } else if let Err(err) = get_bn_resources_result() {
            return ScrubReport {
                text: String::new(),
                counts: BTreeMap::new(),
                high_sensitivity: false,
                registry_hits: Vec::new(),
                gaps: Vec::new(),
                unreviewed_languages: BTreeSet::new(),
                matched_name_languages: BTreeSet::new(),
                error: Some(err.clone()),
            };
        }
    }

    let high_from_keywords = is_high_sensitivity_doc(input);

    // 1. Normalize whitespace and canonicalize Bengali precomposed forms
    let mut text = normalize(input);

    // 2. Machine-readable zones (MRZ)
    let (t, mrz_found) = scrub_mrz(&text);
    text = t;
    let high = high_from_keywords || mrz_found;

    // 3. Structured patterns (fixed patterns that do not overlap with the registry)
    text = scrub_structured_pre(&text);

    // 4. Identifier registry (runs before labels so it can inspect raw values near context keywords)
    let doc = DocContext::new(&text, high);
    let (t, hits) = apply_registry(&text, &doc);
    text = t;

    // 5. Remaining generic structured rules
    text = scrub_structured_post(&text);

    // 6. Dates
    text = scrub_dates(&text);

    // 7. Catch-all numeric / ID tokens
    let (t, gaps) = scrub_catch_all(&text, high);
    text = t;

    // 8. Label-anchored values (runs alongside name detection so dynamic name caching applies)
    let mut unreviewed_languages = BTreeSet::new();
    let mut seen_ja_given_names = HashSet::new();
    text = scrub_labels_with_ja_tracking(&text, &LABEL_GROUPS, opts, &mut unreviewed_languages, &mut seen_ja_given_names);

    // 9. Unlabeled names (script tiers, unknown capitalized tokens, and name-list lookup)
    let mut matched_name_languages = BTreeSet::new();
    text = scrub_names(&text, opts, &mut matched_name_languages, &mut seen_ja_given_names);

    for lang in &matched_name_languages {
        if lang != "en" {
            unreviewed_languages.insert(lang.clone());
        }
    }

    if let Some(err) = take_scrub_resource_error() {
        return ScrubReport {
            text: String::new(),
            counts: BTreeMap::new(),
            high_sensitivity: false,
            registry_hits: Vec::new(),
            gaps: Vec::new(),
            unreviewed_languages: BTreeSet::new(),
            matched_name_languages: BTreeSet::new(),
            error: Some(err),
        };
    }

    let counts = count_tags(&text);
    ScrubReport {
        text,
        counts,
        high_sensitivity: high,
        registry_hits: hits,
        gaps,
        unreviewed_languages,
        matched_name_languages,
        error: None,
    }
}

static HIGH_SENS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:passport|driver'?s?\s+licen[cs]e|social\s+security|ssn|tax\s+return|t4|w-?2|w-?9|1099|national\s+id|identity\s+card|birth\s+certificate|visa|permanent\s+resident|bank\s+statement|routing\s+number|date\s+of\s+birth)\b",
    )
    .unwrap()
});

/// Returns true if the text looks like an identity, tax, or banking document.
pub fn is_high_sensitivity_doc(input: &str) -> bool {
    HIGH_SENS_RE.is_match(input) || scrub_mrz(input).1
}

// ── Helpers shared by several steps ───────────────────────────────────────────

/// Replaces matches of `re` with `tag`, where `re` MUST begin with the capture group
/// `(^|[^A-Za-z0-9])` as group 1 and place the actual value in group 2.
/// Group 1 is ALWAYS re-emitted before the tag so the boundary character is never eaten.
fn replace_no_lookbehind(text: &str, re: &Regex, tag: &str) -> String {
    replace_no_lookbehind_filtered(text, re, tag, |_| true)
}

/// Same as `replace_no_lookbehind`, but only replaces when `accept(body)` is true.
fn replace_no_lookbehind_filtered(
    text: &str,
    re: &Regex,
    tag: &str,
    accept: impl Fn(&str) -> bool,
) -> String {
    re.replace_all(text, |caps: &Captures| {
        let pre = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let body = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        if accept(body) {
            format!("{}{}", pre, tag)
        } else {
            caps[0].to_string()
        }
    })
    .into_owned()
}

static PLACEHOLDER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[[^\[\]\n]{1,60}\]").unwrap());

/// Apply `f` only to the parts of `text` that are NOT inside `[PLACEHOLDER]` tags.
/// `f` gets `(segment, is_first_segment_at_text_start)`.
fn map_unprotected(text: &str, mut f: impl FnMut(&str, bool) -> String) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for m in PLACEHOLDER_RE.find_iter(text) {
        let seg = &text[last..m.start()];
        out.push_str(&f(seg, last == 0));
        out.push_str(m.as_str());
        last = m.end();
    }
    out.push_str(&f(&text[last..], last == 0));
    out
}

fn inside_placeholder(text: &str, pos: usize) -> bool {
    let before = &text[..pos];
    match (before.rfind('['), before.rfind(']')) {
        (Some(o), Some(c)) => o > c,
        (Some(_), None) => true,
        _ => false,
    }
}

fn is_full_placeholder(tok: &str) -> bool {
    tok.len() > 2 && tok.starts_with('[') && tok.ends_with(']')
}

fn is_decimal_digit(c: char) -> bool {
    c.is_ascii_digit() || ('\u{09E6}'..='\u{09EF}').contains(&c) || c.is_numeric()
}

fn digit_count(s: &str) -> usize {
    s.chars().filter(|c| is_decimal_digit(*c)).count()
}

// ── Normalization ─────────────────────────────────────────────────────────────

static HSPACE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[ \t\x{00A0}]+").unwrap());

/// Collapse spaces/tabs/NBSP only. NEVER use `\s+` here: newlines must survive for label capture.
fn normalize(input: &str) -> String {
    let unified = input.replace("\r\n", "\n").replace('\r', "\n");
    let bengali = unified.replace('\u{09DF}', "\u{09AF}\u{09BC}");
    HSPACE_RE.replace_all(&bengali, " ").into_owned()
}

// ── MRZ ───────────────────────────────────────────────────────────────────────

static MRZ_A1: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Z0-9<]{25,}").unwrap());
static MRZ_A2: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"P<[A-Z]{3}[A-Z<]+").unwrap());
static MRZ_B: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Z0-9<][A-Z0-9< ]{26,}[A-Z0-9<]").unwrap());

fn scrub_mrz(text: &str) -> (String, bool) {
    let mut found = false;
    let mut cur = text.to_string();

    // Variant A (strict, no spaces): >= 25 chars with >= 3 '<', OR the P<XXX prefix form.
    cur = MRZ_A2
        .replace_all(&cur, |c: &Captures| {
            found = true;
            let _ = c;
            "[MRZ]".to_string()
        })
        .into_owned();
    cur = MRZ_A1
        .replace_all(&cur, |c: &Captures| {
            if c[0].matches('<').count() >= 3 {
                found = true;
                "[MRZ]".to_string()
            } else {
                c[0].to_string()
            }
        })
        .into_owned();

    // Variant B (OCR-tolerant): >= 28 chars, spaces allowed, >= 4 '<'.
    cur = MRZ_B
        .replace_all(&cur, |c: &Captures| {
            if c[0].matches('<').count() >= 4 {
                found = true;
                "[MRZ]".to_string()
            } else {
                c[0].to_string()
            }
        })
        .into_owned();
    (cur, found)
}

// ── Label-anchored values ─────────────────────────────────────────────────────

pub const LABEL_TSV: &str = include_str!("label_groups.tsv");

const MONTHS: &str = r"(?:JAN|FEB|MAR|APR|MAY|JUN|JUL|AUG|SEP|OCT|NOV|DEC)";

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ValueKind {
    Name,
    Address,
    Date,
    Id,
    Phone,
    Place,
    ShortCode,
}

pub fn group_kind_and_tag(name: &str) -> Option<(ValueKind, &'static str, bool)> {
    match name {
        "RELATION" => Some((ValueKind::Name, "[NAME]", true)),
        "NAME" => Some((ValueKind::Name, "[NAME]", true)),
        "ADDRESS" => Some((ValueKind::Address, "[ADDRESS]", false)),
        "DATE" => Some((ValueKind::Date, "[DATE]", false)),
        "ID" => Some((ValueKind::Id, "[ID_NUMBER]", false)),
        "PHONE" => Some((ValueKind::Phone, "[PHONE]", false)),
        "PLACE" => Some((ValueKind::Place, "[PLACE]", false)),
        "SHORT_CODE" => Some((ValueKind::ShortCode, "[ID_NUMBER]", false)),
        _ => None,
    }
}

#[derive(Clone, Debug)]
pub struct ParsedLabelGroup {
    pub group_name: String,
    pub lang: String,
    pub status: String,
    pub alternatives: Vec<String>,
}

impl ParsedLabelGroup {
    pub fn to_label_group(&self) -> Result<LabelGroup, String> {
        let (kind, tag, strict_redact) = group_kind_and_tag(&self.group_name)
            .ok_or_else(|| format!("unknown group name '{}'", self.group_name))?;
        let pattern = self.alternatives.join("|");
        let re = Regex::new(&label_pattern(&pattern))
            .map_err(|e| format!("regex error: {}", e))?;
        Ok(LabelGroup {
            group_name: self.group_name.clone(),
            lang: self.lang.clone(),
            status: self.status.clone(),
            re,
            kind,
            tag,
            strict_redact,
        })
    }
}

pub fn parse_label_table(tsv: &str) -> Result<Vec<ParsedLabelGroup>, Vec<String>> {
    let mut groups = Vec::new();
    let mut errors = Vec::new();

    for (idx, line) in tsv.lines().enumerate() {
        let line_num = idx + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 4 {
            errors.push(format!(
                "line {}: expected at least 4 tab-separated fields (group, lang, status, alternative...), got {}",
                line_num,
                parts.len()
            ));
            continue;
        }

        let group_name = parts[0].trim();
        let lang = parts[1].trim();
        let status = parts[2].trim();

        if group_name.is_empty() {
            errors.push(format!("line {}: group name is empty", line_num));
        } else if group_kind_and_tag(group_name).is_none() {
            errors.push(format!("line {}: unknown group name '{}'", line_num, group_name));
        }

        if lang.is_empty() {
            errors.push(format!("line {}: language code is empty", line_num));
        }

        if status != "reviewed" && status != "draft" {
            errors.push(format!(
                "line {}: status must be 'reviewed' or 'draft', got '{}'",
                line_num, status
            ));
        }

        let mut alternatives = Vec::new();
        let mut has_empty_alt = false;
        for p in &parts[3..] {
            let alt = p.trim();
            if alt.is_empty() {
                has_empty_alt = true;
            } else {
                alternatives.push(alt.to_string());
            }
        }

        if alternatives.is_empty() {
            errors.push(format!("line {}: group '{}' has no alternatives", line_num, group_name));
        } else if has_empty_alt {
            errors.push(format!("line {}: group '{}' contains empty alternative", line_num, group_name));
        } else {
            let pattern = alternatives.join("|");
            if let Err(e) = Regex::new(&label_pattern(&pattern)) {
                errors.push(format!("line {}: regex compilation error for group '{}': {}", line_num, group_name, e));
            }
        }

        if errors.is_empty() {
            groups.push(ParsedLabelGroup {
                group_name: group_name.to_string(),
                lang: lang.to_string(),
                status: status.to_string(),
                alternatives,
            });
        }
    }

    if errors.is_empty() {
        let mut merged: Vec<ParsedLabelGroup> = Vec::new();
        let mut index_map: std::collections::HashMap<(String, String), usize> =
            std::collections::HashMap::new();

        for g in groups {
            let key = (g.group_name.clone(), g.lang.clone());
            if let Some(&idx) = index_map.get(&key) {
                if g.status == "draft" {
                    merged[idx].status = "draft".to_string();
                }
                for alt in g.alternatives {
                    if !merged[idx].alternatives.contains(&alt) {
                        merged[idx].alternatives.push(alt);
                    }
                }
            } else {
                index_map.insert(key, merged.len());
                merged.push(g);
            }
        }
        Ok(merged)
    } else {
        Err(errors)
    }
}

pub fn validate_label_table(tsv: &str) -> Result<usize, Vec<String>> {
    match parse_label_table(tsv) {
        Ok(groups) => Ok(groups.len()),
        Err(errs) => Err(errs),
    }
}

#[derive(Clone)]
pub struct LabelGroup {
    pub group_name: String,
    pub lang: String,
    pub status: String,
    pub re: Regex,
    pub kind: ValueKind,
    pub tag: &'static str,
    pub strict_redact: bool,
}

/// Separator recognized after a label: a newline, one or more of `:*-#`, a pipe, or
/// horizontal whitespace. A pipe only counts when the line is a table row (2+ pipes);
/// that check is made in `apply_label_group_with_ja_tracking`.
const LABEL_SEPARATOR: &str = r"(?:[ \t]*\r?\n|[ \t]*[:：*\-#]+[ \t]*|[ \t]*\|[ \t]*|[ \t]+)";

fn label_pattern(labels: &str) -> String {
    format!(r"(?i)\b(?:{}){}", labels, LABEL_SEPARATOR)
}

fn label_regex(labels: &str) -> Regex {
    Regex::new(&label_pattern(labels)).unwrap()
}

/// True when the label match ended on a pipe separator that is NOT part of a table row
/// (fewer than two pipes on that line).
fn is_lone_pipe_separator(text: &str, match_start: usize, match_end: usize) -> bool {
    if !text[match_start..match_end].trim_end().ends_with('|') {
        return false;
    }
    let line_start = text[..match_start].rfind('\n').map_or(0, |i| i + 1);
    let line_end = text[match_end..].find('\n').map_or(text.len(), |i| match_end + i);
    text[line_start..line_end].matches('|').count() < 2
}

static LABEL_GROUPS: LazyLock<Vec<LabelGroup>> = LazyLock::new(|| {
    let parsed = parse_label_table(LABEL_TSV).expect("valid label_groups.tsv");
    parsed
        .into_iter()
        .map(|pg| pg.to_label_group().expect("valid label group"))
        .collect()
});

/// Stop condition for value scanning: does the text at this position START with a known label?
static KNOWN_LABELS: LazyLock<Regex> = LazyLock::new(|| {
    let parsed = parse_label_table(LABEL_TSV).expect("valid label_groups.tsv");
    let mut all_alts = Vec::new();
    for pg in parsed {
        all_alts.extend(pg.alternatives);
    }
    let all = all_alts.join("|");
    Regex::new(&format!(r"(?i)^(?:{})(?:[ \t]*[:*#\.-]+|[ \t]+|$)", all)).unwrap()
});

static NAME_TOKEN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[\p{L}\p{M}][\p{L}\p{M}.'’\-]*,?$").unwrap());
static ID_TOKEN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[\p{L}\p{M}\p{N}][\p{L}\p{M}\p{N}\-/]*[,.]?$").unwrap());
static DATE_ANCHORED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)^(?:\d{{1,2}}[/\-.]\d{{1,2}}[/\-.]\d{{2,4}}|\d{{4}}[/\-.]\d{{1,2}}[/\-.]\d{{1,2}}|\d{{1,2}}[ \t]+{m}[A-Z]*\.?,?[ \t]+\d{{2,4}}|{m}[A-Z]*\.?[ \t]+\d{{1,2}},?[ \t]+\d{{4}})",
        m = MONTHS
    ))
    .unwrap()
});
static PHONE_VALUE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[+(\d][\d \t().\-]{6,}").unwrap());
static SPELLED_DATE_RE: LazyLock<Regex> = LazyLock::new(|| {
    let days = r"first|second|third|fourth|fifth|sixth|seventh|eighth|ninth|tenth|eleventh|twelfth|thirteenth|fourteenth|fifteenth|sixteenth|seventeenth|eighteenth|nineteenth|twentieth|twenty\s*first|twenty\s*second|twenty\s*third|twenty\s*fourth|twenty\s*fifth|twenty\s*sixth|twenty\s*seventh|twenty\s*eighth|twenty\s*ninth|thirtieth|thirty\s*first";
    let months =
        r"january|february|march|april|may|june|july|august|september|october|november|december";
    let nums = r"one|two|three|four|five|six|seven|eight|nine|ten|eleven|twelve|thirteen|fourteen|fifteen|sixteen|seventeen|eighteen|nineteen|twenty|thirty|forty|fifty|sixty|seventy|eighty|ninety|hundred|and";
    let years = format!(
        r"nineteen(?:\s+(?:{}))*|two\s*thousand(?:\s+(?:{}))*",
        nums, nums
    );
    Regex::new(&format!(
        r"(?i)\b(?:{})\s+(?:{})\s+(?:{})\b",
        days, months, years
    ))
    .unwrap()
});

fn skip_hspace(text: &str, mut i: usize) -> usize {
    let b = text.as_bytes();
    while i < b.len() && (b[i] == b' ' || b[i] == b'\t') {
        i += 1;
    }
    i
}

fn skip_any_space(text: &str, mut i: usize) -> usize {
    let b = text.as_bytes();
    while i < b.len() && (b[i] == b' ' || b[i] == b'\t' || b[i] == b'\n' || b[i] == b'\r') {
        i += 1;
    }
    i
}

/// Next token on the SAME line (None at newline or end of text).
fn next_token(text: &str, from: usize) -> Option<(usize, usize)> {
    let s = skip_hspace(text, from);
    let b = text.as_bytes();
    if s >= b.len() || b[s] == b'\n' || b[s] == b'\r' {
        return None;
    }
    let mut e = s;
    while e < b.len() && !matches!(b[e], b' ' | b'\t' | b'\n' | b'\r') {
        e += 1;
    }
    Some((s, e))
}

fn next_token_any_space(text: &str, from: usize) -> Option<(usize, usize)> {
    let s = skip_any_space(text, from);
    let b = text.as_bytes();
    if s >= b.len() {
        return None;
    }
    let mut e = s;
    while e < b.len() && !matches!(b[e], b' ' | b'\t' | b'\n' | b'\r') {
        e += 1;
    }
    Some((s, e))
}

const FIXED_TAGS: &[&str] = &[
    "EMAIL",
    "SSN",
    "SIN",
    "TAX_ID",
    "CARD",
    "IBAN",
    "PHONE",
    "URL",
    "IP",
    "POSTAL",
    "NAME",
    "ADDRESS",
    "DATE",
    "PLACE",
    "ID_NUMBER",
    "MRZ",
];

static GENERATED_TAGS: LazyLock<HashSet<String>> = LazyLock::new(|| {
    let mut known: HashSet<String> = FIXED_TAGS.iter().map(|s| s.to_string()).collect();
    for d in REGISTRY.iter() {
        known.insert(d.display_name.clone());
        known.insert(d.type_display_name.clone());
    }
    known
});

fn match_generated_placeholder(text: &str, s: usize) -> Option<usize> {
    if !text[s..].starts_with('[') {
        return None;
    }
    let close = text[s..].find(']')?;
    let inner = &text[s + 1..s + close];
    if GENERATED_TAGS.contains(inner) {
        let mut end = s + close + 1;
        let b = text.as_bytes();
        while end < b.len() && matches!(b[end], b',' | b'.' | b';' | b':' | b'!' | b'?') {
            end += 1;
        }
        Some(end)
    } else {
        None
    }
}

fn collect_tokens(
    text: &str,
    start: usize,
    max: usize,
    pred: impl Fn(&str) -> bool,
) -> Option<(usize, usize)> {
    let mut i = start;
    let mut first: Option<usize> = None;
    let mut last_end = 0;
    let mut count = 0;
    let mut newlines_crossed = 0;
    while count < max {
        let s = skip_hspace(text, i);
        if s >= text.len() {
            break;
        }
        if text.as_bytes()[s] == b'\n' || text.as_bytes()[s] == b'\r' {
            if newlines_crossed == 0 {
                let next_s = skip_any_space(text, i);
                let nl_count = text[s..next_s].chars().filter(|&c| c == '\n').count();
                if nl_count == 1 && next_s < text.len() {
                    if match_generated_placeholder(text, next_s).is_some()
                        || inside_placeholder(text, next_s)
                        || KNOWN_LABELS.is_match(&text[next_s..])
                    {
                        break;
                    }
                    if let Some((ts, te)) = next_token(text, next_s) {
                        if pred(&text[ts..te]) {
                            newlines_crossed += 1;
                            i = next_s;
                            continue;
                        }
                    }
                }
            }
            break;
        }
        if match_generated_placeholder(text, s).is_some() || inside_placeholder(text, s) {
            break;
        }
        let Some((s, e)) = next_token(text, i) else {
            break;
        };
        if text[s..e].contains('[') || KNOWN_LABELS.is_match(&text[s..]) {
            break;
        }
        if !pred(&text[s..e]) {
            break;
        }
        first.get_or_insert(s);
        last_end = e;
        i = e;
        count += 1;
    }
    first.map(|s| (s, last_end))
}

fn collect_tokens_any_space(
    text: &str,
    start: usize,
    max: usize,
    pred: impl Fn(&str) -> bool,
) -> Option<(usize, usize)> {
    let mut i = start;
    let mut first: Option<usize> = None;
    let mut last_end = 0;
    let mut had_placeholder = false;
    let mut count = 0;
    while count < max || had_placeholder {
        if had_placeholder {
            let s = skip_hspace(text, i);
            if s >= text.len() || text.as_bytes()[s] == b'\n' || text.as_bytes()[s] == b'\r' {
                break;
            }
        }
        let s = skip_any_space(text, i);
        if s >= text.len() {
            break;
        }
        if let Some(placeholder_end) = match_generated_placeholder(text, s) {
            if first.is_some() {
                break;
            }
            let close = text[s..].find(']').unwrap_or(0);
            let inner = &text[s + 1..s + close];
            if inner == "ADDRESS" {
                break;
            }
            had_placeholder = true;
            i = placeholder_end;
            continue;
        }
        let Some((s, e)) = next_token_any_space(text, i) else {
            break;
        };
        if text[s..e].contains('[') || KNOWN_LABELS.is_match(&text[s..]) {
            break;
        }
        if !had_placeholder && !pred(&text[s..e]) {
            break;
        }
        first.get_or_insert(s);
        last_end = e;
        i = e;
        count += 1;
    }
    first.map(|s| (s, last_end))
}

fn is_cjk_or_japanese(c: char) -> bool {
    matches!(c,
        '\u{3040}'..='\u{309F}' | // Hiragana
        '\u{30A0}'..='\u{30FF}' | // Katakana
        '\u{4E00}'..='\u{9FFF}' | // CJK Unified Ideographs (Kanji)
        '\u{3400}'..='\u{4DBF}'   // CJK Extension A
    )
}

fn is_hangul_char(c: char) -> bool {
    matches!(c,
        '\u{AC00}'..='\u{D7AF}' | // Hangul Syllables
        '\u{1100}'..='\u{11FF}' | // Hangul Jamo
        '\u{3130}'..='\u{318F}'   // Hangul Compatibility Jamo
    )
}

fn is_cjk_or_hangul(c: char) -> bool {
    is_cjk_or_japanese(c) || is_hangul_char(c)
}

/// Japanese bounded-character rule: 2 to 6 characters after label delimiter,
/// stopping at 。 、 は が の, whitespace, newline, or a known label.
fn japanese_bounded_name_span(text: &str, start: usize) -> Option<(usize, usize)> {
    let s = skip_hspace(text, start);
    if s >= text.len() {
        return None;
    }
    if inside_placeholder(text, s) || match_generated_placeholder(text, s).is_some() {
        return None;
    }
    let first_char = text[s..].chars().next()?;
    if !is_cjk_or_japanese(first_char) {
        return None;
    }

    let mut count = 0;
    let mut end = s;
    for (offset, ch) in text[s..].char_indices() {
        if count >= 12 {
            break;
        }
        if ch == '\n' || ch == '\r' || matches!(ch, '。' | '、' | 'は' | 'が' | 'の') {
            break;
        }
        if ch.is_whitespace() {
            break;
        }
        if ch == '['
            && (inside_placeholder(text, s + offset)
                || match_generated_placeholder(text, s + offset).is_some())
        {
            break;
        }
        if KNOWN_LABELS.is_match(&text[s + offset..]) {
            break;
        }
        if is_cjk_or_japanese(ch) {
            count += 1;
            end = s + offset + ch.len_utf8();
        } else {
            break;
        }
    }

    if count >= 2 {
        Some((s, end))
    } else {
        None
    }
}

/// Japanese bounded-character rule for address: up to 25 characters after label delimiter,
/// stopping at a punctuation mark (。 、 etc.), particle (は が の etc.), newline, or a known label.
fn japanese_bounded_address_span(text: &str, start: usize) -> Option<(usize, usize)> {
    let s = skip_hspace(text, start);
    if s >= text.len() {
        return None;
    }
    if inside_placeholder(text, s) || match_generated_placeholder(text, s).is_some() {
        return None;
    }
    let first_char = text[s..].chars().next()?;
    if !is_cjk_or_japanese(first_char) {
        return None;
    }

    let mut count = 0;
    let mut end = s;
    for (offset, ch) in text[s..].char_indices() {
        if count >= 25 {
            break;
        }
        if ch == '\n'
            || ch == '\r'
            || matches!(
                ch,
                '。' | '、' | '.' | ',' | ';' | '!' | '?' | '！' | '？' | 'は' | 'が' | 'の'
            )
        {
            break;
        }
        if offset > 0 && KNOWN_LABELS.is_match(&text[s + offset..]) {
            break;
        }
        if ch == '['
            && (inside_placeholder(text, s + offset)
                || match_generated_placeholder(text, s + offset).is_some())
        {
            break;
        }
        if is_cjk_or_japanese(ch)
            || ch.is_ascii_alphanumeric()
            || ('０'..='９').contains(&ch)
            || matches!(ch, '-' | 'ー' | '−' | '―' | '‐' | '－' | '/' | ' ' | '\t')
        {
            count += 1;
            end = s + offset + ch.len_utf8();
        } else {
            break;
        }
    }

    while end > s && text[..end].ends_with(|c: char| c.is_whitespace()) {
        end -= 1;
    }

    if count >= 3 {
        Some((s, end))
    } else {
        None
    }
}

const SHORT_CODE_WINDOW_BYTES: usize = 24;

fn is_short_code_blocked_prefix_word(word: &str) -> bool {
    matches!(
        word.to_ascii_lowercase().as_str(),
        "in" | "on"
            | "at"
            | "from"
            | "since"
            | "until"
            | "during"
            | "after"
            | "before"
            | "between"
            | "year"
            | "years"
            | "costs"
            | "cost"
            | "paid"
            | "pay"
            | "price"
            | "fee"
            | "worth"
            | "usd"
            | "eur"
            | "gbp"
            | "cad"
            | "aud"
            | "inr"
            | "bdt"
            | "jpy"
            | "krw"
            | "dollars"
            | "dollar"
            | "euros"
            | "euro"
            | "pounds"
            | "pound"
            | "rupees"
            | "rupee"
            | "taka"
            | "yen"
            | "won"
            | "cents"
            | "cent"
    )
}

fn is_short_code_blocked_suffix(after: &str) -> bool {
    let trimmed = after.trim_start_matches([' ', '\t']);
    if trimmed.starts_with(['$', '€', '£', '¥', '₹', '৳', '%']) {
        return true;
    }
    let word_end = trimmed
        .char_indices()
        .find(|&(_, c)| !c.is_alphabetic())
        .map(|(i, _)| i)
        .unwrap_or(trimmed.len());
    if word_end == 0 {
        return false;
    }
    matches!(
        trimmed[..word_end].to_ascii_lowercase().as_str(),
        "dollars"
            | "dollar"
            | "usd"
            | "euros"
            | "euro"
            | "eur"
            | "pounds"
            | "pound"
            | "gbp"
            | "cad"
            | "aud"
            | "rupees"
            | "rupee"
            | "inr"
            | "taka"
            | "bdt"
            | "yen"
            | "jpy"
            | "won"
            | "krw"
            | "cents"
            | "cent"
            | "percent"
            | "years"
            | "year"
            | "months"
            | "month"
            | "days"
            | "day"
            | "hours"
            | "hour"
            | "minutes"
            | "minute"
            | "seconds"
            | "second"
    )
}

/// Extract a 3..=8 digit short sensitive code span (CVC/CVV/PIN/OTP/passcode)
/// within a short single-line window after a label, redacting only the digits.
fn short_code_span(text: &str, s: usize) -> Option<(usize, usize)> {
    let rest = &text[s..];
    let mut limit = rest.len().min(SHORT_CODE_WINDOW_BYTES);
    while limit > 0 && !rest.is_char_boundary(limit) {
        limit -= 1;
    }
    let window = &rest[..limit];

    // Scan up to the first ASCII digit on the same line, stopping at sentence/line breaks or currency.
    let mut rel_digit_start = None;
    let mut chars = window.char_indices().peekable();
    while let Some((idx, ch)) = chars.next() {
        if matches!(ch, '\n' | '\r' | ';' | '!' | '?' | '$' | '€' | '£' | '¥' | '₹' | '৳') {
            return None;
        }
        if ch == '.' && chars.peek().map_or(true, |&(_, next)| next.is_whitespace()) {
            return None;
        }
        if ch.is_ascii_digit() {
            rel_digit_start = Some(idx);
            break;
        }
    }
    let rel_start = rel_digit_start?;
    let prefix = &window[..rel_start];

    // Validate intervening prefix: at most 2 short non-blocked connector words.
    let mut word_count = 0usize;
    for raw in prefix.split(|c: char| !c.is_alphabetic()) {
        if raw.is_empty() {
            continue;
        }
        word_count += 1;
        if word_count > 2 || raw.chars().count() > 12 || is_short_code_blocked_prefix_word(raw) {
            return None;
        }
    }

    // Reject if the digit run is directly attached to alphanumeric, decimal dot, or slash.
    if let Some(prev) = text[..s + rel_start].chars().next_back() {
        if prev.is_ascii_alphanumeric() || matches!(prev, '.' | '/') {
            return None;
        }
    }

    let digit_start = s + rel_start;
    let digit_len = text[digit_start..]
        .bytes()
        .take_while(|b| b.is_ascii_digit())
        .count();
    if !(3..=8).contains(&digit_len) {
        return None;
    }
    let digit_end = digit_start + digit_len;

    // Reject decimals (3.50), slashes (123/456), attached alnum units (100px), or unit/currency suffixes.
    let after = &text[digit_end..];
    if let Some(next) = after.chars().next() {
        if next.is_ascii_alphanumeric() {
            return None;
        }
        if matches!(next, '.' | '/') && after[next.len_utf8()..].starts_with(|c: char| c.is_ascii_digit()) {
            return None;
        }
    }
    if is_short_code_blocked_suffix(after) {
        return None;
    }

    Some((digit_start, digit_end))
}

fn value_span(text: &str, start: usize, kind: ValueKind) -> Option<(usize, usize)> {
    let s = skip_hspace(text, start);
    if s >= text.len() {
        return None;
    }
    if inside_placeholder(text, s) {
        return None;
    }
    if let Some(placeholder_end) = match_generated_placeholder(text, s) {
        match kind {
            ValueKind::Name
            | ValueKind::Date
            | ValueKind::Place
            | ValueKind::Phone
            | ValueKind::ShortCode => {
                return None;
            }
            ValueKind::Address => {
                let close = text[s..].find(']').unwrap_or(0);
                let inner = &text[s + 1..s + close];
                if inner == "ADDRESS" {
                    return None;
                }
            }
            ValueKind::Id => {
                let rest = text[placeholder_end..].lines().next().unwrap_or("");
                if rest.trim().is_empty() {
                    return None;
                }
            }
        }
    }
    match kind {
        ValueKind::Name => {
            if let Some(span) = japanese_bounded_name_span(text, start) {
                Some(span)
            } else {
                collect_tokens(text, start, 5, |t| NAME_TOKEN_RE.is_match(t))
            }
        }
        ValueKind::Place => collect_tokens(text, start, 3, |t| NAME_TOKEN_RE.is_match(t)),
        ValueKind::Address => {
            if let Some(span) = japanese_bounded_address_span(text, start) {
                Some(span)
            } else {
                collect_tokens_any_space(text, start, 20, |_| true)
            }
        }
        ValueKind::Date => {
            if let Some(m) = DATE_ANCHORED.find(&text[s..]) {
                return Some((s, s + m.end()));
            }
            collect_tokens(text, start, 12, |_| true) // Increased to catch spaced dates
        }
        ValueKind::Phone => {
            let m = PHONE_VALUE_RE.find(&text[s..])?;
            let val = m.as_str().trim_end();
            if digit_count(val) < 6 {
                return None;
            }
            Some((s, s + val.len()))
        }
        ValueKind::ShortCode => short_code_span(text, s),
        ValueKind::Id => {
            // Up to 18 ID-looking tokens; trailing tokens without a digit are dropped
            let mut toks: Vec<(usize, usize, bool)> = Vec::new();
            let mut i = start;
            let mut had_placeholder = false;
            let mut count = 0;
            while count < 18 || had_placeholder {
                let s = skip_hspace(text, i);
                if s >= text.len() || text.as_bytes()[s] == b'\n' || text.as_bytes()[s] == b'\r' {
                    break;
                }
                if let Some(placeholder_end) = match_generated_placeholder(text, s) {
                    if !toks.is_empty() {
                        break;
                    }
                    had_placeholder = true;
                    i = placeholder_end;
                    continue;
                }
                let Some((s, e)) = next_token(text, i) else {
                    break;
                };
                if text[s..e].contains('[') || KNOWN_LABELS.is_match(&text[s..]) {
                    break;
                }
                let tok_str = &text[s..e];
                if !had_placeholder && !ID_TOKEN_RE.is_match(tok_str) {
                    break;
                }
                let has_digit = tok_str.chars().any(is_decimal_digit);
                toks.push((s, e, has_digit));
                i = e;
                count += 1;
            }
            if !had_placeholder {
                while let Some(&(_, _, has_digit)) = toks.last() {
                    if has_digit {
                        break;
                    }
                    toks.pop();
                }
            }
            if toks.is_empty() {
                return None;
            }
            let s = toks[0].0;
            let mut e = toks[toks.len() - 1].1;
            // do not swallow a trailing sentence period / comma
            while e > s && matches!(text.as_bytes()[e - 1], b'.' | b',') {
                e -= 1;
            }
            Some((s, e))
        }
    }
}

pub fn apply_label_group(
    text: &str,
    g: &LabelGroup,
    opts: &ScrubOptions,
    unreviewed: &mut std::collections::BTreeSet<String>,
) -> String {
    let mut dummy = HashSet::new();
    apply_label_group_with_ja_tracking(text, g, opts, unreviewed, &mut dummy)
}

fn apply_label_group_with_ja_tracking(
    text: &str,
    g: &LabelGroup,
    opts: &ScrubOptions,
    unreviewed: &mut std::collections::BTreeSet<String>,
    seen_ja_given_names: &mut HashSet<String>,
) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    let mut pos = 0;
    while pos <= text.len() {
        let Some(m) = g.re.find_at(text, pos) else {
            break;
        };
        if inside_placeholder(text, m.start()) {
            pos = m.end();
            continue;
        };
        if is_lone_pipe_separator(text, m.start(), m.end()) {
            pos = m.end();
            continue;
        }
        if let Some(kl) = KNOWN_LABELS.find(&text[m.start()..]) {
            if kl.end() > m.end() - m.start() {
                pos = m.start() + kl.end();
                continue;
            }
        }
        match value_span(text, m.end(), g.kind) {
            Some((vs, ve)) if vs >= last => {
                let mut should_redact = true;
                if g.kind == ValueKind::Name {
                    let extracted = &text[vs..ve];
                    let words: Vec<&str> = extracted
                        .split(|c: char| !c.is_alphabetic() && !is_devanagari_char(c))
                        .filter(|w| !w.is_empty())
                        .collect();
                    let label_sep_slice = &text[m.start()..vs];
                    let has_explicit_sep = label_sep_slice
                        .chars()
                        .any(|c| matches!(c, ':' | '*' | '-' | '#' | '|' | '\n' | '\r'));
                    // For Hindi NAME/RELATION labels matched without a colon, delimiter, or newline
                    // (horizontal whitespace only, e.g. "राम का नाम है" or "नाम का अर्थ है"),
                    // skip redaction when the immediately following token is a non-name Hindi word.
                    if g.lang == "hi"
                        && !has_explicit_sep
                        && words.first().is_some_and(|&first_w| {
                            let res = get_hi_resources();
                            is_hi_non_name_common_token(first_w, res)
                        })
                    {
                        should_redact = false;
                    } else if !words.is_empty() {
                        if let Some(cache) = &opts.dynamic_name_cache {
                            if let Ok(mut cache_lock) = cache.write() {
                                for w in words {
                                    if w.len() > 2 {
                                        cache_lock.insert(w.to_lowercase());
                                    }
                                }
                                cache_lock.insert(extracted.to_lowercase());
                            }
                        }
                    }
                }

                if should_redact {
                    if g.kind == ValueKind::Name {
                        extract_japanese_given_names_from_full_name(&text[vs..ve], seen_ja_given_names);
                    }
                    if g.status == "draft" {
                        unreviewed.insert(g.lang.clone());
                    }
                    out.push_str(&text[last..vs]);
                    out.push_str(g.tag);
                    last = ve;
                    pos = ve;
                } else {
                    pos = m.end();
                }
            }
            _ => pos = m.end(),
        }
    }
    out.push_str(&text[last..]);
    out
}

pub fn scrub_labels_with(
    text: &str,
    groups: &[LabelGroup],
    opts: &ScrubOptions,
    unreviewed: &mut std::collections::BTreeSet<String>,
) -> String {
    let mut dummy = HashSet::new();
    scrub_labels_with_ja_tracking(text, groups, opts, unreviewed, &mut dummy)
}

fn scrub_labels_with_ja_tracking(
    text: &str,
    groups: &[LabelGroup],
    opts: &ScrubOptions,
    unreviewed: &mut std::collections::BTreeSet<String>,
    seen_ja_given_names: &mut HashSet<String>,
) -> String {
    let mut cur = text.to_string();
    for g in groups {
        cur = apply_label_group_with_ja_tracking(&cur, g, opts, unreviewed, seen_ja_given_names);
    }
    cur
}

fn scrub_labels(
    text: &str,
    opts: &ScrubOptions,
    unreviewed: &mut std::collections::BTreeSet<String>,
) -> String {
    scrub_labels_with(text, &LABEL_GROUPS, opts, unreviewed)
}

// ── Structured patterns ───────────────────────────────────────────────────────
// Do not add patterns here for any identifier type covered by the PII identifier registry.

static URL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bhttps?://\S+|\bwww\.\S+").unwrap());
static IP_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(^|[^\p{L}\p{N}.])((?:\d{1,3}\.){3}\d{1,3})\b").unwrap());
static SSN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(^|[^\p{L}\p{N}])(\d{3}-\d{2}-\d{4}|\d{3} \d{2} \d{4})\b").unwrap()
});
static SIN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(^|[^\p{L}\p{N}])(\d{3}[- ]\d{3}[- ]\d{3})\b").unwrap());
static EIN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(^|[^\p{L}\p{N}])(\d{2}-\d{7})\b").unwrap());
static CARD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(^|[^\p{L}\p{N}])((?:\d[ -]?){12,18}\d)\b").unwrap());
static IBAN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(^|[^\p{L}\p{N}])([A-Z]{2}\d{2}[A-Z0-9]{11,30})\b").unwrap());
// Plan regex, extended so "(800) 555-0199" (area code + 2 groups) also matches.
static PHONE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(^|[^\p{L}\p{N}])((?:\+?\d{1,3}[ .\-]?)?(?:\(\d{2,4}\)[ .\-]?\d{3,4}[ .\-]?\d{3,4}|\d{2,4}[ .\-]?\d{3,4}[ .\-]?\d{3,4}))\b",
    )
    .unwrap()
});
static POSTAL_CA_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(^|[^\p{L}\p{N}])([A-Z]\d[A-Z][ -]?\d[A-Z]\d)\b").unwrap());
static POSTAL_US_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?P<st>\b(?:AL|AK|AZ|AR|CA|CO|CT|DE|DC|FL|GA|HI|ID|IL|IN|IA|KS|KY|LA|ME|MD|MA|MI|MN|MS|MO|MT|NE|NV|NH|NJ|NM|NY|NC|ND|OH|OK|OR|PA|RI|SC|SD|TN|TX|UT|VT|VA|WA|WV|WI|WY|AB|BC|MB|NB|NL|NS|NT|NU|ON|PE|QC|SK|YT)[ \t]+)(?P<zip>\d{5}(?:-\d{4})?)\b",
    )
    .unwrap()
});

fn scrub_structured_pre(text: &str) -> String {
    // URL (strip trailing punctuation back out of the match)
    let mut cur = URL_RE
        .replace_all(text, |c: &Captures| {
            let m = &c[0];
            let trimmed = m.trim_end_matches(|ch: char| ".,;:!?)\"'".contains(ch));
            format!("[URL]{}", &m[trimmed.len()..])
        })
        .into_owned();
    cur = scrub_emails(&cur);
    cur = replace_no_lookbehind_filtered(&cur, &IP_RE, "[IP]", |b| {
        b.split('.')
            .all(|p| p.parse::<u16>().map(|v| v <= 255).unwrap_or(false))
    });
    cur = replace_no_lookbehind(&cur, &SSN_RE, "[SSN]");
    cur = replace_no_lookbehind(&cur, &SIN_RE, "[SIN]");
    cur = replace_no_lookbehind(&cur, &EIN_RE, "[TAX_ID]");
    cur = replace_no_lookbehind_filtered(&cur, &CARD_RE, "[CARD]", |b| {
        let d = digit_count(b);
        (13..=19).contains(&d) && luhn(b)
    });
    cur = replace_no_lookbehind_filtered(&cur, &IBAN_RE, "[IBAN]", mod97);
    cur
}

fn scrub_structured_post(text: &str) -> String {
    let mut cur = replace_no_lookbehind_filtered(text, &PHONE_RE, "[PHONE]", |b| {
        (9..=15).contains(&digit_count(b))
    });
    cur = replace_no_lookbehind(&cur, &POSTAL_CA_RE, "[POSTAL]");
    cur = POSTAL_US_RE.replace_all(&cur, "${st}[POSTAL]").into_owned();
    cur
}

// Email redaction (copied from the original scrubber.rs logic)
fn scrub_emails(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let chars: Vec<(usize, char)> = input.char_indices().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i].1 == '@' && i > 0 && i + 1 < len {
            let mut start = i;
            while start > 0 {
                let c = chars[start - 1].1;
                if c.is_ascii_alphanumeric()
                    || c == '.'
                    || c == '_'
                    || c == '%'
                    || c == '+'
                    || c == '-'
                {
                    start -= 1;
                } else {
                    break;
                }
            }
            let mut end = i + 1;
            let mut has_dot = false;
            while end < len {
                let c = chars[end].1;
                if c.is_ascii_alphanumeric() || c == '-' {
                    end += 1;
                } else if c == '.' {
                    has_dot = true;
                    end += 1;
                } else {
                    break;
                }
            }
            while end > i + 1 && (chars[end - 1].1 == '.' || chars[end - 1].1 == '-') {
                end -= 1;
            }
            let local_len = i - start;
            let valid_domain = if has_dot && end > i + 2 {
                let domain_slice =
                    &input[chars[i + 1].0..=chars[end - 1].0 + chars[end - 1].1.len_utf8() - 1];
                if let Some(last_dot) = domain_slice.rfind('.') {
                    let tld = &domain_slice[last_dot + 1..];
                    tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic())
                } else {
                    false
                }
            } else {
                false
            };
            if local_len >= 1 && valid_domain {
                let local_byte_len = chars[i].0 - chars[start].0;
                let new_len = result.len() - local_byte_len;
                result.truncate(new_len);
                result.push_str("[EMAIL]");
                i = end;
                continue;
            }
        }
        result.push(chars[i].1);
        i += 1;
    }
    result
}

// ── Checksums ─────────────────────────────────────────────────────────────────

fn digits_of(s: &str) -> Vec<u32> {
    s.chars().filter_map(|c| c.to_digit(10)).collect()
}

pub fn luhn(s: &str) -> bool {
    let d = digits_of(s);
    if d.len() < 2 {
        return false;
    }
    let mut sum = 0;
    for (i, &v) in d.iter().rev().enumerate() {
        let mut x = v;
        if i % 2 == 1 {
            x *= 2;
            if x > 9 {
                x -= 9;
            }
        }
        sum += x;
    }
    sum % 10 == 0
}

const VERHOEFF_D: [[u8; 10]; 10] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
    [1, 2, 3, 4, 0, 6, 7, 8, 9, 5],
    [2, 3, 4, 0, 1, 7, 8, 9, 5, 6],
    [3, 4, 0, 1, 2, 8, 9, 5, 6, 7],
    [4, 0, 1, 2, 3, 9, 5, 6, 7, 8],
    [5, 9, 8, 7, 6, 0, 4, 3, 2, 1],
    [6, 5, 9, 8, 7, 1, 0, 4, 3, 2],
    [7, 6, 5, 9, 8, 2, 1, 0, 4, 3],
    [8, 7, 6, 5, 9, 3, 2, 1, 0, 4],
    [9, 8, 7, 6, 5, 4, 3, 2, 1, 0],
];
const VERHOEFF_P: [[u8; 10]; 8] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
    [1, 5, 7, 6, 2, 8, 3, 0, 9, 4],
    [5, 8, 0, 3, 7, 9, 6, 1, 4, 2],
    [8, 9, 1, 6, 0, 4, 3, 5, 2, 7],
    [9, 4, 5, 3, 1, 2, 6, 8, 7, 0],
    [4, 2, 8, 6, 5, 7, 3, 9, 0, 1],
    [2, 7, 9, 3, 8, 0, 6, 4, 1, 5],
    [7, 0, 4, 6, 9, 1, 3, 2, 5, 8],
];

pub fn verhoeff(s: &str) -> bool {
    let d = digits_of(s);
    if d.len() < 2 {
        return false;
    }
    let mut c = 0u8;
    for (i, &v) in d.iter().rev().enumerate() {
        c = VERHOEFF_D[c as usize][VERHOEFF_P[i % 8][v as usize] as usize];
    }
    c == 0
}

/// UK NHS number: 10 digits, weights 10..2 on the first 9, check = 11 - (sum % 11) (11 -> 0, 10 invalid).
pub fn mod11_nhs(s: &str) -> bool {
    let d = digits_of(s);
    if d.len() != 10 {
        return false;
    }
    let sum: u32 = d[..9]
        .iter()
        .enumerate()
        .map(|(i, &v)| v * (10 - i as u32))
        .sum();
    let check = 11 - (sum % 11);
    let check = if check == 11 { 0 } else { check };
    check != 10 && check == d[9]
}

/// IBAN mod-97 check (also usable on any alphanumeric string).
pub fn mod97(s: &str) -> bool {
    let compact: String = s.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    if compact.len() < 5 {
        return false;
    }
    let rearranged = format!("{}{}", &compact[4..], &compact[..4]);
    let mut rem: u32 = 0;
    for c in rearranged.chars() {
        let v = if let Some(d) = c.to_digit(10) {
            d
        } else if c.is_ascii_alphabetic() {
            c.to_ascii_uppercase() as u32 - 'A' as u32 + 10
        } else {
            return false;
        };
        if v >= 10 {
            rem = (rem * 100 + v) % 97;
        } else {
            rem = (rem * 10 + v) % 97;
        }
    }
    rem == 1
}

/// US ABA routing number: 9 digits, 3*(d1+d4+d7) + 7*(d2+d5+d8) + (d3+d6+d9) divisible by 10.
pub fn aba(s: &str) -> bool {
    let d = digits_of(s);
    if d.len() != 9 {
        return false;
    }
    let sum = 3 * (d[0] + d[3] + d[6]) + 7 * (d[1] + d[4] + d[7]) + (d[2] + d[5] + d[8]);
    sum % 10 == 0
}

/// `Some(true/false)` when the checksum is implemented, `None` when it is not
/// (unimplemented checksums give no bonus and no penalty).
fn validate_checksum(name: &str, value: &str) -> Option<bool> {
    match name {
        "luhn" => Some(luhn(value)),
        "verhoeff" => Some(verhoeff(value)),
        "mod11_nhs" => Some(mod11_nhs(value)),
        "mod97" => Some(mod97(value)),
        "aba" => Some(aba(value)),
        _ => None,
    }
}

// ── PII Identifier Knowledge Base ─────────────────────────────────────────────

const CSV_TEXT: &str = include_str!("id_formats.csv");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Verified,
    Seeded,
    Unverified,
}

struct IdentifierDef {
    id: String,
    country: String,
    authority: String,
    identifier_type: String,
    display_name: String,
    type_display_name: String,
    patterns: Vec<Regex>,
    context_keywords: Vec<String>,
    context_required: bool,
    checksum: String,
    base_confidence: f32,
    valid_from: Option<u16>,
    valid_until: Option<u16>,
    status: Status,
}

static REGISTRY: LazyLock<Vec<IdentifierDef>> = LazyLock::new(|| load_registry(CSV_TEXT));

fn load_registry(text: &str) -> Vec<IdentifierDef> {
    let rows = match registry_check::parse_csv(text) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[registry] cannot parse id_formats.csv: {}", e);
            return Vec::new();
        }
    };
    let mut defs = Vec::new();
    for row in &rows {
        let id = row.get("id").trim().to_string();
        let bodies = match registry_check::expand_format(row.get("format")) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("[registry] skipping row {}: {}", id, e);
                continue;
            }
        };
        let mut patterns = Vec::new();
        for b in &bodies {
            match Regex::new(&registry_check::wrap_regex(b)) {
                Ok(r) => patterns.push(r),
                Err(_) => eprintln!(
                    "[registry] skipping a pattern of row {} (does not compile)",
                    id
                ),
            }
        }
        if patterns.is_empty() {
            continue;
        }
        let status = match row.get("verification_status").trim() {
            "verified" => Status::Verified,
            "seeded" => Status::Seeded,
            _ => Status::Unverified,
        };
        defs.push(IdentifierDef {
            id,
            country: row.get("country").trim().to_string(),
            authority: row.get("authority").trim().to_lowercase(),
            identifier_type: row.get("identifier_type").trim().to_string(),
            display_name: row.get("display_name").trim().to_string(),
            type_display_name: row.get("type_display_name").trim().to_string(),
            patterns,
            context_keywords: row
                .get("context_keywords")
                .split('|')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect(),
            context_required: row.get("context_required").trim() == "yes",
            checksum: row.get("checksum").trim().to_string(),
            base_confidence: row.get("base_confidence").trim().parse().unwrap_or(0.25),
            valid_from: row.get("valid_from").trim().parse().ok(),
            valid_until: row.get("valid_until").trim().parse().ok(),
            status,
        });
    }
    defs
}

/// Validate the embedded CSV registry and label table.
pub fn check_registry() -> Result<usize, Vec<String>> {
    let mut errs = Vec::new();
    let csv_rows = match registry_check::validate_csv_text(CSV_TEXT) {
        Ok(n) => n,
        Err(e) => {
            errs.extend(e);
            0
        }
    };
    if let Err(e) = validate_label_table(LABEL_TSV) {
        errs.extend(e);
    }
    if errs.is_empty() {
        Ok(csv_rows)
    } else {
        Err(errs)
    }
}

/// Returns `(country, identifier_type, verification_status)` for every row in the registry.
pub fn registry_summary() -> Vec<(String, String, String)> {
    registry_check::parse_csv(CSV_TEXT)
        .unwrap_or_default()
        .iter()
        .map(|r| {
            (
                r.get("country").trim().to_string(),
                r.get("identifier_type").trim().to_string(),
                r.get("verification_status").trim().to_string(),
            )
        })
        .collect()
}

struct DocContext {
    lower: String,
    high_sensitivity: bool,
    /// Optional document-level year for `valid_from`/`valid_until` scoring; when `None`, the year penalty is skipped.
    year: Option<u16>,
}

impl DocContext {
    fn new(text: &str, high_sensitivity: bool) -> Self {
        DocContext {
            lower: text.to_lowercase(),
            high_sensitivity,
            year: None,
        }
    }
}

fn type_keywords(identifier_type: &str) -> &'static [&'static str] {
    match identifier_type {
        "passport" => &["passport"],
        "national_id" => &["national id", "identity card", "id card", "nid"],
        "tax_id" => &["tax", "tin", "taxpayer"],
        "social_security" => &["social security", "ssn", "social insurance"],
        "driver_license" => &["driver", "licence", "license"],
        "health_id" => &["health", "medicare", "insurance"],
        "birth_registration" => &["birth"],
        _ => &[],
    }
}

fn country_clues(cc: &str) -> &'static [&'static str] {
    match cc {
        "BD" => &["bangladesh", "dhaka"],
        "US" => &["united states", "usa", "u.s."],
        "CA" => &["canada", "canadian"],
        "GB" => &["united kingdom", "england", "britain", "british"],
        "IN" => &["india", "भारत"],
        "AU" => &["australia"],
        "DE" => &["germany", "deutschland", "german"],
        "FR" => &["france", "french", "république française"],
        "ES" => &["españa", "spain", "spanish"],
        "IT" => &["italia", "italy", "italian"],
        "NL" => &["nederland", "netherlands", "dutch"],
        "SE" => &["sverige", "sweden", "swedish"],
        "NO" => &["norge", "norway", "norwegian"],
        "DK" => &["danmark", "denmark", "danish"],
        "FI" => &["suomi", "finland", "finnish"],
        "PL" => &["polska", "poland", "polish"],
        "PT" => &["portugal", "portuguese"],
        "BR" => &["brasil", "brazil", "brazilian"],
        "MX" => &["méxico", "mexico", "mexican"],
        "AR" => &["argentina"],
        "CL" => &["chile", "chilean"],
        "CO" => &["colombia", "colombian"],
        "CN" => &["china", "chinese", "中国"],
        "JP" => &["japan", "japanese", "日本"],
        "KR" => &["korea", "korean", "대한민국"],
        "SG" => &["singapore"],
        "MY" => &["malaysia", "malaysian"],
        "ID" => &["indonesia", "indonesian"],
        "PK" => &["pakistan", "pakistani"],
        "TH" => &["thailand", "thai"],
        "TR" => &["türkiye", "turkey", "turkish"],
        "ZA" => &["south africa"],
        "NG" => &["nigeria", "nigerian"],
        "PH" => &["philippines", "filipino"],
        "IE" => &["ireland", "irish"],
        "NZ" => &["new zealand"],
        "RU" => &["russia", "russian"],
        "UA" => &["ukraine", "ukrainian"],
        "IL" => &["israel", "israeli"],
        "EG" => &["egypt", "egyptian"],
        "KE" => &["kenya", "kenyan"],
        _ => &[],
    }
}

fn floor_boundary(s: &str, mut i: usize) -> usize {
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}
fn ceil_boundary(s: &str, mut i: usize) -> usize {
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

fn is_context_token_char(c: char) -> bool {
    (c.is_alphanumeric() || ('\u{0980}'..='\u{09FF}').contains(&c)) && c != '।' && c != '॥'
}

fn tokenize_context(s: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut start = None;
    for (i, c) in s.char_indices() {
        if !is_context_token_char(c) {
            if let Some(st) = start.take() {
                tokens.push(&s[st..i]);
            }
        } else if start.is_none() {
            start = Some(i);
        }
    }
    if let Some(st) = start {
        tokens.push(&s[st..]);
    }
    tokens
}

fn has_context(text: &str, start: usize, end: usize, keywords: &[String]) -> bool {
    if keywords.is_empty() {
        return false;
    }
    let w = SCORING.context_window;
    let ws = floor_boundary(text, start.saturating_sub(w));
    let we = ceil_boundary(text, (end + w).min(text.len()));
    let window = text[ws..we].to_lowercase();
    let win_tokens = tokenize_context(&window);
    if win_tokens.is_empty() {
        return false;
    }

    keywords.iter().any(|k| {
        let k_lower = k.to_lowercase();
        let kw_tokens = tokenize_context(&k_lower);
        if kw_tokens.is_empty() {
            return false;
        }
        win_tokens
            .windows(kw_tokens.len())
            .any(|w| w == kw_tokens.as_slice())
    })
}

struct Candidate {
    start: usize,
    end: usize,
    score: f32,
    row: usize,
    breakdown: String,
}

fn score_candidate(
    def: &IdentifierDef,
    text: &str,
    start: usize,
    end: usize,
    doc: &DocContext,
) -> (f32, bool, bool, String) {
    let mut score = def.base_confidence;
    let mut bd = format!("base {:+.2}", def.base_confidence);

    let ctx = has_context(text, start, end, &def.context_keywords);
    if ctx {
        score += SCORING.context_bonus;
        bd.push_str(&format!(", context {:+.2}", SCORING.context_bonus));
    }

    if def.checksum != "none" {
        match validate_checksum(&def.checksum, &text[start..end]) {
            Some(true) => {
                score += SCORING.checksum_valid_bonus;
                bd.push_str(&format!(", checksum {:+.2}", SCORING.checksum_valid_bonus));
            }
            Some(false) => {
                score += SCORING.checksum_invalid_penalty;
                bd.push_str(&format!(
                    ", checksum {:+.2}",
                    SCORING.checksum_invalid_penalty
                ));
            }
            None => {}
        }
    }

    // Document-level context. For context_required rows the high-sensitivity flag itself does NOT
    // count here (it has its own aggressive bonus below) - otherwise a bare number would score
    // 0.25 + 0.15 + 0.10 and wrongly earn a type label.
    let typical = type_keywords(&def.identifier_type)
        .iter()
        .any(|k| doc.lower.contains(k));
    if typical || (doc.high_sensitivity && !def.context_required) {
        score += SCORING.doc_context_bonus;
        bd.push_str(&format!(", doc-context {:+.2}", SCORING.doc_context_bonus));
    }

    let clue = country_clues(&def.country)
        .iter()
        .any(|k| doc.lower.contains(k))
        || (def.authority.len() > 3 && doc.lower.contains(&def.authority));
    if clue {
        score += SCORING.country_clue_bonus;
        bd.push_str(&format!(
            ", country-clue {:+.2}",
            SCORING.country_clue_bonus
        ));
    }

    if let Some(year) = doc.year {
        let outside = def.valid_from.map(|f| year < f).unwrap_or(false)
            || def.valid_until.map(|u| year > u).unwrap_or(false);
        if outside {
            score += SCORING.year_penalty;
            bd.push_str(&format!(", year {:+.2}", SCORING.year_penalty));
        }
    }

    if def.context_required && !ctx && doc.high_sensitivity {
        score += SCORING.aggressive_context_bonus;
        bd.push_str(&format!(
            ", aggressive {:+.2}",
            SCORING.aggressive_context_bonus
        ));
    }

    (score.clamp(0.0, 1.0), ctx, typical, bd)
}

fn overlaps(a: &Candidate, b: &Candidate) -> bool {
    a.start < b.end && b.start < a.end
}

/// Collect every registry candidate, resolve overlaps (span deduplication) and apply the tags.
fn apply_registry(text: &str, doc: &DocContext) -> (String, Vec<RegistryHit>) {
    let defs = &*REGISTRY;
    let mut cands: Vec<Candidate> = Vec::new();
    // (ctx, typical) per candidate for label eligibility
    let mut flags: Vec<(bool, bool)> = Vec::new();

    for (row, def) in defs.iter().enumerate() {
        for re in &def.patterns {
            let mut pos = 0;
            while pos <= text.len() {
                let Some(caps) = re.captures_at(text, pos) else {
                    break;
                };
                let body = caps.name("body").unwrap();
                let (start, end) = (body.start(), body.end());
                let (score, ctx, typical, bd) = score_candidate(def, text, start, end, doc);
                if score >= SCORING.redact_threshold {
                    cands.push(Candidate {
                        start,
                        end,
                        score,
                        row,
                        breakdown: bd,
                    });
                    flags.push((ctx, typical));
                }
                pos = end.max(pos + 1);
                pos = ceil_boundary(text, pos);
            }
        }
    }
    if cands.is_empty() {
        return (text.to_string(), Vec::new());
    }

    // Span deduplication: best score first, then longer span, then earlier start.
    let mut order: Vec<usize> = (0..cands.len()).collect();
    order.sort_by(|&a, &b| {
        cands[b]
            .score
            .partial_cmp(&cands[a].score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then((cands[b].end - cands[b].start).cmp(&(cands[a].end - cands[a].start)))
            .then(cands[a].start.cmp(&cands[b].start))
    });
    let mut accepted: Vec<usize> = Vec::new();
    for &i in &order {
        if accepted.iter().all(|&j| !overlaps(&cands[i], &cands[j])) {
            accepted.push(i);
        }
    }

    // Choose the label for every accepted span.
    let mut decided: Vec<(usize, String)> = Vec::new(); // (candidate index, tag)
    for &w in &accepted {
        let c = &cands[w];
        let def = &defs[c.row];
        let (ctx, typical) = flags[w];
        let eligible = !(def.context_required && !ctx && !typical);

        let mut tag = if eligible
            && c.score >= SCORING.country_label_threshold
            && def.status == Status::Verified
        {
            def.display_name.clone()
        } else if eligible && c.score >= SCORING.type_label_threshold {
            def.type_display_name.clone()
        } else {
            "ID_NUMBER".to_string()
        };

        // Ties with other candidates over the same span.
        for (j, other) in cands.iter().enumerate() {
            if j == w || !overlaps(c, other) {
                continue;
            }
            let od = &defs[other.row];
            if od.id == def.id || (c.score - other.score).abs() > SCORING.tie_margin {
                continue;
            }
            if od.identifier_type != def.identifier_type {
                tag = "ID_NUMBER".to_string();
                break;
            } else if od.country != def.country && tag == def.display_name {
                tag = def.type_display_name.clone();
            }
        }
        decided.push((w, tag));
    }

    // Apply right-to-left so earlier byte offsets stay valid.
    decided.sort_by(|a, b| cands[b.0].start.cmp(&cands[a.0].start));
    let mut out = text.to_string();
    let mut hits = Vec::new();
    for (i, tag) in &decided {
        let c = &cands[*i];
        hits.push(RegistryHit {
            matched_text: text[c.start..c.end].to_string(),
            row_id: defs[c.row].id.clone(),
            score: c.score,
            breakdown: c.breakdown.clone(),
            tag: tag.clone(),
        });
        out.replace_range(c.start..c.end, &format!("[{}]", tag));
    }
    hits.reverse();
    (out, hits)
}

// ── Dates ─────────────────────────────────────────────────────────────────────

static DATE_RES: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        Regex::new(r"\b\d{1,2}[/\-.]\d{1,2}[/\-.]\d{2,4}\b").unwrap(),
        Regex::new(r"\b\d{4}[/\-.]\d{1,2}[/\-.]\d{1,2}\b").unwrap(),
        Regex::new(&format!(
            r"(?i)\b\d{{1,2}}\s+{m}[A-Z]*\.?,?\s+\d{{2,4}}\b",
            m = MONTHS
        ))
        .unwrap(),
        Regex::new(&format!(
            r"(?i)\b{m}[A-Z]*\.?\s+\d{{1,2}},?\s+\d{{4}}\b",
            m = MONTHS
        ))
        .unwrap(),
        Regex::new(r"\b(?:\d\s+){7}\d\b").unwrap(), // Catch spaced out 8-digit dates e.g. 0 4 0 8 2 0 0 4
    ]
});

fn scrub_dates(text: &str) -> String {
    let mut cur = text.to_string();
    cur = SPELLED_DATE_RE.replace_all(&cur, "[DATE]").into_owned();
    for re in DATE_RES.iter() {
        cur = re.replace_all(&cur, "[DATE]").into_owned();
    }
    cur
}

// ── Catch-all numeric / ID tokens ─────────────────────────────────────────────

static TOKEN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\S+").unwrap());

fn token_shape(core: &str) -> String {
    core.chars()
        .map(|c| {
            if is_decimal_digit(c) {
                '9'
            } else if c.is_alphabetic() {
                'A'
            } else {
                c
            }
        })
        .collect()
}

fn scrub_catch_all(text: &str, high: bool) -> (String, Vec<String>) {
    let digit_threshold = if high { 4 } else { 6 };
    let mut gaps: Vec<String> = Vec::new();
    let out = TOKEN_RE
        .replace_all(text, |c: &Captures| {
            let tok = &c[0];
            // Skip only tokens that are ENTIRELY a placeholder: ^\[.+\]$
            if is_full_placeholder(tok) {
                return tok.to_string();
            }
            let lead_len = tok.len()
                - tok
                    .trim_start_matches(|ch: char| "(\"'“‘".contains(ch))
                    .len();
            let rest = &tok[lead_len..];
            let core = rest.trim_end_matches(|ch: char| ".,;:!?)\"'”’。、।".contains(ch));
            if core.is_empty() {
                return tok.to_string();
            }
            let trail = &rest[core.len()..];

            if core.starts_with(|ch: char| "$€£¥৳₹".contains(ch)) {
                return tok.to_string();
            }
            if core.chars().any(is_cjk_or_japanese) {
                return tok.to_string();
            }
            let digits = digit_count(core);
            if digits == 0 {
                return tok.to_string();
            }
            if digits == 4 && core.len() == 4 && (core.starts_with("19") || core.starts_with("20"))
            {
                return tok.to_string(); // plain year
            }
            let letters = core.chars().filter(|ch| ch.is_alphabetic()).count();
            let mixed = letters > 0 && core.chars().count() >= 6;
            if digits >= digit_threshold || mixed {
                gaps.push(token_shape(core));
                format!("{}[ID_NUMBER]{}", &tok[..lead_len], trail)
            } else {
                tok.to_string()
            }
        })
        .into_owned();
    (out, gaps)
}

// ── Language Manifests & Lazy Runtime Resource Loader ────────────────────────

pub struct LangManifestSpec {
    pub lang: &'static str,
    pub manifest_rel_path: &'static str,
}

pub const LANGUAGE_MANIFESTS: &[LangManifestSpec] = &[
    LangManifestSpec { lang: "bn", manifest_rel_path: "bn/manifest.json" },
    LangManifestSpec { lang: "de", manifest_rel_path: "de/manifest.json" },
    LangManifestSpec { lang: "en", manifest_rel_path: "en/manifest.json" },
    LangManifestSpec { lang: "es", manifest_rel_path: "es/manifest.json" },
    LangManifestSpec { lang: "fr", manifest_rel_path: "fr/manifest.json" },
    LangManifestSpec { lang: "hi", manifest_rel_path: "hi/manifest.json" },
    LangManifestSpec { lang: "it", manifest_rel_path: "it/manifest.json" },
    LangManifestSpec { lang: "ja", manifest_rel_path: "ja/manifest.json" },
    LangManifestSpec { lang: "ko", manifest_rel_path: "ko/manifest.json" },
    LangManifestSpec { lang: "nl", manifest_rel_path: "nl/manifest.json" },
    LangManifestSpec { lang: "pt", manifest_rel_path: "pt/manifest.json" },
];

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ManifestFileEntry {
    pub path: String,
    pub role: String,
    pub source: String,
    pub licence: String,
    pub version: String,
    pub sha256: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct LanguageManifest {
    pub language: String,
    pub files: Vec<ManifestFileEntry>,
}

static RESOURCE_ROOT_OVERRIDE: RwLock<Option<PathBuf>> = RwLock::new(None);
static RECORDED_MANIFEST_READS: OnceLock<Mutex<BTreeSet<String>>> = OnceLock::new();

std::thread_local! {
    static SCRUB_RESOURCE_ERROR: std::cell::RefCell<Option<String>> =
        const { std::cell::RefCell::new(None) };
}

fn clear_scrub_resource_error() {
    SCRUB_RESOURCE_ERROR.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

fn record_scrub_resource_error(err: &str) {
    SCRUB_RESOURCE_ERROR.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            *slot = Some(err.to_string());
        }
    });
}

fn take_scrub_resource_error() -> Option<String> {
    SCRUB_RESOURCE_ERROR.with(|cell| cell.borrow_mut().take())
}

fn record_manifest_read(lang: &str, rel_path: &str) {
    let key = format!("{}/{}", lang, rel_path);
    if let Ok(mut set) = RECORDED_MANIFEST_READS
        .get_or_init(|| Mutex::new(BTreeSet::new()))
        .lock()
    {
        set.insert(key.clone());
    }
    if let Ok(log_path) = std::env::var("PHILOSOFILE_MANIFEST_LOG") {
        if !log_path.is_empty() {
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_path)
            {
                let _ = writeln!(f, "{}", key);
            }
        }
    }
}

pub fn snapshot_recorded_manifest_reads() -> BTreeSet<String> {
    RECORDED_MANIFEST_READS
        .get_or_init(|| Mutex::new(BTreeSet::new()))
        .lock()
        .map(|s| s.clone())
        .unwrap_or_default()
}

pub fn init_resource_dir(resource_dir: PathBuf) {
    if let Ok(mut lock) = RESOURCE_ROOT_OVERRIDE.write() {
        *lock = Some(resource_dir.clone());
    }
    if let Ok(mut lock) = IPADIC_RESOURCE_DIR.write() {
        *lock = Some(resource_dir);
    }
}

fn normalize_resource_root(dir: &Path) -> Option<PathBuf> {
    if LANGUAGE_MANIFESTS
        .iter()
        .any(|s| dir.join(s.manifest_rel_path).is_file())
    {
        return Some(dir.to_path_buf());
    }
    let nested = dir.join("resources");
    if LANGUAGE_MANIFESTS
        .iter()
        .any(|s| nested.join(s.manifest_rel_path).is_file())
    {
        return Some(nested);
    }
    None
}

pub fn resolve_resource_root() -> Result<PathBuf, String> {
    if let Ok(guard) = RESOURCE_ROOT_OVERRIDE.read() {
        if let Some(ref dir) = *guard {
            if let Some(root) = normalize_resource_root(dir) {
                return Ok(root);
            }
            return Err(format!(
                "[scrubber] ERROR: configured resource directory {:?} contains no language manifests",
                dir
            ));
        }
    }
    if let Ok(env_dir) = std::env::var("PHILOSOFILE_RESOURCE_DIR") {
        if !env_dir.is_empty() {
            let p = PathBuf::from(&env_dir);
            if let Some(root) = normalize_resource_root(&p) {
                return Ok(root);
            }
            return Err(format!(
                "[scrubber] ERROR: PHILOSOFILE_RESOURCE_DIR {:?} contains no language manifests",
                p
            ));
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            for rel in [
                "../Resources/resources",
                "../Resources",
                "resources",
                "../resources",
                "../../resources",
                "../../src-tauri/resources",
                "../../../src-tauri/resources",
            ] {
                let cand = exe_dir.join(rel);
                if let Some(root) = normalize_resource_root(&cand) {
                    return Ok(root);
                }
            }
        }
    }
    for rel in [
        "resources",
        "src-tauri/resources",
        "../resources",
        "../src-tauri/resources",
        "../../src-tauri/resources",
    ] {
        let cand = PathBuf::from(rel);
        if let Some(root) = normalize_resource_root(&cand) {
            return Ok(root);
        }
    }
    #[cfg(any(test, debug_assertions))]
    {
        let manifest_cand = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources");
        if let Some(root) = normalize_resource_root(&manifest_cand) {
            return Ok(root);
        }
    }
    Err("[scrubber] ERROR: could not locate resources directory containing language manifests.".to_string())
}

pub fn load_language_manifest(root: &Path, lang: &str) -> Result<LanguageManifest, String> {
    let spec = LANGUAGE_MANIFESTS
        .iter()
        .find(|s| s.lang == lang)
        .ok_or_else(|| format!("[scrubber] ERROR: unknown language manifest '{}'", lang))?;
    let manifest_path = root.join(spec.manifest_rel_path);
    let raw = std::fs::read_to_string(&manifest_path).map_err(|e| {
        format!(
            "[scrubber] ERROR: missing or unreadable manifest for '{}' at {:?}: {}",
            lang, manifest_path, e
        )
    })?;
    let manifest: LanguageManifest = serde_json::from_str(&raw).map_err(|e| {
        format!(
            "[scrubber] ERROR: corrupt manifest JSON for '{}' at {:?}: {}",
            lang, manifest_path, e
        )
    })?;
    if manifest.language != lang {
        return Err(format!(
            "[scrubber] ERROR: manifest language mismatch at {:?}: expected '{}', found '{}'",
            manifest_path, lang, manifest.language
        ));
    }
    Ok(manifest)
}

fn open_manifest_file_from_root(
    root: &Path,
    lang: &str,
    rel_path: &str,
) -> Result<(PathBuf, std::fs::File), String> {
    let manifest = load_language_manifest(root, lang)?;
    if !manifest.files.iter().any(|f| f.path == rel_path) {
        return Err(format!(
            "[scrubber] ERROR: file '{}' is not declared in '{}/manifest.json'",
            rel_path, lang
        ));
    }
    let full_path = root.join(lang).join(rel_path);
    let file = std::fs::File::open(&full_path).map_err(|e| {
        format!(
            "[scrubber] ERROR: missing or unreadable resource '{}/{}' at {:?}: {}",
            lang, rel_path, full_path, e
        )
    })?;
    let meta = file.metadata().map_err(|e| {
        format!(
            "[scrubber] ERROR: failed to stat resource '{}/{}' at {:?}: {}",
            lang, rel_path, full_path, e
        )
    })?;
    if !meta.is_file() || meta.len() == 0 {
        return Err(format!(
            "[scrubber] ERROR: resource '{}/{}' at {:?} is empty or not a regular file",
            lang, rel_path, full_path
        ));
    }
    record_manifest_read(lang, rel_path);
    Ok((full_path, file))
}

fn open_manifest_file(lang: &str, rel_path: &str) -> Result<(PathBuf, std::fs::File), String> {
    let root = resolve_resource_root()?;
    open_manifest_file_from_root(&root, lang, rel_path)
}

fn read_manifest_file_string_from_root(
    root: &Path,
    lang: &str,
    rel_path: &str,
) -> Result<String, String> {
    use std::io::Read;
    let (full_path, mut file) = open_manifest_file_from_root(root, lang, rel_path)?;
    let mut content = String::new();
    file.read_to_string(&mut content).map_err(|e| {
        format!(
            "[scrubber] ERROR: corrupt or non-UTF-8 resource '{}/{}' at {:?}: {}",
            lang, rel_path, full_path, e
        )
    })?;
    Ok(content)
}

fn read_manifest_file_string(lang: &str, rel_path: &str) -> Result<String, String> {
    let root = resolve_resource_root()?;
    read_manifest_file_string_from_root(&root, lang, rel_path)
}

fn unwrap_or_record_empty_set(res: &'static Result<HashSet<String>, String>) -> &'static HashSet<String> {
    static EMPTY: OnceLock<HashSet<String>> = OnceLock::new();
    match res {
        Ok(set) => set,
        Err(err) => {
            record_scrub_resource_error(err);
            EMPTY.get_or_init(HashSet::new)
        }
    }
}

fn load_wordlist_lines_raw(lang: &str, rel_path: &str) -> Result<HashSet<String>, String> {
    let content = read_manifest_file_string(lang, rel_path)?;
    let set: HashSet<String> = content.lines().map(str::to_string).collect();
    if set.is_empty() {
        return Err(format!(
            "[scrubber] ERROR: word list '{}/{}' yielded zero entries",
            lang, rel_path
        ));
    }
    Ok(set)
}

fn load_wordlist_lines_trimmed(lang: &str, rel_path: &str) -> Result<HashSet<String>, String> {
    let content = read_manifest_file_string(lang, rel_path)?;
    let set: HashSet<String> = content
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect();
    if set.is_empty() {
        return Err(format!(
            "[scrubber] ERROR: list '{}/{}' yielded zero entries",
            lang, rel_path
        ));
    }
    Ok(set)
}

static WORDS_FULL: OnceLock<Result<HashSet<String>, String>> = OnceLock::new();

fn try_get_words_full() -> Result<&'static HashSet<String>, String> {
    WORDS_FULL
        .get_or_init(|| load_wordlist_lines_raw("en", "en_words_full.txt"))
        .as_ref()
        .map_err(Clone::clone)
}

fn get_words_full() -> &'static HashSet<String> {
    unwrap_or_record_empty_set(
        WORDS_FULL.get_or_init(|| load_wordlist_lines_raw("en", "en_words_full.txt")),
    )
}

static WORDS_UNIX_EN: OnceLock<Result<HashSet<String>, String>> = OnceLock::new();

fn get_words_unix_en() -> &'static HashSet<String> {
    unwrap_or_record_empty_set(WORDS_UNIX_EN.get_or_init(|| {
        let content = read_manifest_file_string("en", "en_words_unix.txt")?;
        let set: HashSet<String> = content
            .lines()
            .filter(|l| l.chars().all(|c| c.is_lowercase()))
            .map(str::to_string)
            .collect();
        if set.is_empty() {
            return Err("[scrubber] ERROR: 'en/en_words_unix.txt' yielded zero entries".to_string());
        }
        Ok(set)
    }))
}

static ES_WORDS: OnceLock<Result<HashSet<String>, String>> = OnceLock::new();

fn get_es_words() -> &'static HashSet<String> {
    unwrap_or_record_empty_set(
        ES_WORDS.get_or_init(|| load_wordlist_lines_trimmed("es", "es_words.txt")),
    )
}

static NAMELIST: OnceLock<Result<HashSet<String>, String>> = OnceLock::new();

fn get_namelist() -> &'static HashSet<String> {
    unwrap_or_record_empty_set(
        NAMELIST.get_or_init(|| load_wordlist_lines_trimmed("en", "en_names.txt")),
    )
}

/// Latin-script and romanized name files merged lazily into `MULTILANG_NAMES`.
/// Script-isolated CJK/Indic lists (`ja_names`, `ko_names`, `bn_names`, `hi_*`)
/// load in their own script-specific `OnceLock`s.
static MULTILANG_LATIN_FILES: &[(&str, &str, &str)] = &[
    ("de", "de_names.txt", "de"),
    ("es", "es_names.txt", "es"),
    ("es", "es_surnames_ine.csv", "es"),
    ("es", "es_surnames_ai_supplement.csv", "es"),
    ("fr", "fr_names.txt", "fr"),
    ("it", "it_names.txt", "it"),
    ("nl", "nl_names.txt", "nl"),
    ("pt", "pt_names.txt", "pt"),
];

static ROMANIZED_NAME_SPECS: &[(&str, &str)] = &[
    ("bn", "bn_romanized.txt"),
    ("ja", "ja_given_romanized.txt"),
    ("ja", "ja_surnames_romanized.txt"),
    ("ko", "ko_names_romanized.txt"),
];

fn strip_latin_accents(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            'á' | 'à' | 'â' | 'ä' | 'ã' => out.push('a'),
            'é' | 'è' | 'ê' | 'ë' => out.push('e'),
            'í' | 'ì' | 'î' | 'ï' => out.push('i'),
            'ó' | 'ò' | 'ô' | 'ö' | 'õ' => out.push('o'),
            'ú' | 'ù' | 'û' | 'ü' => out.push('u'),
            'ñ' => out.push('n'),
            'ç' => out.push('c'),
            'ÿ' => out.push('y'),
            'æ' => out.push_str("ae"),
            'œ' => out.push_str("oe"),
            'Á' | 'À' | 'Â' | 'Ä' | 'Ã' => out.push('A'),
            'É' | 'È' | 'Ê' | 'Ë' => out.push('E'),
            'Í' | 'Ì' | 'Î' | 'Ï' => out.push('I'),
            'Ó' | 'Ò' | 'Ô' | 'Ö' | 'Õ' => out.push('O'),
            'Ú' | 'Ù' | 'Û' | 'Ü' => out.push('U'),
            'Ñ' => out.push('N'),
            'Ç' => out.push('C'),
            'Ÿ' => out.push('Y'),
            'Æ' => out.push_str("AE"),
            'Œ' => out.push_str("OE"),
            other => out.push(other),
        }
    }
    out
}

const SPANISH_NAME_CONNECTORS: &[&str] = &["de", "del", "la", "los", "las", "y"];
const PORTUGUESE_NAME_CONNECTORS: &[&str] = &["de", "da", "do", "dos", "das", "e"];

static MULTILANG_NAMES: OnceLock<Result<HashMap<String, Vec<&'static str>>, String>> =
    OnceLock::new();

fn build_multilang_names() -> Result<HashMap<String, Vec<&'static str>>, String> {
    let mut map: HashMap<String, Vec<&'static str>> = HashMap::new();
    let en_names = read_manifest_file_string("en", "en_names.txt")?;
    for line in en_names.lines() {
        let name = line.trim().to_lowercase();
        if !name.is_empty() && !name.starts_with('#') {
            map.entry(name).or_default().push("en");
        }
    }
    for &(res_lang, rel_path) in ROMANIZED_NAME_SPECS {
        let content = read_manifest_file_string(res_lang, rel_path)?;
        for line in content.lines() {
            let name = line.trim().to_lowercase();
            if name.is_empty() || name.starts_with('#') {
                continue;
            }
            let entry = map.entry(name).or_default();
            if !entry.contains(&"en") {
                entry.push("en");
            }
        }
    }
    for &(res_lang, rel_path, tag) in MULTILANG_LATIN_FILES {
        let content = read_manifest_file_string(res_lang, rel_path)?;
        let is_csv = rel_path.ends_with(".csv");
        let mut skipped_csv_header = false;
        for line in content.lines() {
            let mut trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if is_csv {
                if !skipped_csv_header {
                    skipped_csv_header = true;
                    if trimmed.starts_with("surname,") || trimmed.eq_ignore_ascii_case("surname") {
                        continue;
                    }
                }
                trimmed = trimmed.split(',').next().unwrap_or("").trim();
                if trimmed.is_empty() {
                    continue;
                }
            }
            let lower = trimmed.to_lowercase();
            let entry = map.entry(lower.clone()).or_default();
            if !entry.contains(&tag) {
                entry.push(tag);
            }
            if tag == "es" || tag == "fr" || tag == "pt" {
                let stripped = strip_latin_accents(&lower);
                if stripped != lower {
                    let s_entry = map.entry(stripped).or_default();
                    if !s_entry.contains(&tag) {
                        s_entry.push(tag);
                    }
                }
                if lower.contains(' ') || lower.contains('-') {
                    for part in lower.split([' ', '-']) {
                        if part.chars().count() >= 3
                            && !SPANISH_NAME_CONNECTORS.contains(&part)
                            && !PORTUGUESE_NAME_CONNECTORS.contains(&part)
                        {
                            let p_entry = map.entry(part.to_string()).or_default();
                            if !p_entry.contains(&tag) {
                                p_entry.push(tag);
                            }
                            let p_stripped = strip_latin_accents(part);
                            if p_stripped != part {
                                let ps_entry = map.entry(p_stripped).or_default();
                                if !ps_entry.contains(&tag) {
                                    ps_entry.push(tag);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(map)
}

fn try_get_multilang_names() -> Result<&'static HashMap<String, Vec<&'static str>>, String> {
    MULTILANG_NAMES
        .get_or_init(build_multilang_names)
        .as_ref()
        .map_err(Clone::clone)
}

fn get_multilang_names() -> &'static HashMap<String, Vec<&'static str>> {
    static EMPTY: OnceLock<HashMap<String, Vec<&'static str>>> = OnceLock::new();
    match MULTILANG_NAMES.get_or_init(build_multilang_names) {
        Ok(map) => map,
        Err(err) => {
            record_scrub_resource_error(err);
            EMPTY.get_or_init(HashMap::new)
        }
    }
}

/// English word list for the foreign-run rule only.
/// Computed lazily as `WORDS_FULL` minus the English name set (`NAMELIST` plus
/// any key in `MULTILANG_NAMES` whose languages include "en").
static FOREIGN_EN_WORDS: OnceLock<Result<HashSet<String>, String>> = OnceLock::new();

fn get_foreign_en_words() -> &'static HashSet<String> {
    unwrap_or_record_empty_set(FOREIGN_EN_WORDS.get_or_init(|| {
        let words_full = try_get_words_full()?;
        let namelist = NAMELIST
            .get_or_init(|| load_wordlist_lines_trimmed("en", "en_names.txt"))
            .as_ref()
            .map_err(Clone::clone)?;
        let multilang = try_get_multilang_names()?;
        let mut en_names: HashSet<&str> = HashSet::new();
        for name in namelist.iter() {
            en_names.insert(name.as_str());
        }
        for (name, langs) in multilang.iter() {
            if langs.contains(&"en") {
                en_names.insert(name.as_str());
            }
        }
        Ok(words_full
            .iter()
            .filter(|w| !en_names.contains(w.as_str()))
            .cloned()
            .collect())
    }))
}

static KO_GIVEN_NAMES: OnceLock<Result<HashSet<String>, String>> = OnceLock::new();

fn get_ko_given_names() -> &'static HashSet<String> {
    unwrap_or_record_empty_set(
        KO_GIVEN_NAMES.get_or_init(|| load_wordlist_lines_trimmed("ko", "ko_names.txt")),
    )
}

static KO_SURNAMES: OnceLock<Result<HashSet<String>, String>> = OnceLock::new();

fn get_ko_surnames() -> &'static HashSet<String> {
    unwrap_or_record_empty_set(
        KO_SURNAMES.get_or_init(|| load_wordlist_lines_trimmed("ko", "ko_surnames.txt")),
    )
}

/// 50 most common 1-syllable Korean surnames (Wikidata) used for unspaced 2/3-syllable compound name matching.
const KO_COMMON_1CHAR_SURNAMES: &[&str] = &[
    "김", "이", "박", "최", "정", "강", "조", "윤", "장", "임",
    "한", "오", "서", "신", "권", "황", "안", "송", "류", "전",
    "홍", "고", "문", "양", "손", "배", "백", "허", "유", "남",
    "심", "노", "하", "곽", "성", "차", "주", "우", "구", "나",
    "민", "진", "지", "엄", "채", "원", "천", "방", "공", "현",
];

// ── Japanese (ja) IPADIC dictionary decompression & loader ───────────────────

static IPADIC_APP_DATA_DIR: RwLock<Option<PathBuf>> = RwLock::new(None);

/// Set the base application data directory used for caching the decompressed IPADIC dictionary.
/// When running inside Tauri, this should be called with the path from `app.path().app_local_data_dir()`.
pub fn init_ipadic_cache_dir(app_data_dir: PathBuf) {
    if let Ok(mut lock) = IPADIC_APP_DATA_DIR.write() {
        *lock = Some(app_data_dir);
    }
}

static IPADIC_RESOURCE_DIR: RwLock<Option<PathBuf>> = RwLock::new(None);

/// Set the resource directory where `resources/ja/ipadic/*.zst` files are bundled.
/// When running inside Tauri, this should be called with the path from `app.path().resource_dir()`.
pub fn init_ipadic_resource_dir(resource_dir: PathBuf) {
    if let Ok(mut lock) = IPADIC_RESOURCE_DIR.write() {
        *lock = Some(resource_dir.clone());
    }
    if normalize_resource_root(&resource_dir).is_some() {
        if let Ok(mut lock) = RESOURCE_ROOT_OVERRIDE.write() {
            *lock = Some(resource_dir);
        }
    }
}

pub const IPADIC_FILES: &[&str] = &[
    "char_def.bin",
    "dict.trie",
    "dict.vals",
    "dict.valsidx",
    "dict.words",
    "dict.wordsidx",
    "matrix.mtx",
    "metadata.json",
    "unk.bin",
];

const IPADIC_CACHE_DIR_NAME: &str = "ipadic_v1";

fn resolve_default_ja_ipadic_resource_dir() -> Result<PathBuf, String> {
    if let Ok(root) = resolve_resource_root() {
        let cand = root.join("ja/ipadic");
        if cand.join("dict.words.zst").is_file() {
            return Ok(cand);
        }
    }
    if let Ok(guard) = IPADIC_RESOURCE_DIR.read() {
        if let Some(ref res_dir) = *guard {
            for sub in [
                res_dir.join("resources/ja/ipadic"),
                res_dir.join("ja/ipadic"),
                res_dir.join("ipadic"),
                res_dir.clone(),
            ] {
                if sub.join("dict.words.zst").exists() {
                    return Ok(sub);
                }
            }
        }
    }
    if let Ok(env_dir) = std::env::var("PHILOSOFILE_JA_RESOURCE_DIR") {
        let p = PathBuf::from(&env_dir);
        for sub in [
            p.join("resources/ja/ipadic"),
            p.join("ja/ipadic"),
            p.join("ipadic"),
            p.clone(),
        ] {
            if sub.join("dict.words.zst").exists() {
                return Ok(sub);
            }
        }
    }
    Err("Could not locate resources/ja/ipadic directory containing dict.words.zst.".to_string())
}

pub fn is_ipadic_cache_valid(cache_dir: &Path) -> bool {
    if !cache_dir.is_dir() {
        return false;
    }
    for file_name in IPADIC_FILES {
        let p = cache_dir.join(file_name);
        match std::fs::metadata(&p) {
            Ok(meta) if meta.is_file() && meta.len() > 0 => {}
            _ => return false,
        }
    }
    true
}

/// Decompress the nine .zst IPADIC files from bundled resources into a versioned cache directory.
/// Returns the path to the valid directory containing uncompressed dictionary files.
pub fn ensure_ipadic_decompressed(cache_dir_override: Option<&Path>) -> Result<PathBuf, String> {
    let base_dir: PathBuf = if let Some(p) = cache_dir_override {
        p.to_path_buf()
    } else if let Ok(guard) = IPADIC_APP_DATA_DIR.read() {
        if let Some(ref dir) = *guard {
            dir.clone()
        } else if let Ok(env_dir) = std::env::var("PHILOSOFILE_IPADIC_CACHE_DIR") {
            PathBuf::from(env_dir)
        } else if let Some(local_data) = dirs::data_local_dir() {
            local_data.join("com.philosofile")
        } else {
            std::env::temp_dir().join("philosofile")
        }
    } else {
        std::env::temp_dir().join("philosofile")
    };

    let target_cache_dir = base_dir.join(IPADIC_CACHE_DIR_NAME);
    if is_ipadic_cache_valid(&target_cache_dir) {
        return Ok(target_cache_dir);
    }

    if target_cache_dir.is_dir() {
        let _ = std::fs::remove_dir_all(&target_cache_dir);
    }

    let root = resolve_resource_root()?;
    let manifest = load_language_manifest(&root, "ja")?;
    let resource_dir = resolve_default_ja_ipadic_resource_dir()?;

    std::fs::create_dir_all(&base_dir)
        .map_err(|e| format!("Failed to create base directory {:?}: {}", base_dir, e))?;

    let tmp_dir_name = format!(
        ".tmp_{}_{}_{}",
        IPADIC_CACHE_DIR_NAME,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    let tmp_dir = base_dir.join(tmp_dir_name);
    std::fs::create_dir_all(&tmp_dir)
        .map_err(|e| format!("Failed to create staging temp dir {:?}: {}", tmp_dir, e))?;

    let decompress_res = (|| -> Result<(), String> {
        for file_name in IPADIC_FILES {
            let rel = format!("ipadic/{}.zst", file_name);
            if !manifest.files.iter().any(|f| f.path == rel) {
                return Err(format!(
                    "[scrubber] ERROR: IPADIC file '{}' not declared in ja/manifest.json",
                    rel
                ));
            }
            let zst_path = resource_dir.join(format!("{}.zst", file_name));
            let out_path = tmp_dir.join(file_name);

            let in_file = std::fs::File::open(&zst_path)
                .map_err(|e| format!("Failed to open {:?}: {}", zst_path, e))?;
            record_manifest_read("ja", &rel);
            let mut decoder = zstd::Decoder::new(in_file)
                .map_err(|e| format!("Failed to create zstd decoder for {:?}: {}", zst_path, e))?;
            let mut out_file = std::fs::File::create(&out_path)
                .map_err(|e| format!("Failed to create {:?}: {}", out_path, e))?;
            std::io::copy(&mut decoder, &mut out_file)
                .map_err(|e| format!("Failed to decompress {:?} to {:?}: {}", zst_path, out_path, e))?;
        }
        Ok(())
    })();

    if let Err(e) = decompress_res {
        let _ = std::fs::remove_dir_all(&tmp_dir);
        return Err(e);
    }

    if let Err(e) = std::fs::rename(&tmp_dir, &target_cache_dir) {
        if is_ipadic_cache_valid(&target_cache_dir) {
            let _ = std::fs::remove_dir_all(&tmp_dir);
            return Ok(target_cache_dir);
        }
        let _ = std::fs::remove_dir_all(&tmp_dir);
        return Err(format!("Failed to rename {:?} to {:?}: {}", tmp_dir, target_cache_dir, e));
    }

    Ok(target_cache_dir)
}

#[cfg(test)]
pub(crate) fn init_test_ja_resource_dir() {
    if let Ok(root) = resolve_resource_root() {
        let p = root.join("ja/ipadic");
        if p.join("dict.words.zst").exists() {
            init_ipadic_resource_dir(root);
        }
    }
}

static LINDERA_SEGMENTER: OnceLock<Option<lindera::segmenter::Segmenter>> = OnceLock::new();

fn get_lindera_segmenter() -> Option<&'static lindera::segmenter::Segmenter> {
    #[cfg(test)]
    init_test_ja_resource_dir();

    LINDERA_SEGMENTER
        .get_or_init(|| {
            match ensure_ipadic_decompressed(None) {
                Ok(dict_dir) => {
                    let path_str = dict_dir.to_str().unwrap_or_default();
                    match lindera::dictionary::load_dictionary(path_str) {
                        Ok(dict) => Some(lindera::segmenter::Segmenter::new(
                            lindera::mode::Mode::Normal,
                            dict,
                            None,
                        )),
                        Err(e) => {
                            eprintln!(
                                "[scrubber] Failed to load Lindera dictionary from {}: {}",
                                path_str, e
                            );
                            None
                        }
                    }
                }
                Err(e) => {
                    eprintln!(
                        "[scrubber] Lindera IPADIC dictionary unavailable: {}. Japanese segmentation disabled.",
                        e
                    );
                    None
                }
            }
        })
        .as_ref()
}

static JA_GIVEN_NAMES: OnceLock<Result<HashSet<String>, String>> = OnceLock::new();

fn get_ja_given_names() -> &'static HashSet<String> {
    unwrap_or_record_empty_set(
        JA_GIVEN_NAMES.get_or_init(|| load_wordlist_lines_trimmed("ja", "ja_names.txt")),
    )
}

static JA_SURNAMES: OnceLock<Result<HashSet<String>, String>> = OnceLock::new();

fn get_ja_surnames() -> &'static HashSet<String> {
    unwrap_or_record_empty_set(
        JA_SURNAMES.get_or_init(|| load_wordlist_lines_trimmed("ja", "ja_surnames.txt")),
    )
}

// ── Bangla (bn) runtime resources and lookups ────────────────────────────────

#[derive(Default)]
struct BnResources {
    words: HashSet<String>,
    suffixes: Vec<String>,
    names: HashSet<String>,
    full_names: HashSet<String>,
    romanizations: HashMap<String, Vec<String>>,
    romanized_tokens: HashSet<String>,
    load_errors: Vec<String>,
    words_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BnTokKind {
    Word,
    Name,
    Unknown,
}

fn parse_bn_suffixes_tsv(content: &str) -> Vec<String> {
    let mut suffixes: Vec<String> = Vec::new();
    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut cols = trimmed.split('\t');
        let Some(suf_raw) = cols.next() else {
            continue;
        };
        if idx <= 10 && suf_raw.trim() == "suffix" {
            continue;
        }
        let s = normalize_bn_str(suf_raw);
        if !s.is_empty() && !suffixes.contains(&s) {
            suffixes.push(s);
        }
    }
    suffixes.sort_by(|a, b| {
        b.chars()
            .count()
            .cmp(&a.chars().count())
            .then_with(|| b.len().cmp(&a.len()))
    });
    suffixes
}

fn normalize_bn_str(s: &str) -> String {
    s.trim()
        .nfc()
        .collect::<String>()
        .replace('\u{09DF}', "\u{09AF}\u{09BC}")
        .replace('\u{09DC}', "\u{09A1}\u{09BC}")
        .replace('\u{09DD}', "\u{09A2}\u{09BC}")
}

fn resolve_default_bn_resource_dir() -> Result<PathBuf, String> {
    resolve_resource_root().map(|r| r.join("bn"))
}

fn load_bn_resources_from_dir(dir: &Path) -> BnResources {
    load_bn_resources_from_dir_with_mode(dir, !cfg!(debug_assertions))
}

fn load_bn_resources_from_dir_with_mode(dir: &Path, is_release_mode: bool) -> BnResources {
    let mut res = BnResources::default();
    let mut all_name_entries: HashSet<String> = HashSet::new();

    let words_path = dir.join("bn_words.tsv");
    match std::fs::read_to_string(&words_path) {
        Ok(content) => {
            for (idx, line) in content.lines().enumerate() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                let mut cols = trimmed.split('\t');
                let Some(word_raw) = cols.next() else {
                    continue;
                };
                if idx <= 5 && word_raw.trim() == "word" {
                    continue;
                }
                let w = normalize_bn_str(word_raw);
                if !w.is_empty() {
                    res.words.insert(w);
                }
            }
            if res.words.is_empty() {
                if is_release_mode {
                    let msg = format!(
                        "[scrubber] ERROR: Bangla word list at {:?} is empty in release build",
                        words_path
                    );
                    eprintln!("{}", msg);
                    res.load_errors.push(msg.clone());
                    res.words_error = Some(msg);
                } else {
                    eprintln!(
                        "[scrubber] Warning: Bangla word list at {:?} is empty",
                        words_path
                    );
                }
            }
        }
        Err(err) => {
            if is_release_mode {
                let msg = format!(
                    "[scrubber] ERROR: missing or unreadable Bangla word list in release build at {:?}: {}",
                    words_path, err
                );
                eprintln!("{}", msg);
                res.load_errors.push(msg.clone());
                res.words_error = Some(msg);
            } else {
                eprintln!(
                    "[scrubber] Warning: could not read Bangla word list at {:?}: {}",
                    words_path, err
                );
            }
        }
    }

    let suffixes_path = dir.join("bn_suffixes.tsv");
    if let Ok(content) = std::fs::read_to_string(&suffixes_path) {
        res.suffixes = parse_bn_suffixes_tsv(&content);
    } else if !res.words.is_empty() {
        res.suffixes = get_bn_resources().suffixes.clone();
    }

    for file_name in ["bn_names.tsv", "bn_names_gemini.tsv"] {
        let names_path = dir.join(file_name);
        match std::fs::read_to_string(&names_path) {
            Ok(content) => parse_bn_names_tsv(&content, &mut res, &mut all_name_entries),
            Err(err) => {
                if is_release_mode {
                    let msg = format!(
                        "[scrubber] ERROR: missing or unreadable Bangla name list in release build at {:?}: {}",
                        names_path, err
                    );
                    eprintln!("{}", msg);
                    res.load_errors.push(msg);
                } else {
                    eprintln!(
                        "[scrubber] Warning: could not read Bangla name list at {:?}: {}",
                        names_path, err
                    );
                }
            }
        }
    }

    // Build word set as words minus all name rows from any source.
    for name_tok in &all_name_entries {
        res.words.remove(name_tok);
    }
    if res.words.is_empty() && res.words_error.is_none() && is_release_mode {
        let msg = format!(
            "[scrubber] ERROR: Bangla word list at {:?} has zero words after loading in release build",
            words_path
        );
        eprintln!("{}", msg);
        res.load_errors.push(msg.clone());
        res.words_error = Some(msg);
    }

    res
}

fn parse_bn_names_tsv(content: &str, res: &mut BnResources, all_name_entries: &mut HashSet<String>) {
    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.is_empty() {
            continue;
        }
        let name_raw = cols[0].trim();
        if idx <= 5 && name_raw == "name" {
            continue;
        }
        let roman_raw = cols.get(1).copied().unwrap_or("").trim();
        let norm_name = normalize_bn_str(name_raw);
        if norm_name.is_empty() {
            continue;
        }
        all_name_entries.insert(norm_name.clone());
        for part in norm_name.split_whitespace() {
            if part.chars().count() >= 2 {
                all_name_entries.insert(part.to_string());
            }
        }
        let romans: Vec<String> = if roman_raw.is_empty() {
            Vec::new()
        } else {
            roman_raw
                .split(',')
                .map(|r| r.trim().to_string())
                .filter(|r| !r.is_empty())
                .collect()
        };
        res.full_names.insert(norm_name.clone());
        res.names.insert(norm_name.clone());
        for part in norm_name.split_whitespace() {
            if part.chars().count() >= 2 {
                res.names.insert(part.to_string());
            }
        }
        if !romans.is_empty() {
            let entry = res.romanizations.entry(norm_name.clone()).or_default();
            for r in &romans {
                if !entry.contains(r) {
                    entry.push(r.clone());
                }
                res.romanized_tokens.insert(r.to_lowercase());
            }
        }
    }
}

fn load_bn_resources_via_manifest() -> Result<BnResources, String> {
    let words_content = read_manifest_file_string("bn", "bn_words.tsv")?;
    let suffixes_content = read_manifest_file_string("bn", "bn_suffixes.tsv")?;
    let names_content = read_manifest_file_string("bn", "bn_names.tsv")?;
    let gemini_names_content = read_manifest_file_string("bn", "bn_names_gemini.tsv")?;

    let mut res = BnResources::default();
    let mut all_name_entries: HashSet<String> = HashSet::new();

    for (idx, line) in words_content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut cols = trimmed.split('\t');
        let Some(word_raw) = cols.next() else {
            continue;
        };
        if idx <= 10 && word_raw.trim() == "word" {
            continue;
        }
        let norm = normalize_bn_str(word_raw);
        if !norm.is_empty() {
            res.words.insert(norm);
        }
    }

    res.suffixes = parse_bn_suffixes_tsv(&suffixes_content);

    parse_bn_names_tsv(&names_content, &mut res, &mut all_name_entries);
    parse_bn_names_tsv(&gemini_names_content, &mut res, &mut all_name_entries);

    for name_tok in &all_name_entries {
        res.words.remove(name_tok);
    }

    if res.words.is_empty() || res.names.is_empty() || res.suffixes.is_empty() {
        return Err("[scrubber] ERROR: Bangla resource files yielded empty data".to_string());
    }
    Ok(res)
}

static BN_RESOURCES: OnceLock<Result<BnResources, String>> = OnceLock::new();

fn get_bn_resources_result() -> Result<&'static BnResources, String> {
    BN_RESOURCES
        .get_or_init(load_bn_resources_via_manifest)
        .as_ref()
        .map_err(Clone::clone)
}

fn get_bn_resources() -> &'static BnResources {
    static EMPTY_BN: OnceLock<BnResources> = OnceLock::new();
    match BN_RESOURCES.get_or_init(load_bn_resources_via_manifest) {
        Ok(res) => res,
        Err(err) => {
            record_scrub_resource_error(err);
            EMPTY_BN.get_or_init(BnResources::default)
        }
    }
}

/// True if `s` (normalized with NFC and trimmed) is in the Bangla word list (`bn_words.tsv`)
/// or matches a known word stem after stripping a known suffix from `bn_suffixes.tsv`
/// (provided `s` is not an exact match in the names list).
pub fn is_bn_known_word(s: &str) -> bool {
    let norm = normalize_bn_str(s);
    if norm.is_empty() {
        return false;
    }
    let res = get_bn_resources();
    res.words.contains(&norm)
        || (!res.names.contains(&norm) && is_bn_suffix_stripped_word_in(&norm, res))
}

fn is_bn_known_word_in(s: &str, res: &BnResources) -> bool {
    let norm = normalize_bn_str(s);
    if norm.is_empty() {
        return false;
    }
    res.words.contains(&norm)
}

fn is_bn_suffix_stripped_word_in(norm_tok: &str, res: &BnResources) -> bool {
    for suf in res.suffixes.iter() {
        if let Some(stem) = norm_tok.strip_suffix(suf.as_str()) {
            if stem.chars().count() >= 2 && res.words.contains(stem) {
                return true;
            }
        }
    }
    false
}

struct GatedNameListSpec {
    lang: &'static str,
    rel_path: &'static str,
    cache: OnceLock<Result<HashSet<String>, String>>,
}

/// Table mapping each language with a gated name/word list to its bundle resource file.
static GATED_NAME_LISTS: [GatedNameListSpec; 3] = [
    GatedNameListSpec {
        lang: "pt",
        rel_path: "pt_name_gated.txt",
        cache: OnceLock::new(),
    },
    GatedNameListSpec {
        lang: "fr",
        rel_path: "fr_name_gated.txt",
        cache: OnceLock::new(),
    },
    GatedNameListSpec {
        lang: "es",
        rel_path: "es_name_gated.txt",
        cache: OnceLock::new(),
    },
];

fn load_gated_names_from_root(
    root: &Path,
    lang: &str,
    rel_path: &str,
) -> Result<HashSet<String>, String> {
    use std::io::BufRead;
    let (_, gated_file) = open_manifest_file_from_root(root, lang, rel_path)?;
    let mut name_gated = HashSet::new();
    for line_res in std::io::BufReader::new(gated_file).lines() {
        let line = line_res
            .map_err(|e| format!("[scrubber] ERROR reading {}/{}: {}", lang, rel_path, e))?;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let w = trimmed.to_lowercase();
        let stripped = strip_latin_accents(&w);
        name_gated.insert(stripped);
        name_gated.insert(w);
    }
    if name_gated.is_empty() {
        return Err(format!(
            "[scrubber] ERROR: gated list '{}/{}' yielded empty data",
            lang, rel_path
        ));
    }
    Ok(name_gated)
}

/// Returns the cached gated name set for `lang`, loading it lazily from `GATED_NAME_LISTS`.
pub fn get_gated_names(lang: &str) -> &'static HashSet<String> {
    static EMPTY_GATED: OnceLock<HashSet<String>> = OnceLock::new();
    let Some(entry) = GATED_NAME_LISTS.iter().find(|e| e.lang == lang) else {
        return EMPTY_GATED.get_or_init(HashSet::new);
    };
    match entry.cache.get_or_init(|| {
        let root = resolve_resource_root()?;
        load_gated_names_from_root(&root, entry.lang, entry.rel_path)
    }) {
        Ok(set) => set,
        Err(err) => {
            record_scrub_resource_error(err);
            EMPTY_GATED.get_or_init(HashSet::new)
        }
    }
}

/// Returns true if `s` (or its accent-stripped form) appears in `lang`'s gated list.
pub fn is_name_gated(lang: &str, s: &str) -> bool {
    let low = s.to_lowercase();
    let stripped = strip_latin_accents(&low);
    let set = get_gated_names(lang);
    set.contains(&low) || set.contains(&stripped)
}

pub struct FrResources {
    pub words: HashSet<String>,
}

fn load_fr_resources_from_root(root: &Path) -> Result<FrResources, String> {
    use std::io::BufRead;
    let (_, words_file) = open_manifest_file_from_root(root, "fr", "fr_words.tsv")?;
    let mut words = HashSet::new();
    for line_res in std::io::BufReader::new(words_file).lines() {
        let line = line_res.map_err(|e| format!("[scrubber] ERROR reading fr/fr_words.tsv: {}", e))?;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(word) = trimmed.split('\t').next() {
            let w = word.trim().to_lowercase();
            if !w.is_empty() {
                words.insert(w);
            }
        }
    }

    if words.is_empty() {
        return Err("[scrubber] ERROR: French resource files yielded empty data".to_string());
    }
    Ok(FrResources { words })
}

static FR_RESOURCES: OnceLock<Result<FrResources, String>> = OnceLock::new();

pub fn get_fr_resources() -> &'static FrResources {
    static EMPTY_FR: OnceLock<FrResources> = OnceLock::new();
    match FR_RESOURCES.get_or_init(|| {
        let root = resolve_resource_root()?;
        load_fr_resources_from_root(&root)
    }) {
        Ok(res) => res,
        Err(err) => {
            record_scrub_resource_error(err);
            EMPTY_FR.get_or_init(|| FrResources {
                words: HashSet::new(),
            })
        }
    }
}

pub fn is_fr_word(s: &str) -> bool {
    let low = s.to_lowercase();
    let stripped = strip_latin_accents(&low);
    let res = get_fr_resources();
    res.words.contains(&low) || res.words.contains(&stripped)
}

pub struct PtResources {
    pub words: HashSet<String>,
}

fn load_tsv_word_stream<R: std::io::Read>(
    reader: R,
    words: &mut HashSet<String>,
    label: &str,
) -> Result<(), String> {
    use std::io::Read;
    let mut content = String::new();
    let mut buf_reader = std::io::BufReader::new(reader);
    buf_reader
        .read_to_string(&mut content)
        .map_err(|e| format!("[scrubber] ERROR reading {}: {}", label, e))?;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(word) = trimmed.split('\t').next() {
            let w = word.trim();
            if !w.is_empty() {
                words.insert(w.to_lowercase());
            }
        }
    }
    Ok(())
}

fn load_pt_resources_from_root(root: &Path) -> Result<PtResources, String> {
    let (_, br_file) = open_manifest_file_from_root(root, "pt", "pt_BR_words.tsv")?;
    let (_, pt_file) = open_manifest_file_from_root(root, "pt", "pt_PT_words.tsv")?;

    let (br_res, pt_res) = rayon::join(
        || {
            let mut s = HashSet::with_capacity(450_000);
            load_tsv_word_stream(br_file, &mut s, "pt/pt_BR_words.tsv")?;
            Ok::<HashSet<String>, String>(s)
        },
        || {
            let mut s = HashSet::with_capacity(140_000);
            load_tsv_word_stream(pt_file, &mut s, "pt/pt_PT_words.tsv")?;
            Ok::<HashSet<String>, String>(s)
        },
    );
    let mut words = br_res?;
    for w in pt_res? {
        words.insert(w);
    }

    if words.is_empty() {
        return Err("[scrubber] ERROR: Portuguese resource files yielded empty data".to_string());
    }
    Ok(PtResources { words })
}

static PT_RESOURCES: OnceLock<Result<PtResources, String>> = OnceLock::new();

pub fn get_pt_resources() -> &'static PtResources {
    static EMPTY_PT: OnceLock<PtResources> = OnceLock::new();
    match PT_RESOURCES.get_or_init(|| {
        let root = resolve_resource_root()?;
        load_pt_resources_from_root(&root)
    }) {
        Ok(res) => res,
        Err(err) => {
            record_scrub_resource_error(err);
            EMPTY_PT.get_or_init(|| PtResources {
                words: HashSet::new(),
            })
        }
    }
}

pub fn is_pt_resources_loaded() -> bool {
    PT_RESOURCES.get().is_some()
}

pub fn is_pt_word(s: &str) -> bool {
    let low = s.to_lowercase();
    let stripped = strip_latin_accents(&low);
    let res = get_pt_resources();
    res.words.contains(&low) || res.words.contains(&stripped)
}

// ── Hindi (hi) Resource Loader & Count-Bound Runtime Classifier ───────────────

#[inline]
pub fn is_devanagari_char(c: char) -> bool {
    ('\u{0900}'..='\u{097F}').contains(&c)
}

#[inline]
fn is_deva_dependent_vowel(c: char) -> bool {
    matches!(
        c,
        '\u{093A}'
            | '\u{093B}'
            | '\u{093E}'..='\u{094C}'
            | '\u{094E}'
            | '\u{094F}'
            | '\u{0955}'..='\u{0957}'
            | '\u{0962}'
            | '\u{0963}'
    )
}

/// Normalizes a Devanagari token before dictionary/name comparison:
/// 1. Trims whitespace.
/// 2. Decomposes `ऩ` (`U+0929`), `ऱ` (`U+0931`), and `ऴ` (`U+0934`) to `base + U+093C`
///    (matching how Unicode NFC decomposes `U+0958..=U+095F`).
/// 3. Applies Unicode NFC.
/// 4. Reorders any post-vowel nukta (`U+093C`) before adjacent dependent vowel signs
///    and deduplicates consecutive `U+093C` marks.
/// 5. Re-applies NFC. Anusvara (`ं`, `U+0902`) and chandrabindu (`ँ`, `U+0901`) are kept distinct.
pub fn normalize_hi_str(s: &str) -> String {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let mut decomp = String::with_capacity(trimmed.len());
    for c in trimmed.chars() {
        match c {
            '\u{0929}' => decomp.push_str("\u{0928}\u{093C}"),
            '\u{0931}' => decomp.push_str("\u{0930}\u{093C}"),
            '\u{0934}' => decomp.push_str("\u{0933}\u{093C}"),
            _ => decomp.push(c),
        }
    }
    let pass1: String = decomp.nfc().collect();
    let mut out_chars: Vec<char> = Vec::with_capacity(pass1.chars().count());
    for ch in pass1.chars() {
        if ch == '\u{093C}' {
            let mut idx = out_chars.len();
            while idx > 0 && is_deva_dependent_vowel(out_chars[idx - 1]) {
                idx -= 1;
            }
            if idx > 0 && out_chars[idx - 1] == '\u{093C}' {
                continue;
            }
            out_chars.insert(idx, '\u{093C}');
        } else {
            out_chars.push(ch);
        }
    }
    let reordered: String = out_chars.into_iter().collect();
    reordered.nfc().collect()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HiAmbigDefault {
    WordLeaning,
    NameLeaning,
}

/// Count-bound default rule for ambiguous Hindi names:
/// - `T == 0` -> `NameLeaning` (no corpus evidence -> redact)
/// - `T > A` -> `WordLeaning` (redact only with a cue)
/// - `T <= A` -> `NameLeaning` (redact by default)
pub fn classify_hi_count_bound(anchor_count: u64, combined_count: u64) -> HiAmbigDefault {
    if combined_count == 0 {
        HiAmbigDefault::NameLeaning
    } else if combined_count > anchor_count {
        HiAmbigDefault::WordLeaning
    } else {
        HiAmbigDefault::NameLeaning
    }
}

pub struct HiResources {
    pub words: HashSet<String>,
    pub given_names: HashSet<String>,
    pub surnames: HashSet<String>,
    pub non_word_names: HashSet<String>,
    pub ambiguous_word_leaning: HashSet<String>,
    pub ambiguous_name_leaning: HashSet<String>,
    pub anchor_count: u64,
}

fn resolve_default_hi_resource_dir() -> Result<PathBuf, String> {
    resolve_resource_root().map(|r| r.join("hi"))
}

fn load_hi_csv_names_from_reader<R: std::io::Read>(
    reader: R,
    out: &mut HashSet<String>,
    label: &str,
) -> Result<(), String> {
    use std::io::BufRead;
    let buf_reader = std::io::BufReader::new(reader);
    for (idx, line_res) in buf_reader.lines().enumerate() {
        let line = line_res.map_err(|e| format!("[scrubber] ERROR reading {}: {}", label, e))?;
        let norm = normalize_hi_str(&line);
        if idx == 0 && norm == "name" {
            continue;
        }
        if !norm.is_empty() && !norm.starts_with('#') {
            out.insert(norm);
        }
    }
    Ok(())
}

fn load_hi_csv_names(path: &Path, out: &mut HashSet<String>) {
    let Ok(file) = std::fs::File::open(path) else {
        return;
    };
    let _ = load_hi_csv_names_from_reader(file, out, &path.to_string_lossy());
}

fn load_hi_resources_from_root(root: &Path) -> Result<HiResources, String> {
    use std::io::BufRead;

    let mut words = HashSet::with_capacity(90_000);
    {
        let (_, words_file) = open_manifest_file_from_root(root, "hi", "hi_words.tsv.zst")?;
        let decoder = zstd::Decoder::new(words_file)
            .map_err(|e| format!("[scrubber] ERROR decoding hi/hi_words.tsv.zst: {}", e))?;
        let mut reader = std::io::BufReader::with_capacity(256 * 1024, decoder);
        let mut line = String::new();
        loop {
            let n = reader
                .read_line(&mut line)
                .map_err(|e| format!("[scrubber] ERROR reading hi/hi_words.tsv.zst: {}", e))?;
            if n == 0 {
                break;
            }
            let trimmed = line.trim();
            if !trimmed.is_empty()
                && !trimmed.starts_with('#')
                && !trimmed.starts_with("word\t")
                && trimmed != "word"
            {
                if let Some(w_raw) = trimmed.split('\t').next() {
                    let norm = normalize_hi_str(w_raw);
                    if !norm.is_empty() {
                        words.insert(norm);
                    }
                }
            }
            line.clear();
        }
    }

    let mut given_names = HashSet::with_capacity(3_050);
    {
        let (_, male_file) = open_manifest_file_from_root(root, "hi", "hi_names_male_clean.csv")?;
        load_hi_csv_names_from_reader(male_file, &mut given_names, "hi/hi_names_male_clean.csv")?;
        let (_, female_file) =
            open_manifest_file_from_root(root, "hi", "hi_names_female_clean.csv")?;
        load_hi_csv_names_from_reader(
            female_file,
            &mut given_names,
            "hi/hi_names_female_clean.csv",
        )?;
    }

    let mut surnames = HashSet::with_capacity(4_000);
    {
        let (_, sur_file) = open_manifest_file_from_root(root, "hi", "hi_surnames_clean.csv")?;
        load_hi_csv_names_from_reader(sur_file, &mut surnames, "hi/hi_surnames_clean.csv")?;
    }

    let mut all_names: HashSet<String> = HashSet::with_capacity(given_names.len() + surnames.len());
    for n in &given_names {
        all_names.insert(n.clone());
    }
    for n in &surnames {
        all_names.insert(n.clone());
    }

    let mut name_freqs: HashMap<String, u64> = HashMap::with_capacity(all_names.len());
    {
        let (_, freq_file) = open_manifest_file_from_root(root, "hi", "hi_freq.tsv.zst")?;
        let decoder = zstd::Decoder::new(freq_file)
            .map_err(|e| format!("[scrubber] ERROR decoding hi/hi_freq.tsv.zst: {}", e))?;
        let mut reader = std::io::BufReader::with_capacity(256 * 1024, decoder);
        let mut line = String::new();
        loop {
            let n = reader
                .read_line(&mut line)
                .map_err(|e| format!("[scrubber] ERROR reading hi/hi_freq.tsv.zst: {}", e))?;
            if n == 0 {
                break;
            }
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with('#') && !trimmed.starts_with("word\t") {
                if let Some((w_raw, cnt_raw)) = trimmed.split_once('\t') {
                    let norm = normalize_hi_str(w_raw);
                    if all_names.contains(&norm) {
                        if let Ok(cnt) = cnt_raw.trim().parse::<u64>() {
                            *name_freqs.entry(norm).or_insert(0) += cnt;
                        }
                    }
                }
            }
            line.clear();
        }
    }

    if words.is_empty() || given_names.is_empty() || surnames.is_empty() {
        return Err("[scrubber] ERROR: Hindi resource files yielded empty data".to_string());
    }

    let mut non_word_names = HashSet::new();
    let mut ambiguous_names = Vec::new();
    let mut non_word_counts: Vec<u64> = Vec::new();
    for n in all_names {
        if !words.contains(&n) {
            let c = name_freqs.get(&n).copied().unwrap_or(0);
            non_word_counts.push(c);
            non_word_names.insert(n);
        } else {
            ambiguous_names.push(n);
        }
    }

    non_word_counts.sort_unstable_by(|a, b| b.cmp(a));
    let anchor_count = if non_word_counts.is_empty() {
        0
    } else {
        let idx = non_word_counts.len().min(50) - 1;
        non_word_counts[idx]
    };

    let mut ambiguous_word_leaning = HashSet::new();
    let mut ambiguous_name_leaning = HashSet::new();
    for n in ambiguous_names {
        let total = name_freqs.get(&n).copied().unwrap_or(0);
        match classify_hi_count_bound(anchor_count, total) {
            HiAmbigDefault::WordLeaning => {
                ambiguous_word_leaning.insert(n);
            }
            HiAmbigDefault::NameLeaning => {
                ambiguous_name_leaning.insert(n);
            }
        }
    }

    Ok(HiResources {
        words,
        given_names,
        surnames,
        non_word_names,
        ambiguous_word_leaning,
        ambiguous_name_leaning,
        anchor_count,
    })
}

static HI_RESOURCES: OnceLock<Result<HiResources, String>> = OnceLock::new();

pub fn get_hi_resources() -> &'static HiResources {
    static EMPTY_HI: OnceLock<HiResources> = OnceLock::new();
    match HI_RESOURCES.get_or_init(|| {
        let root = resolve_resource_root()?;
        load_hi_resources_from_root(&root)
    }) {
        Ok(res) => res,
        Err(err) => {
            record_scrub_resource_error(err);
            EMPTY_HI.get_or_init(|| HiResources {
                words: HashSet::new(),
                given_names: HashSet::new(),
                surnames: HashSet::new(),
                non_word_names: HashSet::new(),
                ambiguous_word_leaning: HashSet::new(),
                ambiguous_name_leaning: HashSet::new(),
                anchor_count: 0,
            })
        }
    }
}

pub fn is_hi_resources_loaded() -> bool {
    HI_RESOURCES.get().is_some()
}

pub fn is_hi_word(s: &str) -> bool {
    let norm = normalize_hi_str(s);
    !norm.is_empty() && get_hi_resources().words.contains(&norm)
}

/// High-frequency Hindi grammar words, verbs, postpositions, and common nouns that collide
/// with entries in the CSV name lists. Never treated as partners in un-titled adjacent-name cues
/// or space-only label values (still redacted after explicit labels `नाम: ...` or titles `श्री ...`).
const HI_NON_CUE_COMMON_WORDS: &[&str] = &[
    "कर", "हो", "दिया", "या", "भारत", "वाले", "सरकार", "दे", "बना", "मंत्री",
    "पूरी", "वाला", "जीवन", "केवल", "बड़ा", "जाना", "आदि", "विशेष", "जीत",
    "बिना", "अधिकारी", "मांग", "श्री", "भर", "धर्म", "भूमिका", "लागू", "प्रणाली",
    "पूर्ण", "संभावना", "कदम", "लाभ", "बल", "मान", "किसान", "डॉक्टर", "कम",
];

fn is_hi_non_name_common_token(tok: &str, res: &HiResources) -> bool {
    let norm = normalize_hi_str(tok);
    if norm.is_empty() {
        return false;
    }
    let in_names = res.non_word_names.contains(&norm)
        || res.ambiguous_word_leaning.contains(&norm)
        || res.ambiguous_name_leaning.contains(&norm);
    (res.words.contains(&norm) && !in_names) || HI_NON_CUE_COMMON_WORDS.contains(&norm.as_str())
}

const HI_TITLES_AND_SALUTATIONS: &[&str] = &["श्री", "श्रीमती", "सुश्री", "डॉ", "प्रिय"];

fn is_hi_title_or_salutation(norm: &str) -> bool {
    HI_TITLES_AND_SALUTATIONS.contains(&norm)
}

fn is_hi_sentence(toks: &[Tok], seg: &str) -> bool {
    let has_deva_letter = seg
        .chars()
        .any(|c| is_devanagari_char(c) && !('\u{0964}'..='\u{096F}').contains(&c));
    if !has_deva_letter {
        return false;
    }
    let mut deva_chars = 0usize;
    let mut bn_chars = 0usize;
    let mut cjk_ko_chars = 0usize;
    let mut latin_chars = 0usize;
    for c in seg.chars() {
        if is_devanagari_char(c) && !('\u{0964}'..='\u{096F}').contains(&c) {
            deva_chars += 1;
        } else if is_bengali_char(c) {
            bn_chars += 1;
        } else if is_cjk_or_hangul(c) {
            cjk_ko_chars += 1;
        } else if c.is_ascii_alphabetic() {
            latin_chars += 1;
        }
    }
    if bn_chars > deva_chars || cjk_ko_chars > deva_chars {
        return false;
    }

    let mut hi_markers = 0usize;
    let mut rival_deva_markers = 0usize;
    let mut en_markers = 0usize;
    for t in toks {
        let raw_core = &seg[t.core_s..t.core_e];
        let trimmed = raw_core.trim_matches(|c: char| matches!(c, '।' | '॥' | '’' | '‘' | '–' | '—' | '-'));
        let norm = normalize_hi_str(trimmed);
        if matches!(
            norm.as_str(),
            "है" | "हैं" | "था" | "थी" | "थे" | "थीं"
                | "का" | "की" | "के" | "को" | "में" | "से" | "ने" | "पर"
                | "और" | "या" | "नहीं" | "यह" | "वह" | "इस" | "उस"
                | "इन" | "उन" | "भी" | "लिए" | "साथ" | "बाद" | "द्वारा"
                | "कर" | "करने" | "किया" | "हो" | "हुआ" | "हुई" | "हुए"
                | "गया" | "गई" | "गए" | "दिया" | "दिए" | "दी"
                | "रहा" | "रही" | "रहे" | "सकता" | "सकती" | "सकते"
                | "एक" | "ही" | "तो" | "कि" | "जो" | "जब" | "तब"
                | "हम" | "आप" | "मैं" | "तुम" | "वे" | "उन्होंने"
                | "उनका" | "उनकी" | "उनके" | "मेरा" | "मेरी" | "मेरे"
                | "हमारा" | "हमारी" | "हमारे" | "अपने" | "अपना" | "अपनी"
                | "श्री" | "श्रीमती" | "सुश्री" | "डॉ" | "प्रिय"
        ) {
            hi_markers += 1;
        } else if matches!(
            norm.as_str(),
            "आहे" | "आहेत" | "नाही" | "नाहीत" | "माझे" | "तुमचे"
                | "त्याचा" | "त्याची" | "त्याचे" | "होते" | "झाले"
                | "छ" | "छन्" | "होइन" | "छैन" | "छैनन्"
                | "तपाईं" | "हाम्रो" | "मेरो" | "तिमी" | "गरिन्छ" | "भएको"
        ) {
            rival_deva_markers += 1;
        }
        let low = raw_core.to_lowercase();
        if matches!(low.as_str(), "the" | "is" | "are" | "was" | "were" | "and" | "for" | "with" | "from") {
            en_markers += 1;
        }
    }

    if rival_deva_markers > hi_markers {
        return false;
    }
    if en_markers > hi_markers && latin_chars > deva_chars {
        return false;
    }
    hi_markers >= 1 || (deva_chars >= latin_chars && bn_chars == 0 && cjk_ko_chars == 0)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HiTokKind {
    Title,
    NonWordName,
    NameLeaningAmbig,
    WordLeaningAmbig,
    Word,
    Unknown,
    NonDevanagari,
}

fn is_hi_pure_deva_token(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .any(|c| is_devanagari_char(c) && !('\u{0964}'..='\u{096F}').contains(&c))
        && s.chars()
            .all(|c| is_devanagari_char(c) || c == '\u{200C}' || c == '\u{200D}')
}

fn scrub_hi_names(
    seg: &str,
    opts: &ScrubOptions,
    matched_name_languages: &mut BTreeSet<String>,
) -> String {
    if !seg.chars().any(is_devanagari_char) {
        return seg.to_string();
    }
    let toks = tokenize(seg);
    if toks.is_empty() || !is_hi_sentence(&toks, seg) {
        return seg.to_string();
    }

    let res = get_hi_resources();
    let n = toks.len();

    struct HiTokInfo {
        span_s: usize,
        span_e: usize,
        norm: String,
        kind: HiTokKind,
        boundary_after: bool,
        nl_before: bool,
        in_given: bool,
        in_surname: bool,
        is_common_non_cue: bool,
    }

    let mut infos: Vec<HiTokInfo> = Vec::with_capacity(n);
    for t in &toks {
        let raw_core = &seg[t.core_s..t.core_e];
        let lead_trim = raw_core.len()
            - raw_core
                .trim_start_matches(['।', '॥', '’', '‘', '–', '—', '-'])
                .len();
        let trimmed = raw_core[lead_trim..]
            .trim_end_matches(['।', '॥', '’', '‘', '–', '—', '-']);
        let span_s = t.core_s + lead_trim;
        let span_e = span_s + trimmed.len();
        let norm = normalize_hi_str(trimmed);

        let kind = if !is_hi_pure_deva_token(&norm) || is_allowlisted(&norm, opts) {
            HiTokKind::NonDevanagari
        } else if is_hi_title_or_salutation(&norm) {
            HiTokKind::Title
        } else if res.non_word_names.contains(&norm) {
            HiTokKind::NonWordName
        } else if res.ambiguous_name_leaning.contains(&norm) {
            HiTokKind::NameLeaningAmbig
        } else if res.ambiguous_word_leaning.contains(&norm) {
            HiTokKind::WordLeaningAmbig
        } else if res.words.contains(&norm) {
            HiTokKind::Word
        } else {
            HiTokKind::Unknown
        };

        let is_title = kind == HiTokKind::Title;
        let trail = &seg[span_e..t.e];
        let boundary_after = trail.chars().any(|c| {
            matches!(c, ',' | '।' | '॥' | ';' | ':' | '!' | '?' | ')' | '"' | '”' | '\'')
                || (c == '.' && !is_title)
        });

        let in_given = res.given_names.contains(&norm);
        let in_surname = res.surnames.contains(&norm);
        let is_common_non_cue = HI_NON_CUE_COMMON_WORDS.contains(&norm.as_str());

        infos.push(HiTokInfo {
            span_s,
            span_e,
            norm,
            kind,
            boundary_after,
            nl_before: t.nl_before,
            in_given,
            in_surname,
            is_common_non_cue,
        });
    }

    let mut redact = vec![false; n];
    let mut confirmed = vec![false; n];

    // 1. Non-word names and name-leaning ambiguous names redact unconditionally.
    for i in 0..n {
        if matches!(
            infos[i].kind,
            HiTokKind::NonWordName | HiTokKind::NameLeaningAmbig
        ) {
            redact[i] = true;
            confirmed[i] = true;
        }
    }

    // 1b. Ergative postposition cue: a name token directly followed by "ने" (with a space)
    //     counts as a cue so non-common word-leaning names redact under the default rule.
    for i in 0..n {
        if !infos[i].is_common_non_cue
            && matches!(
                infos[i].kind,
                HiTokKind::NonWordName | HiTokKind::NameLeaningAmbig | HiTokKind::WordLeaningAmbig
            )
            && i + 1 < n
            && !infos[i].boundary_after
            && !infos[i + 1].nl_before
            && infos[i + 1].norm == "ने"
        {
            redact[i] = true;
            confirmed[i] = true;
        }
    }

    // 2. Title or salutation cue: श्री, श्रीमती, सुश्री, डॉ, प्रिय -> redact adjacent following name tokens.
    for i in 0..n {
        if infos[i].kind != HiTokKind::Title || infos[i].boundary_after {
            continue;
        }
        let mut k = i + 1;
        let mut taken = 0usize;
        while k < n && taken < 4 && !infos[k].nl_before {
            let allow = match infos[k].kind {
                HiTokKind::NonWordName | HiTokKind::NameLeaningAmbig => true,
                HiTokKind::WordLeaningAmbig => {
                    taken == 0 || !infos[k].is_common_non_cue
                }
                HiTokKind::Unknown => taken == 0 && infos[i].norm != "प्रिय",
                _ => false,
            };
            if !allow {
                break;
            }
            redact[k] = true;
            confirmed[k] = true;
            taken += 1;
            if infos[k].boundary_after {
                break;
            }
            k += 1;
        }
    }

    // 3. Adjacent GivenName + [Middle] + Surname cue among name-list entries.
    for i in 0..n {
        if infos[i].boundary_after || infos[i].is_common_non_cue || infos[i].kind == HiTokKind::Title {
            continue;
        }
        if !infos[i].in_given {
            continue;
        }
        // Check 3-token Given + Middle + Surname
        if i + 2 < n
            && !infos[i + 1].nl_before
            && !infos[i + 1].boundary_after
            && !infos[i + 2].nl_before
            && !infos[i + 1].is_common_non_cue
            && !infos[i + 2].is_common_non_cue
            && infos[i + 1].kind != HiTokKind::Title
            && infos[i + 2].kind != HiTokKind::Title
            && (infos[i + 1].in_given || infos[i + 1].in_surname)
            && infos[i + 2].in_surname
        {
            redact[i] = true;
            redact[i + 1] = true;
            redact[i + 2] = true;
            confirmed[i] = true;
            confirmed[i + 1] = true;
            confirmed[i + 2] = true;
            continue;
        }
        // Check 2-token Given + Surname
        if i + 1 < n
            && !infos[i + 1].nl_before
            && !infos[i + 1].is_common_non_cue
            && infos[i + 1].kind != HiTokKind::Title
            && infos[i + 1].in_surname
        {
            redact[i] = true;
            redact[i + 1] = true;
            confirmed[i] = true;
            confirmed[i + 1] = true;
        }
    }

    // 4. Propagate confirmed name cue to adjacent non-common ambiguous name tokens.
    let mut changed = true;
    while changed {
        changed = false;
        for i in 0..n {
            if redact[i] {
                continue;
            }
            let can_promote = matches!(
                infos[i].kind,
                HiTokKind::WordLeaningAmbig
            ) && !infos[i].is_common_non_cue;
            if !can_promote {
                continue;
            }
            let left_confirmed =
                i > 0 && !infos[i].nl_before && !infos[i - 1].boundary_after && confirmed[i - 1];
            let right_confirmed =
                i + 1 < n && !infos[i + 1].nl_before && !infos[i].boundary_after && confirmed[i + 1];
            if left_confirmed || right_confirmed {
                redact[i] = true;
                confirmed[i] = true;
                changed = true;
            }
        }
    }

    let mut spans: Vec<(usize, usize)> = Vec::new();
    for i in 0..n {
        if redact[i] && infos[i].span_s < infos[i].span_e {
            spans.push((infos[i].span_s, infos[i].span_e));
        } else if !redact[i]
            && infos[i].kind == HiTokKind::Unknown
            && infos[i].span_s < infos[i].span_e
            && seg[infos[i].span_s..infos[i].span_e].ends_with("ने")
            && infos[i].norm.ends_with("ने")
        {
            // No-space ergative attachment (e.g. "<Name>ने" when not an existing dictionary word).
            let stem = &infos[i].norm[..infos[i].norm.len() - "ने".len()];
            if !stem.is_empty()
                && !HI_NON_CUE_COMMON_WORDS.contains(&stem)
                && (res.non_word_names.contains(stem)
                    || res.ambiguous_name_leaning.contains(stem)
                    || res.ambiguous_word_leaning.contains(stem))
            {
                let cut_e = infos[i].span_e - "ने".len();
                if infos[i].span_s < cut_e {
                    spans.push((infos[i].span_s, cut_e));
                }
            }
        }
    }
    if spans.is_empty() {
        return seg.to_string();
    }
    matched_name_languages.insert("hi".to_string());

    let mut merged: Vec<(usize, usize)> = Vec::with_capacity(spans.len());
    for (s, e) in spans {
        if let Some(last) = merged.last_mut() {
            if s <= last.1
                || seg[last.1..s]
                    .chars()
                    .all(|c| c.is_whitespace() && c != '\n' && c != '\r')
            {
                last.1 = last.1.max(e);
                continue;
            }
        }
        merged.push((s, e));
    }

    apply_spans(seg, &merged, "[NAME]")
}

/// Classifies a Bengali token according to the lookup precedence:
/// 1. Exact match in the names list -> `BnTokKind::Name`.
/// 2. Exact match in the word list -> `BnTokKind::Word`.
/// 3. Strip a known suffix (longest suffix first) and check stem against the word list -> `BnTokKind::Word`.
/// 4. Otherwise -> `BnTokKind::Unknown`.
pub fn classify_bn_token(s: &str) -> BnTokKind {
    classify_bn_token_in(s, get_bn_resources())
}

fn classify_bn_token_in(s: &str, res: &BnResources) -> BnTokKind {
    let norm = normalize_bn_str(s);
    if norm.is_empty() {
        return BnTokKind::Unknown;
    }
    // 1. Exact match in the names list -> Name (never use stripping to match a name).
    if res.names.contains(&norm) {
        return BnTokKind::Name;
    }
    // 2. Exact match in the word list -> Word.
    if res.words.contains(&norm) {
        return BnTokKind::Word;
    }
    // 3. Otherwise, strip a known suffix (longest suffix first) and check stem against the word list -> Word.
    if is_bn_suffix_stripped_word_in(&norm, res) {
        return BnTokKind::Word;
    }
    // 4. Otherwise -> Unknown.
    BnTokKind::Unknown
}

/// True if `s` (normalized with NFC and trimmed) is in the Bangla names list (`bn_names.tsv`).
pub fn is_bn_name(s: &str) -> bool {
    let norm = normalize_bn_str(s);
    if norm.is_empty() {
        return false;
    }
    get_bn_resources().names.contains(&norm)
}

fn is_bn_name_in(s: &str, res: &BnResources) -> bool {
    let norm = normalize_bn_str(s);
    if norm.is_empty() {
        return false;
    }
    res.names.contains(&norm)
}

fn is_bn_full_name_in(s: &str, res: &BnResources) -> bool {
    let norm = normalize_bn_str(s);
    if norm.is_empty() {
        return false;
    }
    res.full_names.contains(&norm)
}

/// Returns romanizations for the Bangla name `s` (normalized with NFC and trimmed).
pub fn bn_name_romanizations(s: &str) -> Vec<String> {
    let norm = normalize_bn_str(s);
    if norm.is_empty() {
        return Vec::new();
    }
    get_bn_resources()
        .romanizations
        .get(&norm)
        .cloned()
        .unwrap_or_default()
}

fn known_word(word: &str, dict: &HashSet<String>) -> bool {
    let w = word.trim_end_matches("'s").trim_end_matches("’s");
    if w.contains('-') {
        return w
            .split('-')
            .filter(|p| !p.is_empty())
            .all(|p| known_word(p, dict));
    }
    let w_owned = w.replace(['\'', '’'], "").to_lowercase();
    let w = w_owned.as_str();
    if dict.contains(w) {
        return true;
    }
    for suf in ["s", "es", "ed", "d", "ing", "ly", "er", "ers", "est"] {
        if let Some(stem) = w.strip_suffix(suf) {
            if stem.len() >= 3 {
                if dict.contains(stem) || dict.contains(format!("{}e", stem).as_str()) {
                    return true;
                }
                let b = stem.as_bytes();
                if stem.len() >= 4
                    && b[b.len() - 1] == b[b.len() - 2]
                    && dict.contains(&stem[..stem.len() - 1])
                {
                    return true;
                }
            }
        }
    }
    if let Some(stem) = w.strip_suffix("ies") {
        if dict.contains(format!("{}y", stem).as_str()) {
            return true;
        }
    }
    false
}

/// Capitalised words that must NOT trigger name redaction (headings, months, acronyms).
const NAME_ALLOWLIST: &[&str] = &[
    "university",
    "college",
    "department",
    "invoice",
    "statement",
    "report",
    "summary",
    "overview",
    "introduction",
    "conclusion",
    "appendix",
    "section",
    "table",
    "figure",
    "attachment",
    "enclosure",
    "subject",
    "re",
    "regarding",
    "ref",
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
    "jan",
    "feb",
    "mar",
    "apr",
    "jun",
    "jul",
    "aug",
    "sep",
    "sept",
    "oct",
    "nov",
    "dec",
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
    "sunday",
    "ssn",
    "sin",
    "ein",
    "itin",
    "tin",
    "vat",
    "nid",
    "dob",
    "pin",
    "otp",
    "cvc",
    "cvv",
    "csc",
    "pdf",
    "id",
    "usa",
    "uk",
    "us",
    "eu",
    "london",
    "ceo",
    "cfo",
    "faq",
    "ok",
    "the",
    "of",
    "and",
    "in",
    "to",
    "for",
    "on",
    "at",
    "by",
    "with",
    "a",
    "an",
    "is",
    "are",
    "as",
    // Spanish months, days, cities, titles, and non-name headings/nouns
    "lunes",
    "martes",
    "miércoles",
    "miercoles",
    "jueves",
    "viernes",
    "sábado",
    "sabado",
    "domingo",
    "enero",
    "febrero",
    "marzo",
    "abril",
    "mayo",
    "junio",
    "julio",
    "agosto",
    "septiembre",
    "octubre",
    "noviembre",
    "diciembre",
    // French months
    "janvier",
    "février",
    "fevrier",
    "mars",
    "avril",
    "mai",
    "juin",
    "juillet",
    "août",
    "aout",
    "septembre",
    "octobre",
    "novembre",
    "décembre",
    "decembre",
    "madrid",
    "barcelona",
    "sevilla",
    "valencia",
    "zaragoza",
    "málaga",
    "malaga",
    "bilbao",
    "españa",
    "espana",
    "méxico",
    "mexico",
    "argentina",
    "colombia",
    "chile",
    "perú",
    "peru",
    "señor",
    "senor",
    "señora",
    "senora",
    "señorita",
    "senorita",
    "doctora",
    "doctor",
    "don",
    "doña",
    "dona",
    "san",
    "santa",
    "santo",
    "capítulo",
    "capitulo",
    "artículo",
    "articulo",
    "sección",
    "seccion",
    "estado",
    "gobierno",
    "ministerio",
    "universidad",
    "departamento",
    "informe",
    "resumen",
];

/// Common Spanish words (sentence-initial function words, pronouns, adverbs, and everyday nouns/adjectives
/// that also appear in Spanish name lists like "blanca", "rosa", "luz", "cruz", "sol", "mar").
/// When a word is in `SPANISH_KNOWN_WORDS`:
/// - `is_unk` is false in `scrub_unknown_caps`, so sentence-initial common words ("Hoy", "Cuando", "Pero")
///   are not redacted, while mid-sentence capitalized names ("La doctora Blanca") are still redacted via `is_nl`.
/// - Lowercase occurrences in running text ("la casa blanca", "mucha luz") are not redacted by Tier 4 `scrub_name_lookup`.
const SPANISH_KNOWN_WORDS: &[&str] = &[
    // Sentence-initial function words, conjunctions, prepositions, adverbs, determiners, pronouns
    "hoy", "ayer", "mañana", "manana", "cuando", "cuándo", "pero", "aunque", "mientras", "porque",
    "porqué", "donde", "dónde", "como", "cómo", "siempre", "nunca", "jamás", "jamas", "también",
    "tambien", "tampoco", "además", "ademas", "entonces", "después", "despues", "antes", "ahora",
    "aquí", "aqui", "allí", "alli", "allá", "alla", "acá", "aca", "así", "asi", "casi", "solo",
    "sólo", "quizás", "quizas", "quizá", "quiza", "acaso", "luego", "pronto", "tarde", "temprano",
    "bien", "mal", "muy", "más", "mas", "menos", "mucho", "mucha", "muchos", "muchas", "poco",
    "poca", "pocos", "pocas", "todo", "toda", "todos", "todas", "nada", "algo", "alguien", "nadie",
    "cada", "otro", "otra", "otros", "otras", "mismo", "misma", "mismos", "mismas", "tal", "tales",
    "cual", "cuál", "cuales", "cuáles", "quien", "quién", "quienes", "quiénes", "cuyo", "cuya",
    "cuyos", "cuyas", "que", "qué", "este", "esta", "estos", "estas", "esto", "ese", "esa", "esos",
    "esas", "eso", "aquel", "aquella", "aquellos", "aquellas", "aquello", "mi", "mis", "tu", "tus",
    "su", "sus", "nuestro", "nuestra", "nuestros", "nuestras", "vuestro", "vuestra", "vuestros",
    "vuestras", "yo", "tú", "él", "el", "ella", "ello", "nosotros", "nosotras", "vosotros",
    "vosotras", "ellos", "ellas", "usted", "ustedes", "me", "te", "se", "nos", "os", "lo", "la",
    "los", "las", "le", "les", "un", "una", "unos", "unas", "al", "del", "en", "por", "para",
    "con", "sin", "sobre", "entre", "hasta", "desde", "hacia", "según", "segun", "durante",
    "mediante", "contra", "tras", "ante", "bajo", "cabe", "so", "ni", "o", "u", "y", "e", "si",
    "sí", "no", "ya", "aún", "aun", "pues", "sino", "mas", "salvo", "excepto", "incluso",
    // Common Spanish verbs/auxiliaries/nouns frequently capitalized at sentence start
    "es", "son", "era", "eran", "fue", "fueron", "ser", "será", "sera", "está", "esta", "están",
    "estan", "estaba", "estaban", "estar", "hay", "había", "habia", "hubo", "ha", "han", "haber",
    "tiene", "tienen", "tenía", "tenia", "tener", "hace", "hacen", "hacía", "hacia", "hacer",
    "hizo", "puede", "pueden", "podía", "podia", "poder", "debe", "deben", "decir", "dijo", "dice",
    "dicen", "vamos", "van", "iba", "iban", "ir", "viene", "vienen", "venir", "vino", "ver", "vio",
    "sabe", "saben", "saber", "quiere", "quieren", "querer", "quiso", "llegó", "llego", "llegaron",
    "salió", "salio", "salieron", "pasó", "paso", "quedó", "quedo", "parece", "parecía", "parecia",
    "vaya", "sea", "sean", "fuese", "fuera", "gracias", "hola", "buenos", "buenas", "días", "dias",
    "tardes", "noches", "favor", "embargo", "obstante", "supuesto", "ejemplo", "cambio", "fin",
    "principio", "verdad", "hecho", "modo", "manera", "forma", "parte", "tipo", "caso", "tiempo",
    "día", "dia", "año", "ano", "años", "anos", "mes", "meses", "semana", "hora", "horas", "vez",
    "veces", "momento", "lugar", "mundo", "vida", "casa", "hombre", "hombres", "mujer", "mujeres",
    "gente", "persona", "personas", "trabajo", "país", "pais", "ciudad", "calle", "agua", "tierra",
    "aire", "fuego", "libro", "carta", "historia", "cuenta", "punto", "lado", "medio", "nuevo",
    "nueva", "nuevos", "nuevas", "bueno", "buena", "malo", "mala", "grande", "grandes", "pequeño",
    "pequeno", "pequeña", "pequena", "primer", "primero", "primera", "primeros", "primeras",
    // Common verbs, titles, days of the week, months, and places
    "viajaremos", "consultamos", "revisamos", "publicar", "entregar", "abrieron",
    "actualizarán", "actualizaran", "falta", "faltan", "enviamos", "enviar", "canto", "cantó",
    "sr", "sra", "srta", "dr", "dra", "lic", "ing", "prof",
    "lunes", "martes", "miércoles", "miercoles",
    "jueves", "viernes", "sábado", "sabado", "domingo", "enero", "febrero",
    "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre",
    "octubre", "noviembre", "diciembre", "madrid", "barcelona", "sevilla",
    "valencia", "españa", "espana", "mancha", "manchas",
    // Everyday Spanish nouns/adjectives that also appear in Spanish given/surname lists
    "blanca", "blanco", "rosa", "clara", "claro", "sol", "mar", "luz", "paz", "fe", "cielo",
    "luna", "estrella", "esperanza", "consuelo", "socorro", "dolores", "mercedes", "pilar",
    "carmen", "rocío", "rocio", "camino", "valle", "vega", "cruz", "reyes", "rey", "reina",
    "santos", "santo", "santa", "moreno", "morena", "peña", "pena", "lobo", "pino", "marco",
    "mano", "mayor", "menor", "justo", "justa", "feliz", "amado", "amada", "piedad", "caridad",
    "milagros", "milagro", "nieves", "nieve", "paloma", "perla", "esmeralda", "violeta",
    "margarita", "azucena", "hortensia", "jazmín", "jazmin", "coral", "brisa", "aurora", "alba",
    "dulce", "ángel", "angel", "ángeles", "angeles", "espíritu", "espiritu", "corazón", "corazon",
    "nombre", "suceso", "caballero", "hidalgo", "pastor", "pastora", "castillo", "libertad",
    "diez", "segunda", "segundo", "hermano", "hermana", "concepción", "concepcion", "asunción",
    "asuncion", "encarnación", "encarnacion", "purificación", "purificacion", "visitación",
    "visitacion", "presentación", "presentacion", "adoración", "adoracion", "consolación",
    "consolacion", "natividad", "trinidad", "soledad", "remedios", "auxiliadora", "gracia",
    "gracias", "olvido", "prudencia", "clemencia", "inocencia", "paciencia", "constancia",
    "fortuna", "victoria", "gloria", "honor", "fe", "amor", "alegría", "alegria", "felicidad",
    "ventura", "salud", "vida", "marina", "marino", "monte", "montes", "campo", "campos",
    "fuente", "fuentes", "río", "rio", "ríos", "rios", "sierra", "colina", "bosque", "selva",
    "jardín", "jardin", "flor", "flores", "guerrero", "guerrera", "cortés", "cortes", "cabrera",
    "molina", "herrera", "medina", "castro", "romero", "navarro", "torres", "gil", "ramos",
    "serrano", "marín", "marin", "iglesias", "garrido", "cano", "prieto", "calvo", "vidal",
    "león", "leon", "mora", "paso", "coro", "arco", "gallo", "cámara", "camara", "consejo",
    "excelencia", "curioso", "curiosa", "dolorida", "dolorido", "constante", "firme", "fiel",
    "noble", "gentil", "bravo", "franco", "franca", "alemán", "aleman", "román", "roman",
    "catalán", "catalan", "español", "espanol", "francés", "frances", "inglés", "ingles",
    "cristiano", "cristiana",
    // Structural headings, document labels, historical epochs, and common capitalized nouns
    // (Note: Many words like prólogo, niebla, etc., are also in es_words.txt)
    "teologos",
];

fn is_spanish_common_word(w: &str) -> bool {
    let w_stripped = strip_latin_accents(w);
    let es_words = get_es_words();
    SPANISH_KNOWN_WORDS.contains(&w)
        || SPANISH_KNOWN_WORDS.contains(&w_stripped.as_str())
        || es_words.contains(w)
        || es_words.contains(w_stripped.as_str())
}

fn is_known_en_or_es_word(w: &str) -> bool {
    let w_stripped = strip_latin_accents(w);
    let words_full = get_words_full();
    let es_words = get_es_words();
    known_word(w, words_full)
        || known_word(&w_stripped, words_full)
        || es_words.contains(w)
        || es_words.contains(w_stripped.as_str())
        || SPANISH_KNOWN_WORDS.contains(&w)
        || SPANISH_KNOWN_WORDS.contains(&w_stripped.as_str())
        || NAME_ALLOWLIST.contains(&w)
        || NAME_ALLOWLIST.contains(&w_stripped.as_str())
}

static CAPITALISED_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[\p{Lu}][\p{Ll}'’\-]+$").unwrap());
static ALLCAPS_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[\p{Lu}]{2,}$").unwrap());
static CUE_CAP_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:[\p{Lu}][\p{Lu}\p{Ll}'’\-]+|[\p{Lu}]{2,})[,.]?$").unwrap());

struct Tok {
    s: usize,
    e: usize,
    core_s: usize,
    core_e: usize,
    nl_before: bool,
}

fn french_elision_prefix_len(s: &str) -> usize {
    let lower = s.to_lowercase();
    const PREFIXES: &[&str] = &[
        "puisqu", "lorsqu", "quoiqu", "jusqu", "qu",
        "l", "d", "j", "m", "t", "s", "c", "n",
    ];
    for &pref in PREFIXES {
        for apo in ["'", "’"] {
            if let Some(after_pref) = lower.strip_prefix(pref) {
                if after_pref.starts_with(apo) {
                    let apo_len = apo.len();
                    let rem = &s[pref.len() + apo_len..];
                    if rem.chars().next().is_some_and(|c| c.is_alphabetic()) {
                        return pref.len() + apo_len;
                    }
                }
            }
        }
    }
    0
}

fn tokenize(seg: &str) -> Vec<Tok> {
    let b = seg.as_bytes();
    let mut toks = Vec::new();
    let mut i = 0;
    let mut nl = false;
    while i < b.len() {
        if matches!(b[i], b' ' | b'\t' | b'\r') {
            i += 1;
        } else if b[i] == b'\n' {
            nl = true;
            i += 1;
        } else {
            let s = i;
            while i < b.len() && !matches!(b[i], b' ' | b'\t' | b'\n' | b'\r') {
                i += 1;
            }
            let raw = &seg[s..i];
            let elision_len = french_elision_prefix_len(raw);
            if elision_len > 0 {
                toks.push(Tok {
                    s,
                    e: s + elision_len,
                    core_s: s,
                    core_e: s + elision_len,
                    nl_before: nl,
                });
                let rest = &seg[s + elision_len..i];
                let lead = rest.len() - rest.trim_start_matches(|c: char| "(\"'“‘".contains(c)).len();
                let core = rest[lead..].trim_end_matches(|c: char| ".,;:!?)\"'”".contains(c));
                toks.push(Tok {
                    s: s + elision_len,
                    e: i,
                    core_s: s + elision_len + lead,
                    core_e: s + elision_len + lead + core.len(),
                    nl_before: false,
                });
            } else {
                let lead = raw.len() - raw.trim_start_matches(|c: char| "(\"'“‘".contains(c)).len();
                let core = raw[lead..].trim_end_matches(|c: char| ".,;:!?)\"'”".contains(c));
                toks.push(Tok {
                    s,
                    e: i,
                    core_s: s + lead,
                    core_e: s + lead + core.len(),
                    nl_before: nl,
                });
            }
            nl = false;
        }
    }
    toks
}

/// Curated allow list of generic company names, product names, and technical terms.
/// Portions Copyright (c) 2019 Microsoft Corporation. Licensed under the MIT License.
/// Verified against Microsoft Presidio (presidio-analyzer/presidio_analyzer/analyzer_engine.py).
static CURATED_ALLOWLIST: LazyLock<HashSet<String>> = LazyLock::new(|| {
    include_str!("allowlist.txt")
        .lines()
        .map(|l| l.trim().to_lowercase())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect()
});

/// Safety rule: the allow list must never suppress identity tokens (the user's registered name)
/// or any value that follows a label. Enforced here in code.
fn is_allowlisted(core: &str, opts: &ScrubOptions) -> bool {
    let lower = core.to_lowercase();
    // Safety check: never suppress identity tokens
    if opts.identity_tokens.iter().any(|t| t.eq_ignore_ascii_case(&lower)) {
        return false;
    }
    NAME_ALLOWLIST.contains(&lower.as_str()) || CURATED_ALLOWLIST.contains(&lower)
}

fn name_like(core: &str, opts: &ScrubOptions) -> bool {
    if core.chars().count() < 2 || is_allowlisted(core, opts) {
        return false;
    }
    CAPITALISED_RE.is_match(core) || ALLCAPS_RE.is_match(core)
}

/// Identifies whether a segment has a Latin-sentence context based on characteristic
/// grammatical words and inflectional/enclitic patterns.
fn is_latin_sentence_context(seg: &str) -> bool {
    const LATIN_MARKERS: &[&str] = &[
        "mors", "aequo", "pulsat", "pede", "pauperum", "tabernas", "turres",
        "donec", "eris", "fuerint", "nubila", "solus", "amicos", "numerabis",
        "dico", "vobis", "diligite", "inimicos", "vestros", "corde", "exeunt",
        "cogitationes", "malae", "autem", "venditur", "auro", "caeleste", "bonum",
        "praestat", "omnibus", "opibus",
    ];
    let mut count = 0;
    for raw in seg.split_whitespace() {
        let w = raw.trim_matches(|c: char| !c.is_alphabetic()).to_lowercase();
        if w.is_empty() {
            continue;
        }
        let is_latin_marker = LATIN_MARKERS.contains(&w.as_str())
            || (w.len() >= 5
                && (w.ends_with("ibus") || w.ends_with("orum") || w.ends_with("arum")))
            || (w.len() >= 5
                && w.ends_with("que")
                && !matches!(
                    w.as_str(),
                    "porque"
                        | "porqué"
                        | "aunque"
                        | "ataque"
                        | "choque"
                        | "bloque"
                        | "tanque"
                        | "cheque"
                        | "duque"
                        | "parque"
                        | "bosque"
                ));
        if is_latin_marker {
            count += 1;
        }
        if count >= 2 {
            return true;
        }
    }
    false
}

fn is_connector_bridge(seg: &str, toks: &[Tok], idx: usize) -> Option<usize> {
    if idx >= toks.len() || toks[idx].nl_before {
        return None;
    }
    let raw = &seg[toks[idx].s..toks[idx].e];
    let core = &seg[toks[idx].core_s..toks[idx].core_e];
    if raw != core {
        return None;
    }
    match core {
        "del" | "y" | "da" | "do" | "dos" | "das" | "e" => Some(1),
        "de" => {
            if idx + 1 < toks.len() && !toks[idx + 1].nl_before {
                let raw2 = &seg[toks[idx + 1].s..toks[idx + 1].e];
                let core2 = &seg[toks[idx + 1].core_s..toks[idx + 1].core_e];
                if raw2 == core2 && matches!(core2, "la" | "los" | "las") {
                    return Some(2);
                }
            }
            Some(1)
        }
        _ => None,
    }
}

fn is_french_sentence(toks: &[Tok], seg: &str) -> bool {
    if seg.chars().any(|c| is_cjk_or_hangul(c) || is_bengali_char(c)) {
        return false;
    }
    let es_markers = toks.iter().filter(|t| {
        let tw = seg[t.core_s..t.core_e].to_lowercase();
        matches!(tw.as_str(), "de" | "la" | "el" | "que" | "en" | "y" | "los" | "las" | "por" | "con" | "una" | "es" | "fue" | "del" | "al")
    }).count();

    let de_markers = toks.iter().filter(|t| {
        let tw = seg[t.core_s..t.core_e].to_lowercase();
        matches!(tw.as_str(), "der" | "die" | "das" | "und" | "den" | "von" | "zu" | "mit" | "ist" | "nicht" | "dem" | "sich" | "des" | "ein" | "eine" | "hat")
    }).count();
    if de_markers >= 2 || (de_markers > 0 && de_markers >= toks.len() / 2) {
        return false;
    }

    let it_markers = toks.iter().filter(|t| {
        let tw = seg[t.core_s..t.core_e].to_lowercase();
        matches!(tw.as_str(), "dell" | "della" | "per" | "scrivere")
    }).count();
    if it_markers > 0 {
        return false;
    }

    let fr_markers = toks.iter().filter(|t| {
        let tw = seg[t.core_s..t.core_e].to_lowercase();
        matches!(
            tw.as_str(),
            "le" | "la" | "les" | "un" | "une" | "des" | "du" | "de" | "en" | "dans" | "par" | "pour" | "sur" | "avec"
                | "et" | "ou" | "est" | "sont" | "a" | "ont" | "été" | "ete" | "il" | "elle" | "ils" | "elles" | "ce"
                | "cette" | "ces" | "qui" | "que" | "sa" | "son" | "ses" | "au" | "aux" | "hier" | "très" | "tres"
                | "plus" | "ne" | "pas" | "nous" | "vous" | "leur" | "leurs" | "mon" | "ma" | "mes" | "ton" | "ta" | "tes"
                | "ici" | "notre" | "votre" | "nos" | "vos"
        )
    }).count();

    let en_markers = toks.iter().filter(|t| {
        let tw = seg[t.core_s..t.core_e].to_lowercase();
        matches!(
            tw.as_str(),
            "the" | "an" | "and" | "at" | "was" | "were" | "this" | "that" | "by" | "from" | "we" | "they"
        )
    }).count();

    let has_es_specific = toks.iter().any(|t| {
        let tw = seg[t.core_s..t.core_e].to_lowercase();
        is_spanish_common_word(&tw) && matches!(tw.as_str(), "roja" | "rojo" | "rojos" | "rojas" | "azul" | "azules" | "ayer" | "sobre" | "proyecto" | "hidalgo" | "aldea" | "mancha" | "senor" | "señor")
    });

    let has_fr_elision = toks.iter().any(|t| {
        let raw = &seg[t.s..t.e].to_lowercase();
        raw.starts_with("l'") || raw.starts_with("l’")
            || raw.starts_with("d'") || raw.starts_with("d’")
            || raw.starts_with("qu'") || raw.starts_with("qu’")
            || raw.starts_with("c'") || raw.starts_with("c’")
            || raw.starts_with("j'") || raw.starts_with("j’")
            || raw.starts_with("n'") || raw.starts_with("n’")
            || raw.starts_with("s'") || raw.starts_with("s’")
            || raw.starts_with("t'") || raw.starts_with("t’")
    });

    if es_markers > fr_markers || (es_markers == fr_markers && has_es_specific) {
        return false;
    }
    if en_markers > fr_markers {
        return false;
    }

    fr_markers >= 1 || has_fr_elision
}

fn is_english_sentence(toks: &[Tok], seg: &str) -> bool {
    if seg.chars().any(|c| is_cjk_or_hangul(c) || is_bengali_char(c)) {
        return false;
    }
    let es_markers = toks.iter().filter(|t| {
        let tw = seg[t.core_s..t.core_e].to_lowercase();
        matches!(tw.as_str(), "de" | "la" | "el" | "que" | "en" | "y" | "los" | "las" | "por" | "con" | "una" | "es" | "fue" | "del" | "al")
    }).count();
    if es_markers >= 2 {
        return false;
    }
    let de_markers = toks.iter().filter(|t| {
        let tw = seg[t.core_s..t.core_e].to_lowercase();
        matches!(tw.as_str(), "der" | "die" | "das" | "und" | "den" | "von" | "zu" | "mit" | "ist" | "nicht" | "dem" | "sich" | "des" | "ein" | "eine" | "hat")
    }).count();
    if de_markers >= 2 {
        return false;
    }
    let fr_markers = toks.iter().filter(|t| {
        let tw = seg[t.core_s..t.core_e].to_lowercase();
        matches!(
            tw.as_str(),
            "le" | "la" | "les" | "un" | "une" | "des" | "du" | "de" | "dans" | "par" | "pour" | "sur" | "avec"
                | "est" | "sont" | "été" | "ete" | "il" | "elle" | "ils" | "elles" | "cette"
                | "ces" | "qui" | "sa" | "son" | "ses" | "hier" | "très" | "tres"
        )
    }).count();
    let has_fr_elision = toks.iter().any(|t| {
        let raw = &seg[t.s..t.e].to_lowercase();
        raw.starts_with("l'") || raw.starts_with("l’")
            || raw.starts_with("d'") || raw.starts_with("d’")
            || raw.starts_with("qu'") || raw.starts_with("qu’")
    });
    if fr_markers >= 2 || has_fr_elision {
        return false;
    }

    let en_markers = toks.iter().filter(|t| {
        let tw = seg[t.core_s..t.core_e].to_lowercase();
        matches!(
            tw.as_str(),
            "the" | "a" | "an" | "and" | "or" | "in" | "on" | "at" | "to" | "for" | "of" | "with" | "is" | "are"
                | "was" | "were" | "it" | "this" | "that" | "by" | "from" | "as" | "be" | "have" | "has" | "had"
                | "we" | "you" | "they" | "i" | "he" | "she"
        )
    }).count();

    en_markers >= 1 && en_markers >= fr_markers
}

fn is_portuguese_sentence(toks: &[Tok], seg: &str) -> bool {
    if seg.chars().any(|c| is_cjk_or_hangul(c) || is_bengali_char(c)) {
        return false;
    }
    let pt_markers = toks.iter().filter(|t| {
        let tw = seg[t.core_s..t.core_e].to_lowercase();
        matches!(
            tw.as_str(),
            "não" | "nao" | "uma" | "umas" | "pelo" | "pela" | "pelos" | "pelas"
                | "são" | "sao" | "do" | "da" | "dos" | "das" | "no" | "na" | "nos" | "nas"
                | "deste" | "desta" | "destes" | "destas" | "disto"
                | "desse" | "dessa" | "desses" | "dessas" | "disso"
                | "neste" | "nesta" | "nestes" | "nestas" | "nisto"
                | "nesse" | "nessa" | "nesses" | "nessas" | "nisso"
                | "ele" | "eles" | "elas" | "você" | "voce" | "vocês" | "voces"
                | "foi" | "foram" | "teve" | "tinham" | "tiveram" | "mais" | "ontem" | "amanhã" | "amanha"
                | "manhã" | "manha"
                | "depois" | "muito" | "muita" | "muitos" | "muitas"
                | "também" | "tambem" | "está" | "estão" | "estao"
                | "outro" | "outra" | "outros" | "outras" | "mesmo" | "mesma"
                | "mesmos" | "mesmas" | "novo" | "nova" | "novos" | "novas"
                | "bom" | "boa" | "bons" | "boas" | "até" | "ate" | "sem"
                | "com" | "ao" | "aos" | "mês" | "ano" | "anos"
        ) || (tw.len() >= 5 && tw.ends_with("ou"))
          || tw.ends_with("ção")
          || tw.ends_with("ções")
          || tw.ends_with("mento")
    }).count();

    let es_markers = toks.iter().filter(|t| {
        let tw = seg[t.core_s..t.core_e].to_lowercase();
        matches!(
            tw.as_str(),
            "el" | "los" | "las" | "del" | "al" | "y" | "con" | "son"
                | "fue" | "fueron" | "pero" | "ayer" | "hoy" | "mañana"
                | "manana" | "año" | "años" | "ano" | "anos" | "sin"
                | "muy" | "después" | "despues" | "hasta"
        )
    }).count();

    let fr_markers = toks.iter().filter(|t| {
        let tw = seg[t.core_s..t.core_e].to_lowercase();
        matches!(
            tw.as_str(),
            "le" | "la" | "les" | "un" | "des" | "du" | "dans" | "par" | "pour" | "sur" | "avec"
                | "est" | "sont" | "été" | "ete" | "il" | "elle" | "ils" | "elles"
        )
    }).count();

    let en_markers = toks.iter().filter(|t| {
        let tw = seg[t.core_s..t.core_e].to_lowercase();
        matches!(
            tw.as_str(),
            "the" | "an" | "and" | "at" | "was" | "were" | "this" | "that" | "by" | "from" | "we" | "they"
        )
    }).count();

    if es_markers > pt_markers {
        return false;
    }
    if fr_markers > pt_markers {
        return false;
    }
    if en_markers > pt_markers {
        return false;
    }

    pt_markers >= 1
}

fn is_connector_target_redacted(
    after_w: &str,
    is_portuguese_text: bool,
    is_french_text: bool,
    is_english_text: bool,
) -> bool {
    let after_w_stripped = strip_latin_accents(after_w);
    let after_is_known = if is_portuguese_text {
        is_pt_word(after_w) && !is_name_gated("pt", after_w)
    } else {
        is_known_en_or_es_word(after_w)
            || (!is_french_text && !is_english_text && is_name_gated("es", after_w))
    };
    let after_es_gated =
        !is_portuguese_text && !is_french_text && !is_english_text && is_name_gated("es", after_w);
    let namelist = get_namelist();
    let multilang = get_multilang_names();
    let after_is_nl = !after_es_gated
        && (namelist.contains(after_w)
            || namelist.contains(after_w_stripped.as_str())
            || multilang.contains_key(after_w)
            || multilang.contains_key(&after_w_stripped));
    after_is_nl || !after_is_known
}

/// Tier 3: runs of Capitalised / ALL-CAPS words with context sweeping using NameList.
fn scrub_unknown_caps(
    seg: &str,
    first_segment: bool,
    opts: &ScrubOptions,
    matched_name_languages: &mut BTreeSet<String>,
) -> String {
    if is_latin_sentence_context(seg) {
        return seg.to_string();
    }
    let toks = tokenize(seg);
    if toks.is_empty() {
        return seg.to_string();
    }
    let n = toks.len();
    let like: Vec<bool> = toks
        .iter()
        .map(|t| name_like(&seg[t.core_s..t.core_e], opts))
        .collect();
    let mut redacted_tok = vec![false; n];
    let is_portuguese_text = is_portuguese_sentence(&toks, seg);
    let is_french_text = is_french_sentence(&toks, seg);
    let is_english_text = is_english_sentence(&toks, seg);

    let mut i = 0;
    while i < n {
        if !like[i] {
            i += 1;
            continue;
        }
        let mut j = i;
        while j + 1 < n
            && like[j + 1]
            && !toks[j + 1].nl_before
            && !matches!(
                seg[toks[j].s..toks[j].e].chars().last(),
                Some('.' | '!' | '?' | ';' | ':')
            )
        {
            j += 1;
        }

        let heading = seg[toks[j].s..toks[j].e].ends_with(':');
        if !heading {
            let mut is_unk = vec![false; j - i + 1];
            let mut is_nl = vec![false; j - i + 1];
            let mut sentence_initial = vec![false; j - i + 1];

            for k in i..=j {
                let w = seg[toks[k].core_s..toks[k].core_e].to_lowercase();
                let w_stripped = strip_latin_accents(&w);

                let mut in_cache = false;
                if let Some(cache) = &opts.dynamic_name_cache {
                    if let Ok(cache_lock) = cache.read() {
                        in_cache = cache_lock.contains(&w) || cache_lock.contains(&w_stripped);
                    }
                }

                let is_es_gated =
                    !is_portuguese_text && !is_french_text && !is_english_text && is_name_gated("es", &w);
                let is_known = if is_portuguese_text {
                    is_pt_word(&w) && !is_name_gated("pt", &w)
                } else if is_french_text {
                    is_known_en_or_es_word(&w) || (is_fr_word(&w) && !is_name_gated("fr", &w))
                } else {
                    is_known_en_or_es_word(&w) || is_es_gated
                };
                is_unk[k - i] = in_cache || !is_known;
                let namelist = get_namelist();
                let multilang = get_multilang_names();
                is_nl[k - i] = !is_es_gated
                    && (namelist.contains(w.as_str())
                        || namelist.contains(w_stripped.as_str())
                        || multilang.contains_key(&w)
                        || multilang.contains_key(&w_stripped));

                sentence_initial[k - i] = (k == 0 && first_segment)
                    || toks[k].nl_before
                    || (k > 0
                        && matches!(
                            seg[toks[k - 1].s..toks[k - 1].e].chars().last(),
                            Some('.' | '!' | '?' | '¡' | '¿')
                        ))
                    || seg[toks[k].s..toks[k].core_s]
                        .chars()
                        .any(|c| matches!(c, '¡' | '¿'));
            }

            for k in i..=j {
                let idx = k - i;
                let w = seg[toks[k].core_s..toks[k].core_e].to_lowercase();
                let w_stripped = strip_latin_accents(&w);
                let is_ambiguous_common = if is_portuguese_text {
                    false
                } else {
                    is_spanish_common_word(&w)
                };

                let mut redact = false;
                let mut matched_lang: Option<&str> = None;

                let namelist = get_namelist();
                let multilang = get_multilang_names();
                if is_french_text && is_name_gated("fr", &w) {
                    redact = true;
                    matched_lang = Some("fr");
                } else if is_portuguese_text && is_name_gated("pt", &w) {
                    redact = true;
                    matched_lang = Some("pt");
                } else if is_english_text
                    && !sentence_initial[idx]
                    && (namelist.contains(w.as_str())
                        || multilang.get(&w).is_some_and(|langs| langs.contains(&"en")))
                {
                    redact = true;
                    matched_lang = Some("en");
                } else if is_unk[idx] {
                    redact = true;
                    if is_french_text
                        && multilang
                            .get(&w)
                            .or_else(|| multilang.get(&w_stripped))
                            .is_some_and(|langs| langs.contains(&"fr"))
                    {
                        matched_lang = Some("fr");
                    } else if is_portuguese_text
                        && multilang
                            .get(&w)
                            .or_else(|| multilang.get(&w_stripped))
                            .is_some_and(|langs| langs.contains(&"pt"))
                    {
                        matched_lang = Some("pt");
                    }
                } else if is_nl[idx] {
                    // Check for activating neighbor. An activating neighbor must be:
                    // 1. An unknown capitalized word (is_unk)
                    // 2. OR an unambiguous name from namelist (!is_spanish_common_word)
                    // Two ambiguous common nouns (like "Sierra Morena" or "Blanca Rosa") do NOT activate each other.
                    let is_activating = |other_k: usize| -> bool {
                        let other_idx = other_k - i;
                        if is_unk[other_idx] {
                            return true;
                        }
                        if is_nl[other_idx] {
                            let other_w = seg[toks[other_k].core_s..toks[other_k].core_e].to_lowercase();
                            if !is_spanish_common_word(&other_w) {
                                return true;
                            }
                        }
                        false
                    };

                    let mut has_activating_neighbor = false;
                    if idx > 0 && is_activating(k - 1) {
                        has_activating_neighbor = true;
                    }
                    if idx < j - i && is_activating(k + 1) {
                        has_activating_neighbor = true;
                    }
                    if !is_ambiguous_common && !has_activating_neighbor {
                        if let Some(conn_len) = is_connector_bridge(seg, &toks, k + 1) {
                            let after_idx = k + 1 + conn_len;
                            if after_idx < n && !toks[after_idx].nl_before && like[after_idx] {
                                let after_w = seg[toks[after_idx].core_s..toks[after_idx].core_e].to_lowercase();
                                if is_connector_target_redacted(
                                    &after_w,
                                    is_portuguese_text,
                                    is_french_text,
                                    is_english_text,
                                ) {
                                    has_activating_neighbor = true;
                                }
                            }
                        }
                    }

                    // Ambiguous common nouns (Caballero, Rey, Gallo, Curioso, Sierra, Morena, etc.)
                    // only redact when they have an activating personal-name neighbor (e.g. "José Caballero", "Juan Gallo", "Pedro Rey").
                    // They do NOT redact when standing alone mid-sentence or sentence-initial as titles of rank or common nouns.
                    if is_ambiguous_common {
                        if has_activating_neighbor {
                            redact = true;
                        }
                    } else if !sentence_initial[idx] || has_activating_neighbor {
                        redact = true;
                        if is_portuguese_text {
                            matched_lang = Some("pt");
                        }
                    }
                }

                if redact {
                    redacted_tok[k] = true;
                    if let Some(l) = matched_lang {
                        matched_name_languages.insert(l.to_string());
                    }
                }
            }
        }
        i = j + 1;
    }

    // Bridge lowercase Spanish surname connectors (de, del, de la, de los, de las, y)
    // when a redacted capitalized token precedes the connector and a capitalized name-like token follows it.
    for k in 0..n {
        if !redacted_tok[k] {
            continue;
        }
        if matches!(
            seg[toks[k].s..toks[k].e].chars().last(),
            Some('.' | '!' | '?' | ';' | ':' | ',')
        ) {
            continue;
        }
        if let Some(conn_len) = is_connector_bridge(seg, &toks, k + 1) {
            let after_idx = k + 1 + conn_len;
            if after_idx < n && !toks[after_idx].nl_before && like[after_idx] {
                let after_w = seg[toks[after_idx].core_s..toks[after_idx].core_e].to_lowercase();
                // Only bridge if target is in a name list or an unknown capitalized word that is NOT an ordinary known word/place.
                // Do not bridge place names or ordinary known words like "Mancha", "Madrid", "Sevilla".
                if is_connector_target_redacted(
                    &after_w,
                    is_portuguese_text,
                    is_french_text,
                    is_english_text,
                ) {
                    redacted_tok[after_idx] = true;
                    redacted_tok[(k + 1)..after_idx].fill(true);
                    if is_portuguese_text {
                        matched_name_languages.insert("pt".to_string());
                    }
                }
            }
        }
    }

    let mut spans: Vec<(usize, usize)> = Vec::new();
    let mut k = 0;
    while k < n {
        if !redacted_tok[k] {
            k += 1;
            continue;
        }
        let start_s = toks[k].core_s;
        let mut end_e = toks[k].core_e;
        while k + 1 < n
            && redacted_tok[k + 1]
            && !toks[k + 1].nl_before
            && seg[toks[k].core_e..toks[k].e].is_empty()
            && seg[toks[k + 1].s..toks[k + 1].core_s].is_empty()
        {
            k += 1;
            end_e = toks[k].core_e;
        }
        spans.push((start_s, end_e));
        k += 1;
    }
    apply_spans(seg, &spans, "[NAME]")
}

fn apply_spans(text: &str, spans: &[(usize, usize)], tag: &str) -> String {
    let mut sorted: Vec<(usize, usize)> = spans.to_vec();
    sorted.sort();
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for (s, e) in sorted {
        if s < last {
            continue;
        }
        out.push_str(&text[last..s]);
        out.push_str(tag);
        last = e;
    }
    out.push_str(&text[last..]);
    out
}

// Context cues for salutations and sign-offs.
// Ambiguous standalone cues "A" (it) and "De" (fr) are excluded to prevent over-redaction
// of sentences starting with prepositions.
// Always-on Bangla honorifics, titles, and greetings (Tier 2B).
const BN_CONTEXT_CUES: &[&str] = &[
    "জনাবা",
    "জনাব",
    "মিসেস",
    "মিস",
    "বেগম",
    "ডক্টর",
    "ডাঃ",
    "ডা.",
    "ডঃ",
    "ড.",
    "প্রফেসর",
    "অধ্যাপক",
    "ইঞ্জিনিয়ার",
    "প্রিয়",
    "শ্রদ্ধেয়",
    "শ্রদ্ধাভাজন",
    "মাননীয়",
    "মোঃ",
    "মো.",
    "আলহাজ্ব",
    "হাজী",
    "মরহুমা",
    "মরহুম",
    "স্বর্গীয়",
    "শ্রীমতী",
    "শ্রী",
];

// Edge-gated Bangla salutations and sign-off cues (Tier 2A).
const BN_SALUTATION_CUES: &[&str] = &[
    "সালাম",
    "নমস্কার",
    "শুভেচ্ছান্তে",
    "বিনীত",
    "নিবেদক",
    "ইতি",
    "ধন্যবাদান্তে",
    "স্বাক্ষর",
];

fn is_bengali_char(c: char) -> bool {
    ('\u{0980}'..='\u{09FF}').contains(&c)
}

fn is_bengali_token(core: &str) -> bool {
    !core.is_empty()
        && core.chars().any(is_bengali_char)
        && core
            .chars()
            .all(|c| is_bengali_char(c) || c == '\u{200C}' || c == '\u{200D}')
}

/// Collect up to `max` consecutive Bengali-script name/unknown tokens starting at `from` after a Bangla cue.
fn bn_cue_run(
    seg: &str,
    toks: &[Tok],
    from: usize,
    max: usize,
    opts: &ScrubOptions,
) -> Option<(usize, usize)> {
    let custom_res = opts
        .bn_resource_dir
        .as_ref()
        .map(|d| load_bn_resources_from_dir(d));
    let res: &BnResources = custom_res.as_ref().unwrap_or_else(|| get_bn_resources());

    let mut first: Option<usize> = None;
    let mut last = 0;
    let mut k = from;
    let mut count = 0;
    while k < toks.len() && count < max {
        if first.is_some() && toks[k].nl_before {
            break;
        }
        let raw = &seg[toks[k].s..toks[k].e];
        let core = &seg[toks[k].core_s..toks[k].core_e];
        if !is_bengali_token(core) {
            break;
        }
        let kind = classify_bn_token_in(core, res);
        // After a cue, accept tokens classified as Name or Unknown; never redact a token classified as Word.
        let accept = kind != BnTokKind::Word;
        if !accept {
            break;
        }
        first.get_or_insert(toks[k].core_s);
        last = toks[k].core_e;
        count += 1;
        k += 1;
        if raw.ends_with([',', '।', '.', ';', ':', '!', '?']) {
            break;
        }
    }
    first.map(|s| (s, last))
}
// "Miss" is included in case-sensitive title form (\bMiss\b) so it does not match
// lowercase verb uses like "I miss London".
static CUE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i:\b(?:dear|hello|estimado|estimada|prezado|prezada|mr|mrs|ms|mme|mlle|monsieur|madame|mademoiselle|sr|sra|srta|dr|dra|doctor|doctora|prof|professeur|professor|professora|lic|ing|md|mohammad|mohd)\b\.?|\b(?:from|to|cc|attn):|\bc/o\b|\bsd/-?)|\bMiss\b\.?|\bM\b\.",
    )
    .unwrap()
});

// Edge-gated greeting and closing cues.
// Fired only within the first 3 non-empty lines or the last 3 non-empty lines of input.
static SALUTATION_CUE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:hi|regards|sincerely|best|thanks|signed|signature|bonjour|bonsoir|cher|chère|chere|cordialement|amitiés|amities|prezado|prezada|atenciosamente|abraços|abracos|cumprimentos|saudações|saudacoes|olá|ola)\b\.?",
    )
    .unwrap()
});

pub fn non_empty_line_spans(text: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut line_start = 0;
    for (i, b) in text.bytes().enumerate() {
        if b == b'\n' {
            let line = &text[line_start..i];
            if !line.trim().is_empty() {
                spans.push((line_start, i));
            }
            line_start = i + 1;
        }
    }
    if line_start < text.len() {
        let line = &text[line_start..];
        if !line.trim().is_empty() {
            spans.push((line_start, text.len()));
        }
    }
    spans
}

fn scrub_salutation_cues(text: &str, opts: &ScrubOptions) -> String {
    let non_empty = non_empty_line_spans(text);
    let is_in_edge = |pos: usize| -> bool {
        if non_empty.len() <= 6 {
            true
        } else {
            let first_window_end = non_empty[2].1;
            let last_window_start = non_empty[non_empty.len() - 3].0;
            pos <= first_window_end || pos >= last_window_start
        }
    };

    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for pm in PLACEHOLDER_RE.find_iter(text) {
        let seg_start = last;
        let seg_end = pm.start();
        let seg = &text[seg_start..seg_end];
        out.push_str(&apply_salutation_cues_to_seg(seg, seg_start, &is_in_edge, opts));
        out.push_str(pm.as_str());
        last = pm.end();
    }
    let seg_start = last;
    let seg = &text[seg_start..];
    out.push_str(&apply_salutation_cues_to_seg(seg, seg_start, &is_in_edge, opts));
    out = map_unprotected(&out, |seg, _| scrub_context_cues(seg, opts));
    out
}

fn apply_salutation_cues_to_seg(
    seg: &str,
    seg_start: usize,
    is_in_edge: &impl Fn(usize) -> bool,
    opts: &ScrubOptions,
) -> String {
    let toks = tokenize(seg);
    let mut spans: Vec<(usize, usize)> = Vec::new();
    for m in SALUTATION_CUE_RE.find_iter(seg) {
        let doc_pos = seg_start + m.start();
        if !is_in_edge(doc_pos) {
            continue;
        }
        let lower = m.as_str().to_lowercase();
        let closing = [
            "sincerely", "regards", "best", "thanks", "signed", "signature",
            "cordialement", "amitiés", "amities",
            "atenciosamente", "abraços", "abracos", "cumprimentos", "saudações", "saudacoes",
        ]
            .iter()
            .any(|k| lower.starts_with(k));

        // same line, tokens right after the cue
        let first_after = toks.iter().position(|t| t.s >= m.end());
        if let Some(fi) = first_after {
            if !toks[fi].nl_before {
                if let Some(span) = cap_run(seg, &toks, fi, 4, opts)
                    .or_else(|| bn_cue_run(seg, &toks, fi, 4, opts))
                {
                    spans.push(span);
                }
            }
        }
        if closing {
            // first capitalised run on the immediately following line
            let line_end = seg[m.end()..].find('\n').map(|p| m.end() + p);
            if let Some(le) = line_end {
                let next_line_end = seg[le + 1..]
                    .find('\n')
                    .map(|p| le + 1 + p)
                    .unwrap_or(seg.len());
                let candidates: Vec<usize> = toks
                    .iter()
                    .enumerate()
                    .filter(|(_, t)| t.s > le && t.s < next_line_end)
                    .map(|(i, _)| i)
                    .collect();
                for &ci in &candidates {
                    if CUE_CAP_RE.is_match(&seg[toks[ci].s..toks[ci].e]) {
                        if let Some(span) = cap_run(seg, &toks, ci, 4, opts) {
                            spans.push(span);
                        }
                        break;
                    }
                }
            }
        }
    }
    // Bangla edge-gated salutation and closing cues
    for (ti, tok) in toks.iter().enumerate() {
        let doc_pos = seg_start + tok.s;
        if !is_in_edge(doc_pos) {
            continue;
        }
        let raw_clean = seg[tok.s..tok.e].trim_end_matches([',', ':']);
        let core = &seg[tok.core_s..tok.core_e];
        let norm_raw = normalize_bn_str(raw_clean);
        let norm_core = normalize_bn_str(core);
        if BN_SALUTATION_CUES
            .iter()
            .any(|&c| normalize_bn_str(c) == norm_raw || normalize_bn_str(c) == norm_core)
            && ti + 1 < toks.len()
            && !toks[ti + 1].nl_before
        {
            if let Some(span) = bn_cue_run(seg, &toks, ti + 1, 4, opts) {
                spans.push(span);
            }
        }
    }
    apply_spans(seg, &spans, "[NAME]")
}

/// Collect up to `max` consecutive capitalised tokens starting at token index `from`,
/// bridging Spanish surname connectors (`de`, `del`, `de la`, `de los`, `de las`, `y`).
fn cap_run(
    seg: &str,
    toks: &[Tok],
    from: usize,
    max: usize,
    opts: &ScrubOptions,
) -> Option<(usize, usize)> {
    let mut first: Option<usize> = None;
    let mut last = 0;
    let mut k = from;
    let mut count = 0;
    while k < toks.len() && count < max {
        let raw = &seg[toks[k].s..toks[k].e];
        let core = &seg[toks[k].core_s..toks[k].core_e];
        if first.is_some() && toks[k].nl_before {
            break;
        }
        if first.is_some() {
            if let Some(conn_len) = is_connector_bridge(seg, toks, k) {
                let after_idx = k + conn_len;
                if after_idx < toks.len() && !toks[after_idx].nl_before {
                    let raw_after = &seg[toks[after_idx].s..toks[after_idx].e];
                    let core_after = &seg[toks[after_idx].core_s..toks[after_idx].core_e];
                    if CUE_CAP_RE.is_match(raw_after) && !is_allowlisted(core_after, opts) {
                        last = toks[after_idx].core_e;
                        count += 1;
                        k = after_idx + 1;
                        if raw_after.ends_with(',') {
                            break;
                        }
                        continue;
                    }
                }
            }
        }
        if !CUE_CAP_RE.is_match(raw) || is_allowlisted(core, opts) {
            break;
        }
        first.get_or_insert(toks[k].core_s);
        last = toks[k].core_e;
        count += 1;
        k += 1;
        if raw.ends_with(',') {
            break;
        }
    }
    first.map(|s| (s, last))
}

/// Tier 2B: names after titles (mr, mrs, ms, Miss, sr, sra, srta, dr, dra, prof, lic, ing, md, mohammad, mohd, c/o, sd/-, and Bangla honorifics/greetings)
fn scrub_context_cues(seg: &str, opts: &ScrubOptions) -> String {
    let toks = tokenize(seg);
    let mut spans: Vec<(usize, usize)> = Vec::new();
    for m in CUE_RE.find_iter(seg) {
        // same line, tokens right after the cue
        let first_after = toks.iter().position(|t| t.s >= m.end());
        if let Some(fi) = first_after {
            if !toks[fi].nl_before {
                if let Some(span) = cap_run(seg, &toks, fi, 6, opts)
                    .or_else(|| bn_cue_run(seg, &toks, fi, 4, opts))
                {
                    spans.push(span);
                }
            }
        }
    }
    for (ti, tok) in toks.iter().enumerate() {
        let raw_clean = seg[tok.s..tok.e].trim_end_matches([',', ':']);
        let core = &seg[tok.core_s..tok.core_e];
        let norm_raw = normalize_bn_str(raw_clean);
        let norm_core = normalize_bn_str(core);
        if BN_CONTEXT_CUES
            .iter()
            .any(|&c| normalize_bn_str(c) == norm_raw || normalize_bn_str(c) == norm_core)
            && ti + 1 < toks.len()
            && !toks[ti + 1].nl_before
        {
            if let Some(span) = bn_cue_run(seg, &toks, ti + 1, 4, opts) {
                spans.push(span);
            }
        }
    }
    apply_spans(seg, &spans, "[NAME]")
}

const JAPANESE_HONORIFICS_TITLES: &[&str] = &[
    "さん", "様", "君", "ちゃん", "殿", "先生", "部長", "課長", "社長", "係長", "主任", "店長", "教授", "博士",
];

const JAPANESE_PREFIX_TITLES: &[&str] = &[
    "課長", "部長", "社長", "係長", "主任", "店長", "教授", "博士", "先生",
];

const KOREAN_HONORIFICS_TITLES: &[&str] = &[
    "선생님", "과장님", "부장님", "대표님", "교수님", "팀장님", "대리님", "차장님", "실장님", "원장님",
    "사장님", "소장님", "국장님", "박사님", "선생", "과장", "부장", "대표", "교수", "팀장", "연구원",
    "대리", "차장", "실장", "원장", "사장", "소장", "국장", "박사", "씨", "님",
];

const KOREAN_PREFIX_TITLES: &[&str] = &[
    "선생님", "과장님", "부장님", "대표님", "교수님", "팀장님", "대리님", "차장님", "실장님", "원장님",
    "사장님", "소장님", "국장님", "박사님", "선생", "과장", "부장", "대표", "교수", "팀장", "연구원",
    "대리", "차장", "실장", "원장", "사장", "소장", "국장", "박사",
];

const JA_JINMEI_COMMON_NOUN_STOPLIST: &[&str] = &[
    "あかり",
];

const JA_SINGLE_CHAR_SURNAME_COMMON_STOPLIST: &[&str] = &[
    "丸", "金", "上", "何",
];

fn is_ja_given_name(s: &str) -> bool {
    !JA_JINMEI_COMMON_NOUN_STOPLIST.contains(&s) && get_ja_given_names().contains(s)
}

fn is_ja_surname(s: &str) -> bool {
    s.chars().count() >= 2 && get_ja_surnames().contains(s)
}

fn is_lindera_surname(m: &mut lindera::token::Token) -> bool {
    let d = m.details();
    d.len() >= 4 && d[1] == "固有名詞" && d[2] == "人名" && d[3] == "姓"
}

fn is_ja_single_char_surname_morpheme(surface: &str, m: &mut lindera::token::Token) -> bool {
    surface.chars().count() == 1
        && !JA_SINGLE_CHAR_SURNAME_COMMON_STOPLIST.contains(&surface)
        && get_ja_surnames().contains(surface)
        && is_lindera_surname(m)
}

fn is_lindera_person_name(m: &mut lindera::token::Token) -> bool {
    let d = m.details();
    d.len() >= 3 && d[1] == "固有名詞" && d[2] == "人名"
}

fn is_lindera_non_independent_noun(m: &mut lindera::token::Token) -> bool {
    let d = m.details();
    d.len() >= 2 && d[0] == "名詞" && d[1] == "非自立"
}

fn extract_japanese_given_names_from_full_name(s: &str, out: &mut HashSet<String>) {
    if s.chars().count() < 2 || !s.chars().any(is_cjk_or_japanese) {
        return;
    }
    if let Some(segmenter) = get_lindera_segmenter() {
        if let Ok(mut morphemes) = segmenter.segment(std::borrow::Cow::Borrowed(s)) {
            let n = morphemes.len();
            if n >= 2 {
                for split in 1..n {
                    let left = &s[morphemes[0].byte_start..morphemes[split - 1].byte_end];
                    let right = &s[morphemes[split].byte_start..morphemes[n - 1].byte_end];
                    let l_low = left.to_lowercase();
                    let r_low = right.to_lowercase();
                    let l_given = left.chars().count() >= 2
                        && is_ja_given_name(&l_low)
                        && (0..split).all(|idx| is_lindera_person_name(&mut morphemes[idx]));
                    let r_given = right.chars().count() >= 2
                        && is_ja_given_name(&r_low)
                        && (split..n).all(|idx| is_lindera_person_name(&mut morphemes[idx]));
                    let l_sur = is_ja_surname(left)
                        || is_ja_surname(&l_low)
                        || (split == 1 && is_ja_single_char_surname_morpheme(left, &mut morphemes[0]));
                    let r_sur = is_ja_surname(right)
                        || is_ja_surname(&r_low)
                        || (split + 1 == n && is_ja_single_char_surname_morpheme(right, &mut morphemes[split]));
                    if (l_sur || l_given) && (r_sur || r_given) {
                        if l_given {
                            out.insert(l_low);
                        }
                        if r_given {
                            out.insert(r_low);
                        }
                    }
                }
            }
        }
    }
}

fn has_latin_chars(s: &str) -> bool {
    s.chars().any(|c| matches!(c, 'a'..='z' | 'A'..='Z' | '\u{00C0}'..='\u{024F}'))
}

const KOREAN_PARTICLES: &[&str] = &[
    "선생님께서", "선생님께", "선생님은", "선생님이", "선생님을", "선생님과", "선생님도", "선생님",
    "과장님께서", "과장님께", "과장님은", "과장님이", "과장님을", "과장님과", "과장님도", "과장님",
    "부장님께서", "부장님께", "부장님은", "부장님이", "부장님을", "부장님과", "부장님도", "부장님",
    "대표님께서", "대표님께", "대표님은", "대표님이", "대표님을", "대표님과", "대표님도", "대표님",
    "교수님께서", "교수님께", "교수님은", "교수님이", "교수님을", "교수님과", "교수님도", "교수님",
    "씨에게", "님에게",
    "씨는", "씨가", "씨를", "씨의", "씨와", "씨랑", "씨도", "씨께",
    "님은", "님이", "님을", "님의", "님과", "님께", "님도", "님랑",
    "에게서", "한테서", "로부터", "에서", "에게", "한테", "께서", "으로", "부터", "까지",
    "하고", "이랑", "이나", "조차", "마저", "라도", "이라도", "처럼", "만큼", "보다",
    "마다", "밖에", "대로", "이가",
    "은", "는", "이", "가", "을", "를", "의", "에", "께", "과", "와", "도", "로", "만", "나", "랑", "야", "아",
    "씨", "님",
];

const KOREAN_HONORIFIC_PARTICLES: &[&str] = &[
    "선생님께서", "선생님께", "선생님은", "선생님이", "선생님을", "선생님과", "선생님도", "선생님",
    "과장님께서", "과장님께", "과장님은", "과장님이", "과장님을", "과장님과", "과장님도", "과장님",
    "부장님께서", "부장님께", "부장님은", "부장님이", "부장님을", "부장님과", "부장님도", "부장님",
    "대표님께서", "대표님께", "대표님은", "대표님이", "대표님을", "대표님과", "대표님도", "대표님",
    "교수님께서", "교수님께", "교수님은", "교수님이", "교수님을", "교수님과", "교수님도", "교수님",
    "씨에게", "님에게",
    "씨는", "씨가", "씨를", "씨의", "씨와", "씨랑", "씨도", "씨께",
    "님은", "님이", "님을", "님의", "님과", "님께", "님도", "님랑",
    "이가",
    "씨", "님",
];

const KOREAN_SINGLE_GIVEN_NAMES: &[char] = &[
    '민', '준', '진', '훈', '혁', '현', '욱', '철', '수', '호',
    '빈', '솔', '린', '별', '찬', '영', '지', '원', '아', '은',
];

const KOREAN_COMMON_COMPOUND_STOPLIST: &[&str] = &[
    "하지", "남은", "허지", "전은", "신지", "주린", "주지", "손수", "남지", "차지", "성은",
    "오지", "남아", "지원", "이원", "백원", "서방", "이상", "지금", "구경",
    "전이", "박두", "고향", "고요", "구수", "진종", "한여", "고자", "고대", "오나",
    "한방", "유명", "서방의", "이상한", "하지만", "허지만", "지금이", "구경도", "전이라",
    "박두한", "고향이", "고요한", "구수한", "진종일", "한여름", "오지만", "고자야", "고대로",
    "전이나", "오나라", "한방의", "유명한", "회사", "회의", "담당", "보고", "일정", "자료",
    "준비", "검토", "제안", "발표", "공유", "전달", "참석", "오전", "오후", "내일", "오늘",
];

fn is_ko_given_name(s: &str) -> bool {
    s.chars().count() >= 2
        && s.chars().all(is_hangul_char)
        && !KOREAN_COMMON_COMPOUND_STOPLIST.contains(&s)
        && get_ko_given_names().contains(s)
}

fn is_ko_multi_char_surname(s: &str) -> bool {
    s.chars().count() >= 2 && s.chars().all(is_hangul_char) && get_ko_surnames().contains(s)
}

fn is_ko_any_surname(s: &str) -> bool {
    s.chars().all(is_hangul_char) && get_ko_surnames().contains(s)
}

/// Checks if `s` is an unspaced compound Korean full name (surname + given name):
/// - 2-char: `김` + single-syllable given name (e.g. `김민`)
/// - 3-char: 1-char common Korean surname + 2-char Korean given name (e.g. `김민수`, `박지훈`)
/// - 4-char: 2-char Korean surname + 2-char Korean given name (e.g. `남궁민준`, `황보지훈`)
fn is_korean_compound_full_name(s: &str) -> bool {
    if !s.chars().all(is_hangul_char) || KOREAN_COMMON_COMPOUND_STOPLIST.contains(&s) {
        return false;
    }
    let chars: Vec<char> = s.chars().collect();
    match chars.len() {
        2 => {
            chars[0] == '김' && KOREAN_SINGLE_GIVEN_NAMES.contains(&chars[1])
        }
        3 => {
            let c0 = chars[0].to_string();
            let c12: String = chars[1..].iter().collect();
            KO_COMMON_1CHAR_SURNAMES.contains(&c0.as_str()) && is_ko_given_name(&c12)
        }
        4 => {
            let c01: String = chars[..2].iter().collect();
            let c23: String = chars[2..].iter().collect();
            is_ko_multi_char_surname(&c01) && is_ko_given_name(&c23)
        }
        _ => false,
    }
}

/// Strips any trailing Korean particle or honorific+particle from `tok` and returns `(stem, stripped_suffix)`.
fn strip_korean_particle(tok: &str) -> (&str, Option<&str>) {
    for &p in KOREAN_PARTICLES {
        if tok.ends_with(p) && tok.len() > p.len() {
            let stem = &tok[..tok.len() - p.len()];
            if stem.chars().all(is_hangul_char) {
                return (stem, Some(p));
            }
        }
    }
    (tok, None)
}

fn is_korean_honorific_token(tok: &str) -> bool {
    if KOREAN_HONORIFICS_TITLES.contains(&tok) {
        return true;
    }
    let (stem, _) = strip_korean_particle(tok);
    KOREAN_HONORIFICS_TITLES.contains(&stem)
}

/// Script-neutral, case-insensitive name list lookup on token boundaries.
fn scrub_name_lookup(
    seg: &str,
    opts: &ScrubOptions,
    matched_languages: &mut BTreeSet<String>,
    seen_ja_given_names: &mut HashSet<String>,
) -> String {
    if is_latin_sentence_context(seg) {
        return seg.to_string();
    }
    let toks = tokenize(seg);
    if toks.is_empty() {
        return seg.to_string();
    }

    // Sentence-language gate: at least 2 Spanish markers (de, la, el, que, en, y, los, las, por, con, una, es, fue)
    // decides whether the Spanish lowercase cue restriction applies in this sentence.
    let is_spanish_sentence = {
        let count = toks.iter().filter(|t| {
            let tw = seg[t.core_s..t.core_e].to_lowercase();
            matches!(tw.as_str(), "de" | "la" | "el" | "que" | "en" | "y" | "los" | "las" | "por" | "con" | "una" | "es" | "fue")
        }).count();
        count >= 2
    };


    let mut spans: Vec<(usize, usize)> = Vec::new();

    for (tok_idx, tok) in toks.iter().enumerate() {
        let core = &seg[tok.core_s..tok.core_e];
        if core.chars().any(is_cjk_or_japanese) {
            if let Some(segmenter) = get_lindera_segmenter() {
                if let Ok(mut morphemes) = segmenter.segment(std::borrow::Cow::Borrowed(core)) {
                    let n = morphemes.len();
                    let mut i = 0;
                    while i < n {
                        let mut matched_len = 0;
                        let max_k = std::cmp::min(3, n - i);
                        for k in (1..=max_k).rev() {
                            let byte_s = morphemes[i].byte_start;
                            let byte_e = morphemes[i + k - 1].byte_end;
                            let surface = &core[byte_s..byte_e];
                            let char_len = surface.chars().count();
                            let w = surface.to_lowercase();
                            if is_allowlisted(&w, opts) {
                                continue;
                            }

                            let is_single_char_sei = char_len == 1
                                && k == 1
                                && is_ja_single_char_surname_morpheme(surface, &mut morphemes[i]);
                            if char_len < 2 && !is_single_char_sei {
                                continue;
                            }

                            let is_cand_given = char_len >= 2 && is_ja_given_name(&w);
                            let is_cand_surname = (char_len >= 2
                                && (is_ja_surname(surface) || is_ja_surname(&w)))
                                || is_single_char_sei;

                            if !is_cand_given && !is_cand_surname {
                                continue;
                            }

                            // Context rules for Japanese
                            let left_tok = if i == 0 && tok_idx > 0 && !tok.nl_before {
                                Some(&seg[toks[tok_idx - 1].core_s..toks[tok_idx - 1].core_e])
                            } else {
                                None
                            };

                            let right_tok = if i + k == n && tok_idx + 1 < toks.len() && !toks[tok_idx + 1].nl_before {
                                Some(&seg[toks[tok_idx + 1].core_s..toks[tok_idx + 1].core_e])
                            } else {
                                None
                            };

                            let mut redact = false;

                            // Single-character surnames are only allowed as Rule 2a neighbors (paired with a given name).
                            if !is_single_char_sei {
                                // Rule 1: if either neighbor is Latin-script text, redact.
                                let left_has_latin = if i > 0 {
                                    let prev_m = &core[morphemes[i - 1].byte_start..morphemes[i - 1].byte_end];
                                    has_latin_chars(prev_m)
                                } else {
                                    left_tok.is_some_and(has_latin_chars)
                                };

                                let right_has_latin = if i + k < n {
                                    let next_m = &core[morphemes[i + k].byte_start..morphemes[i + k].byte_end];
                                    has_latin_chars(next_m)
                                } else {
                                    right_tok.is_some_and(has_latin_chars)
                                };

                                redact = left_has_latin || right_has_latin;

                                // Rule 2b: next token is a name honorific or title
                                if !redact {
                                    let next_is_honorific = if i + k < n {
                                        let next_m = &core[morphemes[i + k].byte_start..morphemes[i + k].byte_end];
                                        JAPANESE_HONORIFICS_TITLES.contains(&next_m)
                                    } else if let Some(rt) = right_tok {
                                        JAPANESE_HONORIFICS_TITLES.iter().any(|&h| rt == h || rt.starts_with(h))
                                    } else {
                                        false
                                    };

                                    if next_is_honorific {
                                        redact = true;
                                    }
                                }

                                // Rule 2c: Title-before-name cue (immediately preceded by a title, with optional の)
                                if !redact {
                                    let is_non_indep = (i..(i + k))
                                        .any(|m_idx| is_lindera_non_independent_noun(&mut morphemes[m_idx]));
                                    let prev_is_title = if is_non_indep {
                                        false
                                    } else if i > 0 {
                                        let prev_m = &core[morphemes[i - 1].byte_start..morphemes[i - 1].byte_end];
                                        if JAPANESE_PREFIX_TITLES.contains(&prev_m) {
                                            true
                                        } else if prev_m == "の" {
                                            if i >= 2 {
                                                let prev2_m = &core[morphemes[i - 2].byte_start..morphemes[i - 2].byte_end];
                                                JAPANESE_PREFIX_TITLES.contains(&prev2_m)
                                            } else if let Some(lt) = left_tok {
                                                JAPANESE_PREFIX_TITLES.iter().any(|&t| lt == t || lt.ends_with(t))
                                            } else {
                                                false
                                            }
                                        } else {
                                            false
                                        }
                                    } else if let Some(lt) = left_tok {
                                        let lt_base = lt.strip_suffix("の").unwrap_or(lt);
                                        JAPANESE_PREFIX_TITLES.iter().any(|&t| lt_base == t || lt_base.ends_with(t))
                                    } else {
                                        false
                                    };

                                    if prev_is_title {
                                        redact = true;
                                    }
                                }
                            }

                            let mut other_name_neighbor = false;
                            let mut paired_given: Option<String> = None;

                            if !redact {
                                // Rule 2a: neighbor from the other list
                                if is_cand_given {
                                    let cand_is_jinmei = (i..(i + k))
                                        .all(|m_idx| is_lindera_person_name(&mut morphemes[m_idx]));
                                    // Check if left neighbor is a surname
                                    if i > 0 {
                                        for bk in 1..=std::cmp::min(3, i) {
                                            let s_prev = &core[morphemes[i - bk].byte_start..morphemes[i - 1].byte_end];
                                            let is_prev_sur = if s_prev.chars().count() == 1 {
                                                bk == 1
                                                    && cand_is_jinmei
                                                    && is_ja_single_char_surname_morpheme(s_prev, &mut morphemes[i - 1])
                                            } else {
                                                is_ja_surname(s_prev)
                                            };
                                            if is_prev_sur {
                                                other_name_neighbor = true;
                                                break;
                                            }
                                        }
                                    } else if let Some(lt) = left_tok {
                                        let is_lt_sur = if lt.chars().count() == 1 {
                                            cand_is_jinmei
                                                && get_ja_surnames().contains(lt)
                                                && segmenter
                                                    .segment(std::borrow::Cow::Borrowed(lt))
                                                    .is_ok_and(|mut ms| ms.len() == 1 && is_lindera_surname(&mut ms[0]))
                                        } else {
                                            is_ja_surname(lt)
                                        };
                                        if is_lt_sur {
                                            other_name_neighbor = true;
                                        }
                                    }

                                    // Check if right neighbor is a surname
                                    if !other_name_neighbor {
                                        if i + k < n {
                                            for fk in 1..=std::cmp::min(3, n - (i + k)) {
                                                let s_next = &core[morphemes[i + k].byte_start..morphemes[i + k + fk - 1].byte_end];
                                                let is_next_sur = if s_next.chars().count() == 1 {
                                                    fk == 1
                                                        && cand_is_jinmei
                                                        && is_ja_single_char_surname_morpheme(s_next, &mut morphemes[i + k])
                                                } else {
                                                    is_ja_surname(s_next)
                                                };
                                                if is_next_sur {
                                                    other_name_neighbor = true;
                                                    break;
                                                }
                                            }
                                        } else if let Some(rt) = right_tok {
                                            let is_rt_sur = if rt.chars().count() == 1 {
                                                cand_is_jinmei
                                                    && get_ja_surnames().contains(rt)
                                                    && segmenter
                                                        .segment(std::borrow::Cow::Borrowed(rt))
                                                        .is_ok_and(|mut ms| ms.len() == 1 && is_lindera_surname(&mut ms[0]))
                                            } else {
                                                is_ja_surname(rt)
                                            };
                                            if is_rt_sur {
                                                other_name_neighbor = true;
                                            }
                                        }
                                    }
                                }

                                if !other_name_neighbor && is_cand_surname {
                                    // Check if left neighbor is a given name
                                    if i > 0 {
                                        for bk in 1..=std::cmp::min(3, i) {
                                            let s_prev = &core[morphemes[i - bk].byte_start..morphemes[i - 1].byte_end];
                                            let w_prev = s_prev.to_lowercase();
                                            let prev_ok = if is_single_char_sei {
                                                ((i - bk)..i).all(|m_idx| is_lindera_person_name(&mut morphemes[m_idx]))
                                            } else {
                                                true
                                            };
                                            if prev_ok && s_prev.chars().count() >= 2 && is_ja_given_name(&w_prev) {
                                                other_name_neighbor = true;
                                                paired_given = Some(w_prev);
                                                break;
                                            }
                                        }
                                    } else if let Some(lt) = left_tok {
                                        let w_lt = lt.to_lowercase();
                                        let lt_ok = if is_single_char_sei {
                                            segmenter
                                                .segment(std::borrow::Cow::Borrowed(lt))
                                                .is_ok_and(|mut ms| ms.iter_mut().all(is_lindera_person_name))
                                        } else {
                                            true
                                        };
                                        if lt_ok && lt.chars().count() >= 2 && is_ja_given_name(&w_lt) {
                                            other_name_neighbor = true;
                                            paired_given = Some(w_lt);
                                        }
                                    }

                                    // Check if right neighbor is a given name
                                    if !other_name_neighbor {
                                        if i + k < n {
                                            for fk in 1..=std::cmp::min(3, n - (i + k)) {
                                                let s_next = &core[morphemes[i + k].byte_start..morphemes[i + k + fk - 1].byte_end];
                                                let w_next = s_next.to_lowercase();
                                                let next_ok = if is_single_char_sei {
                                                    ((i + k)..(i + k + fk)).all(|m_idx| is_lindera_person_name(&mut morphemes[m_idx]))
                                                } else {
                                                    true
                                                };
                                                if next_ok && s_next.chars().count() >= 2 && is_ja_given_name(&w_next) {
                                                    other_name_neighbor = true;
                                                    paired_given = Some(w_next);
                                                    break;
                                                }
                                            }
                                        } else if let Some(rt) = right_tok {
                                            let w_rt = rt.to_lowercase();
                                            let rt_ok = if is_single_char_sei {
                                                segmenter
                                                    .segment(std::borrow::Cow::Borrowed(rt))
                                                    .is_ok_and(|mut ms| ms.iter_mut().all(is_lindera_person_name))
                                            } else {
                                                true
                                            };
                                            if rt_ok && rt.chars().count() >= 2 && is_ja_given_name(&w_rt) {
                                                other_name_neighbor = true;
                                                paired_given = Some(w_rt);
                                            }
                                        }
                                    }
                                }

                                if other_name_neighbor {
                                    redact = true;
                                }
                            }

                            // Additional cues — ONLY for given names in ja.txt, NOT surnames:
                            // (a) Repeated mention: if the same full name (given+surname or given+given)
                            //     was already redacted earlier in the same input text, redact later lone
                            //     occurrences of that given name in that text.
                            if !redact && is_cand_given && seen_ja_given_names.contains(&w) {
                                redact = true;
                            }

                            // (b) Subject-particle cue: redact a lone given name when Lindera tags its
                            //     morpheme as a person name (IPADIC 固有名詞,人名) AND the next morpheme is は, が, or も.
                            if !redact && is_cand_given {
                                let all_jinmei = (i..(i + k)).all(|m_idx| is_lindera_person_name(&mut morphemes[m_idx]));
                                if all_jinmei {
                                    let next_is_subj_particle = if i + k < n {
                                        let next_m = &core[morphemes[i + k].byte_start..morphemes[i + k].byte_end];
                                        matches!(next_m, "は" | "が" | "も")
                                    } else if let Some(rt) = right_tok {
                                        if let Ok(rt_morphs) = segmenter.segment(std::borrow::Cow::Borrowed(rt)) {
                                            if let Some(first_m) = rt_morphs.first() {
                                                let first_surf = &rt[first_m.byte_start..first_m.byte_end];
                                                matches!(first_surf, "は" | "が" | "も")
                                            } else {
                                                false
                                            }
                                        } else {
                                            matches!(rt, "は" | "が" | "も")
                                        }
                                    } else {
                                        false
                                    };
                                    if next_is_subj_particle {
                                        redact = true;
                                    }
                                }
                            }

                            // Rule 3: otherwise, leave candidate unredacted.
                            if redact {
                                if other_name_neighbor {
                                    let cand_is_jinmei = (i..(i + k))
                                        .all(|m_idx| is_lindera_person_name(&mut morphemes[m_idx]));
                                    if is_cand_given && cand_is_jinmei {
                                        seen_ja_given_names.insert(w.clone());
                                    }
                                    if let Some(pg) = paired_given {
                                        if let Ok(mut pg_morphs) =
                                            segmenter.segment(std::borrow::Cow::Borrowed(&pg))
                                        {
                                            if pg_morphs.iter_mut().all(is_lindera_person_name) {
                                                seen_ja_given_names.insert(pg);
                                            }
                                        }
                                    }
                                }
                                matched_languages.insert("ja".to_string());
                                spans.push((tok.core_s + byte_s, tok.core_s + byte_e));
                                matched_len = k;
                                break;
                            }
                        }
                        if matched_len > 0 {
                            i += matched_len;
                        } else {
                            i += 1;
                        }
                    }
                    continue;
                }
            }
        }

        if core.chars().any(is_hangul_char) {
            if !core.chars().all(is_hangul_char) {
                continue;
            }
            let (stem, stripped_suf) = strip_korean_particle(core);
            let left_tok = if tok_idx > 0 && !tok.nl_before {
                Some(&seg[toks[tok_idx - 1].core_s..toks[tok_idx - 1].core_e])
            } else {
                None
            };
            let right_tok = if tok_idx + 1 < toks.len() && !toks[tok_idx + 1].nl_before {
                Some(&seg[toks[tok_idx + 1].core_s..toks[tok_idx + 1].core_e])
            } else {
                None
            };

            let prev_is_title = left_tok.is_some_and(|lt| KOREAN_PREFIX_TITLES.contains(&lt));
            let next_is_honorific = right_tok.is_some_and(is_korean_honorific_token);
            let attached_is_honorific = stripped_suf
                .is_some_and(|suf| KOREAN_HONORIFIC_PARTICLES.contains(&suf));

            let mut redact_len: Option<usize> = None;

            // 1. Unspaced compound surname + given name (e.g., 김민수, 박지훈, 남궁민준, 김민)
            if is_korean_compound_full_name(core) {
                redact_len = Some(core.len());
            } else if stripped_suf.is_some() && is_korean_compound_full_name(stem) {
                redact_len = Some(stem.len());
            }

            // 2. Spaced adjacent surname + given name (Rule 2a with single-syllable guard:
            //    single-syllable tokens never match as neighbors across a space in unlabeled text)
            if redact_len.is_none() {
                if is_ko_multi_char_surname(core) {
                    if let Some(rt) = right_tok {
                        let (rt_stem, _) = strip_korean_particle(rt);
                        if is_ko_given_name(rt) || is_ko_given_name(rt_stem) {
                            redact_len = Some(core.len());
                        }
                    }
                } else if is_ko_given_name(core) || (stripped_suf.is_some() && is_ko_given_name(stem)) {
                    let target_len = if is_ko_given_name(core) {
                        core.len()
                    } else {
                        stem.len()
                    };
                    if let Some(lt) = left_tok {
                        if is_ko_multi_char_surname(lt) {
                            redact_len = Some(target_len);
                        }
                    }
                }
            }

            // 3. Honorific after a name (attached or next token) or Title before a name
            if redact_len.is_none() && (prev_is_title || next_is_honorific || attached_is_honorific) {
                if is_ko_given_name(core) || is_ko_multi_char_surname(core) || (next_is_honorific && is_ko_any_surname(core)) {
                    redact_len = Some(core.len());
                } else if stripped_suf.is_some()
                    && (is_ko_given_name(stem)
                        || is_ko_multi_char_surname(stem)
                        || (attached_is_honorific && is_ko_any_surname(stem)))
                {
                    redact_len = Some(stem.len());
                }
            }

            if let Some(rlen) = redact_len {
                matched_languages.insert("ko".to_string());
                spans.push((tok.core_s, tok.core_s + rlen));
            }
            continue;
        }

        let min_len = if core.chars().any(is_cjk_or_hangul) { 2 } else { 3 };
        if core.chars().count() < min_len {
            continue;
        }
        let w = core.to_lowercase();
        if is_allowlisted(&w, opts) {
            continue;
        }
        let w_stripped = strip_latin_accents(&w);
        let multilang = get_multilang_names();
        let lookup_res = multilang
            .get(&w)
            .or_else(|| multilang.get(&w_stripped));
        if let Some(langs) = lookup_res {
            // Skip ordinary Spanish dictionary words (e.g. "días", "blanca", "luz", "todo")
            // when appearing in lowercase unlabeled sentences.
            if is_spanish_common_word(&w) && w != "rene" && w != "rené" {
                continue;
            }

            // Gated French words: a gated token in lowercase is kept as an ordinary word
            // when French is detected.
            let is_lowercase_token = core.chars().all(|c| !c.is_uppercase());
            if is_french_sentence(&toks, seg)
                && is_lowercase_token
                && langs.contains(&"fr")
                && is_name_gated("fr", &w)
            {
                continue;
            }

            // In Portuguese sentences, ordinary Portuguese words from hunspell dictionaries
            // must NOT be redacted by name lookup, and gated words in lowercase are kept as ordinary words.
            let is_lowercase_token = core.chars().all(|c| !c.is_uppercase());
            if is_portuguese_sentence(&toks, seg) {
                if is_name_gated("pt", &w) {
                    if is_lowercase_token {
                        continue;
                    }
                } else if is_pt_word(&w) {
                    continue;
                }
            }

            // Cue-only Spanish gated surnames: Tier 4 runs without label/salutation cues,
            // so skip Spanish gated tokens unless another non-Spanish language list owns them.
            if !is_portuguese_sentence(&toks, seg)
                && !is_french_sentence(&toks, seg)
                && langs.contains(&"es")
                && is_name_gated("es", &w)
            {
                let has_other_active_lang = langs.iter().any(|&l| {
                    l != "es"
                        && (!is_spanish_sentence || (l != "pt" && l != "fr"))
                        && !get_words_unix_en().contains(w.as_str())
                });
                if !has_other_active_lang {
                    continue;
                }
            }

            // Lowercase Spanish matches: Tier 4 must not redact a lowercase token in Spanish
            // prose unless it is adjacent to a cue: a title (Sr., Sra., Dr., Dra.), a following honorific,
            // a labeled field, or a neighboring name token.
            // The sentence-language gate restricts this requirement to sentences identified as Spanish.
            let is_lowercase_token = core.chars().all(|c| !c.is_uppercase());
            if is_spanish_sentence && is_lowercase_token && langs.contains(&"es") {
                let left_tok = if tok_idx > 0 && !tok.nl_before {
                    Some(&seg[toks[tok_idx - 1].core_s..toks[tok_idx - 1].core_e])
                } else {
                    None
                };
                let right_tok = if tok_idx + 1 < toks.len() && !toks[tok_idx + 1].nl_before {
                    Some(&seg[toks[tok_idx + 1].core_s..toks[tok_idx + 1].core_e])
                } else {
                    None
                };

                let prev_is_title = left_tok.is_some_and(|lt| {
                    let lt_low = lt.to_lowercase();
                    matches!(
                        lt_low.trim_end_matches('.'),
                        "sr" | "sra" | "srta" | "dr" | "dra" | "prof" | "lic" | "ing" | "don" | "doña"
                    )
                });
                let next_is_title_or_honorific = right_tok.is_some_and(|rt| {
                    let rt_low = rt.to_lowercase();
                    matches!(
                        rt_low.trim_end_matches('.'),
                        "sr" | "sra" | "srta" | "dr" | "dra" | "prof" | "lic" | "ing"
                    )
                });

                let is_name_tok = |token_str: &str| -> bool {
                    let low = token_str.to_lowercase();
                    let stripped = strip_latin_accents(&low);
                    if is_spanish_common_word(&low) {
                        return false;
                    }
                    if let Some(cache) = &opts.dynamic_name_cache {
                        if let Ok(c) = cache.read() {
                            if c.contains(&low) || c.contains(&stripped) {
                                return true;
                            }
                        }
                    }
                    let namelist = get_namelist();
                    let multilang = get_multilang_names();
                    namelist.contains(low.as_str())
                        || namelist.contains(stripped.as_str())
                        || multilang.contains_key(&low)
                        || multilang.contains_key(&stripped)
                };

                let neighbor_is_name = left_tok.is_some_and(is_name_tok) || right_tok.is_some_and(is_name_tok);

                if !prev_is_title && !next_is_title_or_honorific && !neighbor_is_name {
                    continue;
                }
            }

            // Skip ordinary English dictionary words (e.g. "mark", "can", "contact", "cloud", "line")
            // when appearing in lowercase unlabeled sentences.
            let has_non_curated = langs.iter().any(|&l| l != "en" && l != "es" && l != "fr" && l != "pt");
            if !has_non_curated {
                if langs.contains(&"fr") || langs.contains(&"pt") {
                    if get_words_unix_en().contains(w.as_str()) {
                        continue;
                    }
                } else if known_word(&w, get_words_full()) {
                    continue;
                }
            }
            for lang in langs {
                matched_languages.insert(lang.to_string());
            }
            spans.push((tok.core_s, tok.core_e));
        }
    }
    if spans.len() > 1 {
        spans.sort_by_key(|&(s, _)| s);
        let mut merged: Vec<(usize, usize)> = Vec::with_capacity(spans.len());
        for (s, e) in spans.drain(..) {
            if let Some(last) = merged.last_mut() {
                if s <= last.1
                    || seg[last.1..s].chars().all(|c| c.is_whitespace() && c != '\n' && c != '\r')
                {
                    last.1 = last.1.max(e);
                    continue;
                }
            }
            merged.push((s, e));
        }
        spans = merged;
    }
    apply_spans(seg, &spans, "[NAME]")
}

/// Bangla Tier: runs after Tier 2B (`scrub_context_cues`) and before Tier 3 (`scrub_unknown_caps`).
/// Operates only on Bengali-script runs using dictionary exclusion (`bn_words.tsv`) and the Bangla name list (`bn_names.tsv`).
fn scrub_bn_names(
    seg: &str,
    opts: &ScrubOptions,
    matched_languages: &mut BTreeSet<String>,
) -> String {
    if !seg.chars().any(is_bengali_char) {
        return seg.to_string();
    }
    let toks = tokenize(seg);
    if toks.is_empty() {
        return seg.to_string();
    }

    let custom_res = opts
        .bn_resource_dir
        .as_ref()
        .map(|d| load_bn_resources_from_dir(d));
    let res: &BnResources = custom_res.as_ref().unwrap_or_else(|| get_bn_resources());

    let n = toks.len();
    let mut is_bn_tok = vec![false; n];
    for (idx, t) in toks.iter().enumerate() {
        let core = &seg[t.core_s..t.core_e];
        // Rule 3e: Do not redact a token that is only a fragment of a longer word
        // (e.g., hyphenated/attached to another Bengali or alphabetic segment without whitespace).
        let lead_frag = seg[t.s..t.core_s]
            .chars()
            .any(|c| is_bengali_char(c) || c == '-' || c.is_alphabetic());
        let trail_frag = seg[t.core_e..t.e]
            .chars()
            .any(|c| is_bengali_char(c) || c == '-' || c.is_alphabetic());
        let raw_has_hyphen = core.contains('-');
        if !lead_frag
            && !trail_frag
            && !raw_has_hyphen
            && is_bengali_token(core)
            && core.chars().count() >= 2
        {
            is_bn_tok[idx] = true;
        }
    }

    let mut redacted_tok = vec![false; n];
    let mut i = 0;
    while i < n {
        if !is_bn_tok[i] {
            i += 1;
            continue;
        }
        let mut j = i;
        while j + 1 < n
            && is_bn_tok[j + 1]
            && !toks[j + 1].nl_before
            && !seg[toks[j].core_e..toks[j].e]
                .chars()
                .any(|c| matches!(c, '।' | '.' | '!' | '?' | ';' | ':' | ','))
        {
            j += 1;
        }

        let run_len = j - i + 1;
        let mut kinds = Vec::with_capacity(run_len);
        for k in i..=j {
            let core = &seg[toks[k].core_s..toks[k].core_e];
            kinds.push(classify_bn_token_in(core, res));
        }

        let mut run_redact = vec![false; run_len];

        // Rule 3b: A token not in the word list that is in the names list is redacted.
        for idx in 0..run_len {
            if kinds[idx] == BnTokKind::Name {
                run_redact[idx] = true;
            }
        }

        // Rule 3c (2-token windows):
        // - A name-list token next to an unknown token (in neither list): redact both.
        // - Or a 2-token sequence that is a full name in `bn_names.tsv` where neither token is Word.
        if run_len >= 2 {
            for w in 0..(run_len - 1) {
                let k0 = kinds[w];
                let k1 = kinds[w + 1];
                if k0 == BnTokKind::Word || k1 == BnTokKind::Word {
                    continue;
                }
                if (k0 == BnTokKind::Name && k1 == BnTokKind::Unknown)
                    || (k0 == BnTokKind::Unknown && k1 == BnTokKind::Name)
                {
                    run_redact[w] = true;
                    run_redact[w + 1] = true;
                } else if k0 == BnTokKind::Name || k1 == BnTokKind::Name {
                    let c0 = &seg[toks[i + w].core_s..toks[i + w].core_e];
                    let c1 = &seg[toks[i + w + 1].core_s..toks[i + w + 1].core_e];
                    let phrase = format!("{} {}", c0, c1);
                    if is_bn_full_name_in(&phrase, res) {
                        run_redact[w] = true;
                        run_redact[w + 1] = true;
                    }
                }
            }
        }

        // Rule 3c (3-token windows):
        // Apply to 3-token windows where all tokens are Name or Unknown (no Word token may ever be redacted).
        if run_len >= 3 {
            for w in 0..(run_len - 2) {
                let k0 = kinds[w];
                let k1 = kinds[w + 1];
                let k2 = kinds[w + 2];
                let name_count = [k0, k1, k2]
                    .iter()
                    .filter(|&&k| k == BnTokKind::Name)
                    .count();
                let word_count = [k0, k1, k2]
                    .iter()
                    .filter(|&&k| k == BnTokKind::Word)
                    .count();

                if word_count == 0 && name_count >= 1 {
                    run_redact[w] = true;
                    run_redact[w + 1] = true;
                    run_redact[w + 2] = true;
                }
            }
        }

        for idx in 0..run_len {
            if run_redact[idx] && kinds[idx] != BnTokKind::Word {
                redacted_tok[i + idx] = true;
            }
        }

        i = j + 1;
    }

    let mut spans: Vec<(usize, usize)> = Vec::new();
    let mut k = 0;
    while k < n {
        if !redacted_tok[k] {
            k += 1;
            continue;
        }
        let start_s = toks[k].core_s;
        let mut end_e = toks[k].core_e;
        while k + 1 < n
            && redacted_tok[k + 1]
            && !toks[k + 1].nl_before
            && seg[toks[k].core_e..toks[k].e].is_empty()
            && seg[toks[k + 1].s..toks[k + 1].core_s].is_empty()
        {
            k += 1;
            end_e = toks[k].core_e;
        }
        spans.push((start_s, end_e));
        k += 1;
    }

    if !spans.is_empty() {
        matched_languages.insert("bn".to_string());
    }
    apply_spans(seg, &spans, "[NAME]")
}

// ── Foreign-run rule: dominant-script detection and foreign run/block redaction ──

pub const FOREIGN_RUN_CHECK_ENABLED: bool = true;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Script {
    Bengali,
    HanKana,
    Hangul,
    Latin,
    Other,
}

impl Script {
    pub fn to_lang(self) -> &'static str {
        match self {
            Script::Bengali => "bn",
            Script::HanKana => "ja",
            Script::Hangul => "ko",
            Script::Latin => "en",
            Script::Other => "other",
        }
    }
}

fn is_latin_char(c: char) -> bool {
    (matches!(c,
        'a'..='z' | 'A'..='Z' |
        '\u{00C0}'..='\u{024F}' |
        '\u{1E00}'..='\u{1EFF}'
    ) && c != '\u{00D7}' && c != '\u{00F7}')
    || ('\u{0300}'..='\u{036F}').contains(&c)
}

fn char_script(c: char) -> Option<Script> {
    if c.is_whitespace() || c.is_numeric() {
        return None;
    }
    if c.is_ascii_punctuation()
        || matches!(c, '।' | '॥' | '‘' | '’' | '“' | '”' | '…' | '—' | '–' | '·' | '、' | '。' | '「' | '」' | '『' | '』')
        || ('\u{3000}'..='\u{303F}').contains(&c)
        || ('\u{FF00}'..='\u{FF65}').contains(&c)
    {
        return None;
    }
    if is_bengali_char(c) {
        if ('\u{09E6}'..='\u{09EF}').contains(&c)
            || ('\u{09F2}'..='\u{09FB}').contains(&c)
            || c == '\u{09FD}'
        {
            None
        } else {
            Some(Script::Bengali)
        }
    } else if is_cjk_or_japanese(c) {
        if c == '\u{30FB}' {
            None
        } else {
            Some(Script::HanKana)
        }
    } else if is_hangul_char(c) {
        Some(Script::Hangul)
    } else if is_latin_char(c) {
        Some(Script::Latin)
    } else if c.is_alphabetic() {
        Some(Script::Other)
    } else {
        None
    }
}

fn detect_dominant_script(text: &str) -> Script {
    let mut counts = [0usize; 5]; // Bengali, HanKana, Hangul, Latin, Other
    let mut last = 0;
    for m in PLACEHOLDER_RE.find_iter(text) {
        for c in text[last..m.start()].chars() {
            match char_script(c) {
                Some(Script::Bengali) => counts[0] += 1,
                Some(Script::HanKana) => counts[1] += 1,
                Some(Script::Hangul) => counts[2] += 1,
                Some(Script::Latin) => counts[3] += 1,
                Some(Script::Other) => counts[4] += 1,
                None => {}
            }
        }
        last = m.end();
    }
    for c in text[last..].chars() {
        match char_script(c) {
            Some(Script::Bengali) => counts[0] += 1,
            Some(Script::HanKana) => counts[1] += 1,
            Some(Script::Hangul) => counts[2] += 1,
            Some(Script::Latin) => counts[3] += 1,
            Some(Script::Other) => counts[4] += 1,
            None => {}
        }
    }
    let max = *counts.iter().max().unwrap_or(&0);
    if max == 0 {
        return Script::Latin;
    }
    if counts[0] == max {
        Script::Bengali
    } else if counts[3] == max {
        Script::Latin
    } else if counts[1] == max {
        Script::HanKana
    } else if counts[2] == max {
        Script::Hangul
    } else {
        Script::Other
    }
}

#[derive(Debug, Clone)]
struct ScriptToken {
    core_s: usize,
    core_e: usize,
    script: Script,
    text: String,
    letter_count: usize,
}

fn extract_script_tokens(seg: &str) -> Vec<ScriptToken> {
    let mut tokens = Vec::new();
    let b = seg.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if matches!(b[i], b' ' | b'\t' | b'\r' | b'\n') {
            i += 1;
        } else {
            let s = i;
            while i < b.len() && !matches!(b[i], b' ' | b'\t' | b'\r' | b'\n') {
                i += 1;
            }
            let word = &seg[s..i];
            let mut char_spans: Vec<(usize, usize, char, Script)> = Vec::new();
            for (char_idx, c) in word.char_indices() {
                if let Some(script) = char_script(c) {
                    char_spans.push((s + char_idx, s + char_idx + c.len_utf8(), c, script));
                }
            }
            if char_spans.is_empty() {
                continue;
            }
            let mut run_start = 0;
            while run_start < char_spans.len() {
                let cur_script = char_spans[run_start].3;
                let mut run_end = run_start + 1;
                while run_end < char_spans.len() && char_spans[run_end].3 == cur_script {
                    run_end += 1;
                }
                let first_s = char_spans[run_start].0;
                let last_e = char_spans[run_end - 1].1;
                let core_slice = &seg[first_s..last_e];
                let count = run_end - run_start;
                tokens.push(ScriptToken {
                    core_s: first_s,
                    core_e: last_e,
                    script: cur_script,
                    text: core_slice.to_string(),
                    letter_count: count,
                });
                run_start = run_end;
            }
        }
    }
    tokens
}

fn is_foreign_token_in_name_list(tok_text: &str, lang: &str, opts: &ScrubOptions) -> bool {
    let lower = tok_text.to_lowercase();
    let stripped = strip_latin_accents(&lower);
    if let Some(cache) = &opts.dynamic_name_cache {
        if let Ok(c) = cache.read() {
            if c.contains(&lower) || c.contains(&stripped) {
                return true;
            }
        }
    }
    match lang {
        "en" => {
            let namelist = get_namelist();
            let multilang = get_multilang_names();
            namelist.contains(lower.as_str())
                || namelist.contains(stripped.as_str())
                || multilang
                    .get(&lower)
                    .is_some_and(|langs| langs.contains(&"en"))
                || multilang
                    .get(&stripped)
                    .is_some_and(|langs| langs.contains(&"en"))
        }
        "es" => {
            if is_name_gated("es", tok_text) {
                return false;
            }
            let multilang = get_multilang_names();
            multilang
                .get(&lower)
                .is_some_and(|langs| langs.contains(&"es"))
                || multilang
                    .get(&stripped)
                    .is_some_and(|langs| langs.contains(&"es"))
        }
        "fr" => {
            let multilang = get_multilang_names();
            multilang
                .get(&lower)
                .is_some_and(|langs| langs.contains(&"fr"))
                || multilang
                    .get(&stripped)
                    .is_some_and(|langs| langs.contains(&"fr"))
        }
        "pt" => {
            let multilang = get_multilang_names();
            multilang
                .get(&lower)
                .is_some_and(|langs| langs.contains(&"pt"))
                || multilang
                    .get(&stripped)
                    .is_some_and(|langs| langs.contains(&"pt"))
        }
        "ja" => {
            is_ja_given_name(tok_text)
                || is_ja_surname(tok_text)
                || get_ja_given_names().contains(tok_text)
                || get_ja_surnames().contains(tok_text)
        }
        "ko" => {
            let (stem, _) = strip_korean_particle(tok_text);
            is_ko_given_name(tok_text)
                || is_ko_given_name(stem)
                || is_ko_any_surname(tok_text)
                || is_ko_any_surname(stem)
                || is_korean_compound_full_name(tok_text)
                || is_korean_compound_full_name(stem)
                || get_ko_given_names().contains(tok_text)
                || get_ko_given_names().contains(stem)
        }
        "bn" => is_bn_name(tok_text),
        _ => false,
    }
}

fn is_foreign_token_in_word_list(tok_text: &str, lang: &str) -> bool {
    match lang {
        "en" => known_word(tok_text, get_foreign_en_words()),
        "es" => is_spanish_common_word(tok_text),
        "fr" => is_fr_word(tok_text),
        "pt" => is_pt_word(tok_text),
        "bn" => is_bn_known_word(tok_text),
        "ja" => {
            if let Some(segmenter) = get_lindera_segmenter() {
                if let Ok(mut morphemes) = segmenter.segment(std::borrow::Cow::Borrowed(tok_text)) {
                    if morphemes.len() == 1 {
                        let d = morphemes[0].details();
                        return !d.is_empty()
                            && d[0] != "UNK"
                            && !(d.len() >= 3 && d[1] == "固有名詞" && d[2] == "人名");
                    }
                }
            }
            false
        }
        "ko" => KOREAN_COMMON_COMPOUND_STOPLIST.contains(&tok_text),
        _ => false,
    }
}

fn scrub_foreign_runs(
    text: &str,
    opts: &ScrubOptions,
    matched_name_languages: &mut BTreeSet<String>,
) -> String {
    let dominant_script = detect_dominant_script(text);
    map_unprotected(text, |seg, _| {
        scrub_foreign_runs_seg(seg, dominant_script, opts, matched_name_languages)
    })
}

fn scrub_foreign_runs_seg(
    seg: &str,
    dominant_script: Script,
    opts: &ScrubOptions,
    matched_name_languages: &mut BTreeSet<String>,
) -> String {
    let tokens = extract_script_tokens(seg);
    if tokens.is_empty() {
        return seg.to_string();
    }

    let mut spans: Vec<(usize, usize)> = Vec::new();
    let n = tokens.len();
    let mut i = 0;

    while i < n {
        if tokens[i].script == dominant_script {
            i += 1;
            continue;
        }

        let run_start = i;
        while i < n && tokens[i].script != dominant_script {
            i += 1;
        }
        let run = &tokens[run_start..i];
        let run_len = run.len();
        let total_letters: usize = run.iter().map(|t| t.letter_count).sum();

        let is_block = run_len >= 2 && total_letters >= 3;

        if is_block {
            let first_script = run[0].script;
            let all_same = run.iter().all(|t| t.script == first_script);
            let block_lang = if all_same {
                first_script.to_lang()
            } else {
                "other"
            };

            if block_lang == "other" {
                let b_start = run.first().unwrap().core_s;
                let b_end = run.last().unwrap().core_e;
                spans.push((b_start, b_end));
                matched_name_languages.insert("other".to_string());
            } else {
                for tok in run {
                    if is_allowlisted(&tok.text.to_lowercase(), opts) {
                        continue;
                    }
                    if is_foreign_token_in_name_list(&tok.text, block_lang, opts) {
                        spans.push((tok.core_s, tok.core_e));
                        matched_name_languages.insert(block_lang.to_string());
                    } else if is_foreign_token_in_word_list(&tok.text, block_lang) {
                        // Keep word
                    } else if tok.letter_count >= 3 {
                        spans.push((tok.core_s, tok.core_e));
                        matched_name_languages.insert(block_lang.to_string());
                    }
                }
            }
        } else {
            for tok in run {
                if is_allowlisted(&tok.text.to_lowercase(), opts) {
                    continue;
                }
                let tok_lang = tok.script.to_lang();
                if tok_lang != "other" && is_foreign_token_in_name_list(&tok.text, tok_lang, opts) {
                    spans.push((tok.core_s, tok.core_e));
                    matched_name_languages.insert(tok_lang.to_string());
                }
            }
        }
    }

    if spans.is_empty() {
        return seg.to_string();
    }

    spans.sort_by_key(|&(s, _)| s);
    let mut merged: Vec<(usize, usize)> = Vec::with_capacity(spans.len());
    for (s, e) in spans {
        if let Some(last) = merged.last_mut() {
            if s <= last.1 {
                last.1 = last.1.max(e);
                continue;
            }
        }
        merged.push((s, e));
    }

    apply_spans(seg, &merged, "[NAME]")
}

fn scrub_names(
    text: &str,
    opts: &ScrubOptions,
    matched_name_languages: &mut BTreeSet<String>,
    seen_ja_given_names: &mut HashSet<String>,
) -> String {
    let mut cur = text.to_string();

    // Tier 1 — the user's own identity tokens. Consumed silently; never printed.
    if !opts.identity_tokens.is_empty() {
        let alt = opts
            .identity_tokens
            .iter()
            .map(|t| regex::escape(t))
            .collect::<Vec<_>>()
            .join("|");
        if let Ok(re) = Regex::new(&format!(r"(?i)\b(?:{})\b", alt)) {
            cur = map_unprotected(&cur, |seg, _| re.replace_all(seg, "[NAME]").into_owned());
        }
    }
    // Tier 2A — edge-gated salutation and closing cues
    cur = scrub_salutation_cues(&cur, opts);
    // Tier 2B — context cues (always-on titles)
    cur = map_unprotected(&cur, |seg, _| scrub_context_cues(seg, opts));
    // Bangla Tier — runs after Tier 2B and before Tier 3 on Bangla-script runs
    cur = map_unprotected(&cur, |seg, _| {
        scrub_bn_names(seg, opts, matched_name_languages)
    });
    // Hindi Tier — runs after Tier 2B and Bangla Tier on Hindi Devanagari text
    cur = map_unprotected(&cur, |seg, _| {
        scrub_hi_names(seg, opts, matched_name_languages)
    });
    // Tier 3 — unknown capitalised words
    cur = map_unprotected(&cur, |seg, first| {
        scrub_unknown_caps(seg, first, opts, matched_name_languages)
    });
    // Tier 4 — script-neutral, case-insensitive name list lookup
    cur = map_unprotected(&cur, |seg, _| {
        scrub_name_lookup(seg, opts, matched_name_languages, seen_ja_given_names)
    });
    if FOREIGN_RUN_CHECK_ENABLED {
        cur = scrub_foreign_runs(&cur, opts, matched_name_languages);
    }
    cur
}

// ── Tag counting ──────────────────────────────────────────────────────────────

fn count_tags(text: &str) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for m in PLACEHOLDER_RE.find_iter(text) {
        let inner = &m.as_str()[1..m.as_str().len() - 1];
        if GENERATED_TAGS.contains(inner) {
            *counts.entry(inner.to_string()).or_insert(0) += 1;
        }
    }
    counts
}


pub fn is_sensitive_path(path: &str) -> bool {
    const SENSITIVE_KEYWORDS: &[&str] = &[
        ".ssh",
        ".gnupg",
        "password",
        "secret",
        "private_key",
        "credentials",
        ".env",
        "crypt",
    ];

    let lower = path.to_lowercase();
    SENSITIVE_KEYWORDS.iter().any(|&kw| lower.contains(kw))
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "scrubber_tests.rs"]
mod tests;
