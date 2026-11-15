// ──────────────────────────────────────────────────────────────────────────────
// pipeline/registry_check.rs — PII Identifier Knowledge Base: CSV parsing + validation
//
// IMPORTANT: this file must depend ONLY on `std` and `regex`. It is included from two places:
//   * `src-tauri/build.rs`            (so a broken id_formats.csv fails `cargo build`)
//   * `src-tauri/src/pipeline/scrubber.rs` (runtime loader and `--check-registry`)
// ──────────────────────────────────────────────────────────────────────────────
#![allow(dead_code)]

use regex::Regex;
use std::collections::HashSet;

/// Required CSV columns, in order. See plan section 4B.1.
pub const COLUMNS: [&str; 17] = [
    "id",
    "country",
    "authority",
    "identifier_type",
    "display_name",
    "type_display_name",
    "format",
    "context_keywords",
    "context_required",
    "checksum",
    "base_confidence",
    "valid_from",
    "valid_until",
    "verification_status",
    "source",
    "examples_ok",
    "examples_no",
];

/// Checksum names accepted in the `checksum` column.
pub const KNOWN_CHECKSUMS: [&str; 8] = [
    "none",
    "luhn",
    "verhoeff",
    "mod11",
    "mod11_nhs",
    "mod11_generic",
    "mod97",
    "aba",
];

pub const KNOWN_STATUSES: [&str; 3] = ["verified", "seeded", "unverified"];

#[derive(Debug, Clone)]
pub struct Row {
    pub line_no: usize,
    pub fields: Vec<String>,
}

impl Row {
    pub fn get(&self, column: &str) -> &str {
        match COLUMNS.iter().position(|c| *c == column) {
            Some(i) => self.fields.get(i).map(|s| s.as_str()).unwrap_or(""),
            None => "",
        }
    }
}

/// Split one CSV line into fields. Supports `"quoted, fields"` and `""` escapes.
pub fn split_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    cur.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                cur.push(c);
            }
        } else if c == '"' {
            in_quotes = true;
        } else if c == ',' {
            fields.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    fields.push(cur);
    fields
}

/// Parse the whole CSV text. Lines starting with `#` are comments. The first non-comment,
/// non-empty line must be the header and must equal `COLUMNS`.
pub fn parse_csv(text: &str) -> Result<Vec<Row>, String> {
    let mut rows = Vec::new();
    let mut header_seen = false;
    for (idx, raw_line) in text.lines().enumerate() {
        let line_no = idx + 1;
        let line = raw_line.trim_end_matches('\r');
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let fields = split_csv_line(line);
        if !header_seen {
            let header: Vec<&str> = fields.iter().map(|s| s.trim()).collect();
            if header != COLUMNS {
                return Err(format!(
                    "line {}: header does not match the expected columns.\n  expected: {}\n  found:    {}",
                    line_no,
                    COLUMNS.join(","),
                    header.join(",")
                ));
            }
            header_seen = true;
            continue;
        }
        if fields.len() != COLUMNS.len() {
            return Err(format!(
                "line {}: expected {} columns but found {} (a cell with a comma must be quoted)",
                line_no,
                COLUMNS.len(),
                fields.len()
            ));
        }
        rows.push(Row { line_no, fields });
    }
    if !header_seen {
        return Err("no header row found".to_string());
    }
    Ok(rows)
}

/// Expand one `format` cell (alternatives separated by `;`) into regex bodies.
///
/// Shorthand: `10D`, `2A`, `5X` = counted classes (D digit, A uppercase letter, X letter-or-digit).
/// Otherwise each char is a class (`#` digit, `A` uppercase letter, `X` letter-or-digit) or a literal.
/// `+` concatenates parts and allows one optional space/hyphen between them.
/// A cell starting with `re:` is used as a raw regex.
pub fn expand_format(fmt: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for alt in fmt.split(';') {
        let alt = alt.trim();
        if alt.is_empty() {
            continue;
        }
        if let Some(raw) = alt.strip_prefix("re:") {
            out.push(raw.to_string());
            continue;
        }
        let mut re = String::new();
        for (i, part) in alt.split('+').enumerate() {
            if i > 0 {
                re.push_str("[ -]?");
            }
            re.push_str(&expand_part(part.trim())?);
        }
        out.push(re);
    }
    if out.is_empty() {
        return Err("format is empty".to_string());
    }
    Ok(out)
}

fn expand_part(p: &str) -> Result<String, String> {
    if p.is_empty() {
        return Err("empty part in format".to_string());
    }
    let digit_len = p.chars().take_while(|c| c.is_ascii_digit()).count();
    if digit_len > 0 && p.chars().count() == digit_len + 1 {
        let n: usize = p[..digit_len]
            .parse()
            .map_err(|_| format!("bad count in '{}'", p))?;
        let class = match p.chars().last().unwrap() {
            'D' => "\\d",
            'A' => "[A-Z]",
            'X' => "[A-Z0-9]",
            other => return Err(format!("unknown class '{}' in '{}'", other, p)),
        };
        return Ok(format!("{}{{{}}}", class, n));
    }
    let mut s = String::new();
    for c in p.chars() {
        match c {
            '#' => s.push_str("\\d"),
            'A' => s.push_str("[A-Z]"),
            'X' => s.push_str("[A-Z0-9]"),
            other => s.push_str(&regex::escape(&other.to_string())),
        }
    }
    Ok(s)
}

/// Wrap a regex body with start/end boundaries. Named groups: `pre`, `body`.
/// (Rust's regex crate has no lookbehind, so the boundary character is consumed;
/// callers must restart scanning at the end of `body`.)
pub fn wrap_regex(body: &str) -> String {
    format!(
        "(?P<pre>^|[^\\p{{L}}\\p{{N}}])(?P<body>{})(?:$|[^\\p{{L}}\\p{{N}}])",
        body
    )
}

fn looks_like_number_field(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E' | '+' | '-'))
}

/// Validate every row. Returns a list of human-readable errors (empty = OK).
pub fn validate_rows(rows: &[Row]) -> Vec<String> {
    let mut errors = Vec::new();
    let mut seen_ids: HashSet<String> = HashSet::new();

    for row in rows {
        let id = row.get("id").trim().to_string();
        let tag = format!("line {} (id '{}')", row.line_no, id);

        if id.is_empty() {
            errors.push(format!("line {}: empty id", row.line_no));
            continue;
        }
        if !seen_ids.insert(id.clone()) {
            errors.push(format!("{}: duplicate id", tag));
        }
        for col in [
            "country",
            "identifier_type",
            "display_name",
            "type_display_name",
            "format",
            "verification_status",
        ] {
            if row.get(col).trim().is_empty() {
                errors.push(format!("{}: column '{}' is empty", tag, col));
            }
        }
        let checksum = row.get("checksum").trim();
        if !KNOWN_CHECKSUMS.contains(&checksum) {
            errors.push(format!(
                "{}: unknown checksum '{}' (known: {})",
                tag,
                checksum,
                KNOWN_CHECKSUMS.join(", ")
            ));
        }
        let status = row.get("verification_status").trim();
        if !KNOWN_STATUSES.contains(&status) {
            errors.push(format!(
                "{}: verification_status must be one of {}",
                tag,
                KNOWN_STATUSES.join("|")
            ));
        }
        let cr = row.get("context_required").trim();
        if cr != "yes" && cr != "no" {
            errors.push(format!("{}: context_required must be yes or no", tag));
        }
        match row.get("base_confidence").trim().parse::<f32>() {
            Ok(v) if (0.0..=1.0).contains(&v) => {}
            _ => errors.push(format!("{}: base_confidence must be a number 0.0-1.0", tag)),
        }
        for col in ["valid_from", "valid_until"] {
            let v = row.get(col).trim();
            if !v.is_empty() && !(v.len() == 4 && v.chars().all(|c| c.is_ascii_digit())) {
                errors.push(format!("{}: {} must be empty or a 4-digit year", tag, col));
            }
        }

        // Scientific-notation corruption (Excel / Numbers).
        for col in ["examples_ok", "examples_no"] {
            for ex in row.get(col).split('|') {
                let ex = ex.trim();
                if looks_like_number_field(ex) && (ex.contains("E+") || ex.contains("e+")) {
                    errors.push(format!(
                        "Row {}, column {}: value '{}' looks like scientific notation (Excel/Numbers corruption). Re-enter the value as plain text.",
                        id, col, ex
                    ));
                }
            }
        }

        // Format must expand and compile; examples must behave.
        match expand_format(row.get("format")) {
            Err(e) => errors.push(format!("{}: bad format: {}", tag, e)),
            Ok(bodies) => {
                let mut compiled = Vec::new();
                for body in &bodies {
                    match Regex::new(&wrap_regex(body)) {
                        Ok(r) => compiled.push(r),
                        Err(e) => errors.push(format!(
                            "{}: format '{}' does not compile: {}",
                            tag,
                            body,
                            e.to_string().lines().last().unwrap_or("")
                        )),
                    }
                }
                if !compiled.is_empty() {
                    for ex in row.get("examples_ok").split('|') {
                        let ex = ex.trim();
                        if ex.is_empty() || ex.starts_with("TODO") {
                            continue;
                        }
                        if !compiled.iter().any(|r| r.is_match(ex)) {
                            errors.push(format!(
                                "{}: examples_ok '{}' does not match its own format",
                                tag, ex
                            ));
                        }
                    }
                    for ex in row.get("examples_no").split('|') {
                        let ex = ex.trim();
                        if ex.is_empty() {
                            continue;
                        }
                        if compiled.iter().any(|r| r.is_match(ex)) {
                            errors.push(format!(
                                "{}: examples_no '{}' unexpectedly matches its own format",
                                tag, ex
                            ));
                        }
                    }
                }
            }
        }
    }
    errors
}

/// Parse + validate. `Ok(row_count)` or `Err(list of errors)`.
pub fn validate_csv_text(text: &str) -> Result<usize, Vec<String>> {
    let rows = parse_csv(text).map_err(|e| vec![e])?;
    let errors = validate_rows(&rows);
    if errors.is_empty() {
        Ok(rows.len())
    } else {
        Err(errors)
    }
}
