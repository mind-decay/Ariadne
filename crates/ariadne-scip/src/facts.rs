//! SCIP fact extraction (scip-driven-edges tier-01, plan D2).
//!
//! [`extract_facts`] reduces a completed [`IngestReport`] to the per-file edge
//! signal `ariadne-salsa` consumes: each occurrence's normalized symbol key,
//! byte range, and roles, plus the indexed content hash for the D4 coverage
//! gate. SCIP occurrence ranges are line/character pairs whose `character`
//! units depend on the document's `position_encoding`
//! [src: crates/ariadne-scip/proto/scip.proto:645-675,118-143]; converting them
//! to byte offsets — the coordinate the tree-sitter symbols use — needs the
//! file's source text, taken from the document's `text` field if the indexer
//! embedded it, else read from disk via the index `project_root` + the document
//! `relative_path` (the same bytes the indexer hashed).

use std::path::Path;

use ariadne_core::{ScipFacts, ScipOccurrence};

use crate::indexer::IngestReport;
use crate::normalize::normalize_scip_symbol;
use crate::proto;

/// Extract per-file SCIP facts from a completed ingest run (plan D2). For every
/// document of every successful SCIP doc: resolve the source text, hash it for
/// the D4 gate, then convert each occurrence's line/character range to bytes,
/// normalize its symbol to a stable key (so equivalent encodings key equal,
/// plan D3), and keep its `symbol_roles`. Returns `(relative_path, ScipFacts)`
/// pairs. A document whose text cannot be resolved is skipped — degraded, never
/// wrong: its files keep the precise tree-sitter resolver (plan D4).
#[must_use]
pub fn extract_facts(report: &IngestReport) -> Vec<(String, ScipFacts)> {
    let mut out = Vec::new();
    for doc in &report.docs {
        let project_root = doc.index.metadata.as_ref().map(|m| m.project_root.as_str());
        for document in &doc.index.documents {
            let Some(text) = document_text(document, project_root) else {
                continue;
            };
            let indexed_hash = *blake3::hash(&text).as_bytes();
            let starts = line_starts(&text);
            let encoding = document.position_encoding;
            let mut occurrences = Vec::with_capacity(document.occurrences.len());
            for occ in &document.occurrences {
                if occ.symbol.is_empty() {
                    continue;
                }
                let Some(byte_range) = occurrence_byte_range(&occ.range, &text, &starts, encoding)
                else {
                    continue;
                };
                let Ok(canon) = normalize_scip_symbol(&occ.symbol) else {
                    continue;
                };
                occurrences.push(ScipOccurrence {
                    symbol: canon.id().to_hex(),
                    byte_range,
                    // `symbol_roles` is a bitset; reinterpret the bits rather
                    // than a sign-losing numeric cast (MSRV 1.85 has no
                    // `i32::cast_unsigned`).
                    roles: u32::from_ne_bytes(occ.symbol_roles.to_ne_bytes()),
                });
            }
            out.push((
                document.relative_path.clone(),
                ScipFacts {
                    occurrences,
                    indexed_hash,
                },
            ));
        }
    }
    out
}

/// Resolve a document's source text: its embedded `text` field if present, else
/// the on-disk file at `project_root`/`relative_path`. `None` when neither is
/// available (no embedded text and no/unreadable disk file).
fn document_text(document: &proto::Document, project_root: Option<&str>) -> Option<Vec<u8>> {
    if !document.text.is_empty() {
        return Some(document.text.clone().into_bytes());
    }
    let root = project_root?;
    // SCIP `project_root` is a URI; the `file://` scheme prefix is stripped to a
    // filesystem path (no percent-decoding — paths with reserved characters fall
    // through to the `read` failure and the file keeps the resolver).
    let root_path = root.strip_prefix("file://").unwrap_or(root);
    let path = Path::new(root_path).join(&document.relative_path);
    std::fs::read(&path).ok()
}

/// Byte offset of each 0-based line's first byte. `starts[0] == 0`; a `\n` at
/// byte `i` starts line N+1 at `i + 1`.
fn line_starts(text: &[u8]) -> Vec<u32> {
    let mut starts = vec![0u32];
    for (i, &b) in text.iter().enumerate() {
        if b == b'\n' {
            starts.push(u32::try_from(i + 1).unwrap_or(u32::MAX));
        }
    }
    starts
}

/// Convert one occurrence `range` (`[sl, sc, el, ec]` or `[sl, sc, ec]`) to a
/// `(byte_start, byte_end)` pair. `None` for a malformed range (wrong arity,
/// negative, or `end < start`) [src: crates/ariadne-scip/proto/scip.proto:645-675].
fn occurrence_byte_range(
    range: &[i32],
    text: &[u8],
    starts: &[u32],
    encoding: i32,
) -> Option<(u32, u32)> {
    let (sl, sc, el, ec) = match *range {
        [sl, sc, el, ec] => (sl, sc, el, ec),
        [sl, sc, ec] => (sl, sc, sl, ec),
        _ => return None,
    };
    let start = byte_at(text, starts, sl, sc, encoding)?;
    let end = byte_at(text, starts, el, ec, encoding)?;
    (end >= start).then_some((start, end))
}

/// Byte offset of a (line, character) position under `encoding`.
fn byte_at(text: &[u8], starts: &[u32], line: i32, character: i32, encoding: i32) -> Option<u32> {
    let line = usize::try_from(line).ok()?;
    let character = u32::try_from(character).ok()?;
    let line_start = *starts.get(line)? as usize;
    let line_end = starts.get(line + 1).map_or(text.len(), |&s| s as usize);
    let in_line = char_offset_to_byte(&text[line_start..line_end], character, encoding);
    u32::try_from(line_start + in_line).ok()
}

/// Byte offset within one line's bytes for a `character` code-unit offset.
/// UTF-8 and unspecified encodings treat `character` as a byte offset directly;
/// UTF-16/UTF-32 walk code points, counting code units, to the byte offset
/// [src: crates/ariadne-scip/proto/scip.proto:118-143].
fn char_offset_to_byte(line: &[u8], character: u32, encoding: i32) -> usize {
    let character = character as usize;
    let utf16 = proto::PositionEncoding::Utf16CodeUnitOffsetFromLineStart as i32;
    let utf32 = proto::PositionEncoding::Utf32CodeUnitOffsetFromLineStart as i32;
    if encoding != utf16 && encoding != utf32 {
        return character.min(line.len());
    }
    let Ok(s) = std::str::from_utf8(line) else {
        return character.min(line.len());
    };
    let mut units = 0usize;
    for (byte_idx, ch) in s.char_indices() {
        if units >= character {
            return byte_idx;
        }
        units += if encoding == utf16 { ch.len_utf16() } else { 1 };
    }
    line.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    const UTF8: i32 = proto::PositionEncoding::Utf8CodeUnitOffsetFromLineStart as i32;
    const UTF16: i32 = proto::PositionEncoding::Utf16CodeUnitOffsetFromLineStart as i32;

    #[test]
    fn line_starts_tracks_each_newline() {
        // "ab\nc\n" → line 0 at 0, line 1 at 3, line 2 at 5.
        assert_eq!(line_starts(b"ab\nc\n"), vec![0, 3, 5]);
    }

    #[test]
    fn utf8_character_is_a_byte_offset() {
        let text = b"fn connect() {}\n    connect();\n";
        let starts = line_starts(text);
        // `connect` name on line 0 is bytes 3..10; the call on line 1 is 20..27.
        assert_eq!(
            occurrence_byte_range(&[0, 3, 0, 10], text, &starts, UTF8),
            Some((3, 10)),
        );
        assert_eq!(
            occurrence_byte_range(&[1, 4, 1, 11], text, &starts, UTF8),
            Some((20, 27)),
        );
    }

    #[test]
    fn three_element_range_reuses_start_line() {
        let text = b"abcdefgh\n";
        let starts = line_starts(text);
        assert_eq!(
            occurrence_byte_range(&[0, 2, 5], text, &starts, UTF8),
            Some((2, 5)),
        );
    }

    #[test]
    fn utf16_offsets_walk_code_units_past_a_surrogate_pair() {
        // "🚀" is one code point = 2 UTF-16 units = 4 UTF-8 bytes, so `connect`
        // starts at UTF-16 unit 2 and byte 4, ending at unit 9 / byte 11.
        let text = "🚀connect\n".as_bytes();
        let starts = line_starts(text);
        assert_eq!(
            occurrence_byte_range(&[0, 2, 0, 9], text, &starts, UTF16),
            Some((4, 11)),
        );
    }

    #[test]
    fn malformed_range_arity_and_inversion_drop() {
        let text = b"abc\n";
        let starts = line_starts(text);
        assert_eq!(occurrence_byte_range(&[0, 1], text, &starts, UTF8), None);
        assert_eq!(
            occurrence_byte_range(&[0, 3, 0, 1], text, &starts, UTF8),
            None
        );
    }
}
