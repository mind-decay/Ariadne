//! Response-economy use case (Block 1, tier-01). A pure, delivery-layer
//! projection every growable tool reuses across all three serving paths
//! (MCP cold, MCP warm/daemon, CLI `query`): a default page cap, an opaque
//! revision-stamped cursor for the remainder, and a verbosity knob. Capping
//! only shrinks an already-computed result — it never changes what a tool
//! computes [src: .claude/plans/data-fidelity-arc/block-1/plan.md D1,D2,D3,D4].
//!
//! The cursor codec is hand-rolled (no base64/cbor dep on the critical path,
//! D2) and mirrors the MCP spec's opaque, MUST-NOT-parse cursor model for
//! list operations, carried in-payload because `tools/call` results are not
//! covered by spec pagination [src:
//! <https://modelcontextprotocol.io/specification/2025-06-18/server/utilities/pagination>].

use std::cmp::Ordering;

use thiserror::Error;

/// Default page size — the per-list cap applied when a caller omits `limit`.
/// 50 keeps every measured growable tool well under the 25k-token MCP cap
/// (rows are 45–80 tokens each); the tier-05 harness verifies it (D4).
pub const DEFAULT_PAGE: usize = 50;

/// Field verbosity for a growable tool's rows (D3). `Concise` (the default)
/// omits the cryptic fields the LLM reasons about worse — raw symbol ids and
/// byte offsets — keeping the semantic name/file; `Detailed` is a lossless
/// superset every in-repo precision consumer pins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Verbosity {
    /// Omit cryptic id/offset fields (the default).
    #[default]
    Concise,
    /// Emit every field — the lossless superset.
    Detailed,
}

/// Opaque, revision-stamped pagination cursor (D2). `offsets` is a per-sublist
/// offset vector (length 1 for a single-list tool, N for a multi-list one), so
/// one cursor type serves every tool. The `revision` is the catalog revision
/// the offsets index into: within a revision an offset into a stable sort is
/// deterministic and complete; across a re-index the revisions mismatch and
/// [`Cursor::decode`] rejects the cursor rather than mis-paging.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    /// Catalog revision the offsets are valid against.
    pub revision: u32,
    /// Per-sublist start offset into each list's stable sort.
    pub offsets: Vec<u64>,
}

/// Pagination + verbosity request bundle a handler hands to [`paginate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Budget {
    /// Maximum rows per sublist in this page.
    pub limit: usize,
    /// Decoded cursor from a prior page, or `None` for the first page.
    pub cursor: Option<Cursor>,
    /// Field verbosity the handler applies when projecting rows.
    pub verbosity: Verbosity,
}

/// One page of a paginated result: the sliced rows plus an opaque
/// `next_cursor` (set only when more rows remain).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page<T> {
    /// The rows in this page, in the caller's stable sort order.
    pub rows: Vec<T>,
    /// Opaque cursor to fetch the next page; `None` when this is the last.
    pub next_cursor: Option<String>,
}

/// A cursor that could not be honored. Maps to a JSON-RPC `invalid_params`
/// (−32602) at the adapter so a client re-queries instead of silently
/// mis-paging (the MCP spec's "handle invalid cursors gracefully", D2).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CursorError {
    /// The cursor string was not produced by [`Cursor::encode`].
    #[error("malformed pagination cursor")]
    Malformed,
    /// The cursor was minted against a different index revision; the index has
    /// since changed, so re-run the query without the cursor.
    #[error(
        "stale pagination cursor (minted at revision {found}, current is {expected}); \
         re-run the query without the cursor"
    )]
    StaleRevision {
        /// The current catalog revision the decode expected.
        expected: u32,
        /// The (older) revision encoded in the cursor.
        found: u32,
    },
}

impl Cursor {
    /// Encode to an opaque lowercase-hex string (url-safe, MUST-NOT-parse by
    /// the client). Layout: `revision` (u32 LE) ‖ `len` (u32 LE) ‖ `offsets`
    /// (len × u64 LE), hex-encoded.
    #[must_use]
    pub fn encode(&self) -> String {
        let mut bytes = Vec::with_capacity(8 + self.offsets.len() * 8);
        bytes.extend_from_slice(&self.revision.to_le_bytes());
        let len = u32::try_from(self.offsets.len()).unwrap_or(u32::MAX);
        bytes.extend_from_slice(&len.to_le_bytes());
        for off in &self.offsets {
            bytes.extend_from_slice(&off.to_le_bytes());
        }
        to_hex(&bytes)
    }

    /// Decode an opaque cursor and validate it against `expected_revision`.
    ///
    /// # Errors
    /// [`CursorError::Malformed`] when the string is not a well-formed cursor;
    /// [`CursorError::StaleRevision`] when it was minted against a different
    /// revision (the index has changed under it).
    pub fn decode(s: &str, expected_revision: u32) -> Result<Self, CursorError> {
        let bytes = from_hex(s).ok_or(CursorError::Malformed)?;
        if bytes.len() < 8 {
            return Err(CursorError::Malformed);
        }
        let revision = le_u32(&bytes, 0);
        let len = le_u32(&bytes, 4) as usize;
        if bytes.len() != 8 + len * 8 {
            return Err(CursorError::Malformed);
        }
        if revision != expected_revision {
            return Err(CursorError::StaleRevision {
                expected: expected_revision,
                found: revision,
            });
        }
        let offsets = (0..len).map(|i| le_u64(&bytes, 8 + i * 8)).collect();
        Ok(Self { revision, offsets })
    }
}

/// Sort `rows` by the caller's stable `compare`, then return the page window
/// `[offset .. offset + limit)` for sublist `sublist_index`, stamping a
/// `next_cursor` only when rows remain beyond the window (D4). The cursor
/// carries `revision` so a later page is rejected once the index changes.
///
/// A `limit` of `0` yields an empty page with **no** `next_cursor`: a cursor at
/// the unchanged offset would re-page the same empty window forever (a liveness
/// footgun), so a zero-width page is reported as terminal.
pub fn paginate<T>(
    mut rows: Vec<T>,
    compare: impl FnMut(&T, &T) -> Ordering,
    budget: &Budget,
    revision: u32,
    sublist_index: usize,
) -> Page<T> {
    rows.sort_by(compare);
    let total = rows.len();
    let start = budget
        .cursor
        .as_ref()
        .and_then(|c| c.offsets.get(sublist_index).copied())
        .map_or(0, |o| usize::try_from(o).unwrap_or(usize::MAX))
        .min(total);
    let end = start.saturating_add(budget.limit).min(total);
    let page: Vec<T> = rows.into_iter().skip(start).take(end - start).collect();
    let next_cursor = (budget.limit > 0 && end < total).then(|| {
        let mut offsets = budget
            .cursor
            .as_ref()
            .map_or_else(Vec::new, |c| c.offsets.clone());
        if offsets.len() <= sublist_index {
            offsets.resize(sublist_index + 1, 0);
        }
        offsets[sublist_index] = end as u64;
        Cursor { revision, offsets }.encode()
    });
    Page {
        rows: page,
        next_cursor,
    }
}

/// The human truncation steer a handler carries in a page's `note` when more
/// rows remain (D5). Single-sourced here so every serving path (MCP cold, MCP
/// warm/daemon, CLI) emits byte-identical wording — the cold and warm twins
/// cannot drift. `noun` is the per-tool row label (e.g. `"references"`);
/// `shown` is this page's row count, `total` the full result size.
#[must_use]
pub fn truncation_note(shown: usize, total: usize, noun: &str) -> String {
    format!("Showing {shown} of {total} {noun} — call again with next_cursor for the next page.")
}

/// Read a little-endian `u32` from `b[at..at + 4]`. Callers length-check the
/// buffer first, so the fixed-window index is always in range.
fn le_u32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// Read a little-endian `u64` from `b[at..at + 8]`. Callers length-check the
/// buffer first, so the fixed-window index is always in range.
fn le_u64(b: &[u8], at: usize) -> u64 {
    u64::from_le_bytes([
        b[at],
        b[at + 1],
        b[at + 2],
        b[at + 3],
        b[at + 4],
        b[at + 5],
        b[at + 6],
        b[at + 7],
    ])
}

/// Lowercase-hex encode (hand-rolled, no `hex` dep).
fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

/// Lowercase-hex decode; `None` on any non-hex char or odd length.
fn from_hex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    let nibble = |c: u8| -> Option<u8> {
        match c {
            b'0'..=b'9' => Some(c - b'0'),
            b'a'..=b'f' => Some(c - b'a' + 10),
            _ => None,
        }
    };
    let bytes = s.as_bytes();
    (0..bytes.len() / 2)
        .map(|i| Some((nibble(bytes[2 * i])? << 4) | nibble(bytes[2 * i + 1])?))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_round_trips_revision_and_offsets() {
        let c = Cursor {
            revision: 537,
            offsets: vec![3, 0, 18_446_744_073_709_551_615],
        };
        let decoded = Cursor::decode(&c.encode(), 537).expect("round-trip decode");
        assert_eq!(decoded, c);
    }

    #[test]
    fn decode_rejects_wrong_revision() {
        let minted = Cursor {
            revision: 41,
            offsets: vec![50],
        }
        .encode();
        let err = Cursor::decode(&minted, 42).expect_err("revision mismatch must reject");
        assert_eq!(
            err,
            CursorError::StaleRevision {
                expected: 42,
                found: 41
            }
        );
    }

    #[test]
    fn decode_rejects_garbage() {
        assert_eq!(Cursor::decode("zz", 0), Err(CursorError::Malformed));
        assert_eq!(Cursor::decode("00", 0), Err(CursorError::Malformed));
    }

    #[test]
    fn paginate_pages_completely_without_gap_or_dup() {
        // 7 items, limit 3: pages of 3 + 3 + 1, union == sorted input.
        let input = vec![6_u32, 1, 4, 2, 7, 3, 5];
        let revision = 9;
        let cmp = |a: &u32, b: &u32| a.cmp(b);

        let mut budget = Budget {
            limit: 3,
            cursor: None,
            verbosity: Verbosity::Concise,
        };
        let mut seen: Vec<u32> = Vec::new();
        let mut pages = 0;
        loop {
            let page = paginate(input.clone(), cmp, &budget, revision, 0);
            pages += 1;
            assert!(page.rows.len() <= 3, "page never exceeds the limit");
            seen.extend(page.rows.iter().copied());
            match page.next_cursor {
                Some(cursor) => {
                    let decoded = Cursor::decode(&cursor, revision).expect("cursor decodes");
                    budget.cursor = Some(decoded);
                }
                None => break,
            }
        }
        assert_eq!(pages, 3, "7 / 3 → three pages");
        assert_eq!(
            seen,
            vec![1, 2, 3, 4, 5, 6, 7],
            "union == sorted input, no gap/dup"
        );
    }

    #[test]
    fn paginate_zero_limit_emits_no_cursor() {
        // limit:0 yields an empty page and, crucially, no cursor: a cursor at
        // the unchanged offset would re-page the same empty window forever, so
        // a zero-width page must be terminal (liveness).
        let page = paginate(
            vec![1_u32, 2, 3],
            Ord::cmp,
            &Budget {
                limit: 0,
                cursor: None,
                verbosity: Verbosity::Concise,
            },
            1,
            0,
        );
        assert!(page.rows.is_empty(), "limit:0 → empty page");
        assert!(
            page.next_cursor.is_none(),
            "limit:0 → no cursor (re-feeding would make no progress)"
        );
    }

    #[test]
    fn truncation_note_is_single_sourced() {
        // The one wording both serving paths share — proven byte-stable here so
        // the cold/warm twins cannot diverge.
        assert_eq!(
            truncation_note(50, 244, "references"),
            "Showing 50 of 244 references — call again with next_cursor for the next page."
        );
    }

    #[test]
    fn paginate_single_page_has_no_cursor() {
        let page = paginate(
            vec![1_u32, 2, 3],
            Ord::cmp,
            &Budget {
                limit: DEFAULT_PAGE,
                cursor: None,
                verbosity: Verbosity::Detailed,
            },
            1,
            0,
        );
        assert_eq!(page.rows, vec![1, 2, 3]);
        assert!(
            page.next_cursor.is_none(),
            "result under the cap → no cursor"
        );
    }
}
