//! `search_code` — ranked symbol search over the in-RAM [`Catalog`].
//!
//! Generalises [`crate::tools::list_symbols`] (name-substring only) to a
//! regex-or-substring name match plus optional `path` glob / `kind` / `lang`
//! / `visibility` filters, returning a ranked [`SymbolSummary`] list. A pure
//! read projection over the catalog — no new domain port (tier-07 D8).
//!
//! The name matcher and path glob are each compiled once before the scan;
//! a malformed `query` regex or `path` glob surfaces as a typed
//! [`McpError`], never a panic. The regex is bounded by `size_limit` +
//! `nest_limit` so an adversarial pattern cannot exhaust resources (no
//! `ReDoS`) [src: tier-07 `<context>`; docs.rs/regex/1.12.3 `RegexBuilder`].

use ariadne_core::Visibility;
use regex::{Regex, RegexBuilder};

use crate::catalog::Catalog;
use crate::errors::McpError;
use crate::tools::summarize;
use crate::types::{SearchCodeInput, SymbolSummary};

const DEFAULT_LIMIT: u32 = 64;

/// Compiled-regex resource ceilings: 1 MiB program size and a 64-deep nest
/// limit keep a pathological pattern from exhausting memory / stack while
/// staying generous for any realistic symbol-name search.
const REGEX_SIZE_LIMIT: usize = 1 << 20;
const REGEX_NEST_LIMIT: u32 = 64;

/// Name matcher compiled once for the whole scan.
enum Matcher {
    /// Lower-cased substring of the canonical name (empty = match all).
    Substring(String),
    /// Case-insensitive compiled regular expression.
    Regex(Regex),
}

impl Matcher {
    /// Build the matcher from the input, mapping a malformed regex to a
    /// typed error.
    fn compile(input: &SearchCodeInput) -> Result<Self, McpError> {
        if input.regex {
            let re = RegexBuilder::new(&input.query)
                .case_insensitive(true)
                .size_limit(REGEX_SIZE_LIMIT)
                .nest_limit(REGEX_NEST_LIMIT)
                .build()
                .map_err(|e| McpError::InvalidInput(format!("regex `{}`: {e}", input.query)))?;
            Ok(Self::Regex(re))
        } else {
            Ok(Self::Substring(input.query.to_lowercase()))
        }
    }

    /// Whether `name` matches. The lower-cased `name_lc` is reused for the
    /// substring test and rank.
    fn is_match(&self, name: &str, name_lc: &str) -> bool {
        match self {
            Self::Substring(needle) => needle.is_empty() || name_lc.contains(needle.as_str()),
            Self::Regex(re) => re.is_match(name),
        }
    }
}

/// Top-K symbols matching the name pattern and every supplied filter, ranked
/// exact > prefix > other, then by canonical name.
///
/// # Errors
/// Returns [`McpError::InvalidInput`] when `query` is an invalid regex (with
/// `regex: true`) or `path` is an invalid glob — never panics on caller
/// input.
pub fn handle(cat: &Catalog, input: &SearchCodeInput) -> Result<Vec<SymbolSummary>, McpError> {
    let limit = usize::try_from(input.limit.unwrap_or(DEFAULT_LIMIT).max(1)).unwrap_or(usize::MAX);
    let matcher = Matcher::compile(input)?;
    let needle = match &matcher {
        Matcher::Substring(n) => n.clone(),
        Matcher::Regex(_) => String::new(),
    };
    let path_glob = match input.path.as_deref() {
        Some(p) => Some(
            glob::Pattern::new(p)
                .map_err(|e| McpError::InvalidInput(format!("path glob `{p}`: {e}")))?,
        ),
        None => None,
    };
    let kind_filter = input.kind.as_deref();
    let lang_filter = input.lang.as_deref();
    let vis_filter = input.visibility.as_deref();

    // (rank, name, id) for every match, sorted then truncated so the ranked
    // top-K is global rather than the first-K in id order.
    let mut ranked: Vec<(u8, String, ariadne_core::SymbolId)> = Vec::new();
    for (id, meta) in &cat.symbols {
        // Lower-case only on the substring path: the regex matcher reads the
        // raw name and `rank` ignores `name_lc` in regex mode, so skip the
        // per-symbol allocation there (audit I2). `String::new()` does not heap-
        // allocate, so the unused regex-mode binding is free.
        let name_lc = if input.regex {
            String::new()
        } else {
            meta.name.to_lowercase()
        };
        if !matcher.is_match(&meta.name, &name_lc) {
            continue;
        }
        if let Some(want) = kind_filter {
            if meta.kind != want {
                continue;
            }
        }
        if let Some(want) = lang_filter {
            if !meta.lang.tag().eq_ignore_ascii_case(want) {
                continue;
            }
        }
        if let Some(want) = vis_filter {
            if !visibility_matches(meta.visibility, want) {
                continue;
            }
        }
        if let Some(pattern) = &path_glob {
            let Some(path) = cat.path_of(meta.file) else {
                continue;
            };
            if !pattern.matches_path(std::path::Path::new(path)) {
                continue;
            }
        }
        ranked.push((rank(&name_lc, &needle, input.regex), meta.name.clone(), *id));
    }

    ranked.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.cmp(&b.1))
            .then(a.2.cmp(&b.2))
    });
    ranked.truncate(limit);
    Ok(ranked
        .into_iter()
        .map(|(_, _, id)| summarize(cat, id))
        .collect())
}

/// Rank a hit: `0` exact, `1` prefix, `2` otherwise (and always `2` in regex
/// mode, where the literal `query` is a pattern, not a comparable name).
fn rank(name_lc: &str, needle: &str, is_regex: bool) -> u8 {
    if is_regex || needle.is_empty() {
        2
    } else if name_lc == needle {
        0
    } else if name_lc.starts_with(needle) {
        1
    } else {
        2
    }
}

/// Case-insensitive match of a [`Visibility`] against its tag string. The
/// `Unknown` arm doubles as the catch-all for the `#[non_exhaustive]` enum.
fn visibility_matches(vis: Visibility, want: &str) -> bool {
    let tag = match vis {
        Visibility::Public => "public",
        Visibility::Restricted => "restricted",
        Visibility::Private => "private",
        _ => "unknown",
    };
    tag.eq_ignore_ascii_case(want)
}
