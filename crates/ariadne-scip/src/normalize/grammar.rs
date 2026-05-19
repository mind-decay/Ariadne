//! Internal SCIP symbol grammar parser. Public entry point is
//! [`crate::normalize::normalize_scip_symbol`]; this module only handles
//! the byte-level walk so [`super`] stays focused on the types.

use super::{CanonicalSymbol, Descriptor, DescriptorSuffix};
use crate::errors::ScipError;

pub(super) fn parse(raw: &str) -> Result<CanonicalSymbol, ScipError> {
    if let Some(local_id) = raw.strip_prefix("local ") {
        if local_id.is_empty() {
            return Err(malformed(raw, "empty local id"));
        }
        return Ok(CanonicalSymbol {
            scheme: "local".to_owned(),
            manager: None,
            package_name: None,
            version: None,
            descriptors: vec![Descriptor {
                name: local_id.to_owned(),
                suffix: DescriptorSuffix::Local,
                disambiguator: None,
            }],
        });
    }

    let mut cursor = Cursor::new(raw);
    let scheme = cursor
        .read_field()
        .ok_or_else(|| malformed(raw, "missing scheme"))?;
    if scheme.is_empty() {
        return Err(malformed(raw, "empty scheme"));
    }
    if scheme == "local" {
        return Err(malformed(raw, "scheme must not be 'local'"));
    }
    let manager = cursor
        .read_field()
        .ok_or_else(|| malformed(raw, "missing manager"))?;
    let package_name = cursor
        .read_field()
        .ok_or_else(|| malformed(raw, "missing package name"))?;
    let version = cursor
        .read_field()
        .ok_or_else(|| malformed(raw, "missing version"))?;

    let descriptors = parse_descriptors(cursor.remaining(), raw)?;
    if descriptors.is_empty() {
        return Err(malformed(raw, "missing descriptors"));
    }

    Ok(CanonicalSymbol {
        scheme,
        manager: placeholder_to_none(manager),
        package_name: placeholder_to_none(package_name),
        version: placeholder_to_none(version),
        descriptors,
    })
}

fn placeholder_to_none(value: String) -> Option<String> {
    if value == "." || value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn malformed(raw: &str, reason: &'static str) -> ScipError {
    ScipError::MalformedSymbol {
        symbol: raw.to_owned(),
        reason,
    }
}

struct Cursor<'a> {
    rest: &'a [u8],
}

impl<'a> Cursor<'a> {
    fn new(s: &'a str) -> Self {
        Self { rest: s.as_bytes() }
    }

    fn remaining(&self) -> &'a [u8] {
        self.rest
    }

    /// Read one space-terminated field, resolving the double-space escape.
    /// Returns `None` if no separator is found.
    fn read_field(&mut self) -> Option<String> {
        let mut out = String::new();
        let mut i = 0;
        while i < self.rest.len() {
            if self.rest[i] == b' ' {
                if self.rest.get(i + 1) == Some(&b' ') {
                    out.push(' ');
                    i += 2;
                    continue;
                }
                self.rest = &self.rest[i + 1..];
                return Some(out);
            }
            out.push(self.rest[i] as char);
            i += 1;
        }
        None
    }
}

fn parse_descriptors(bytes: &[u8], raw: &str) -> Result<Vec<Descriptor>, ScipError> {
    let mut descriptors = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let (desc, next) = parse_one_descriptor(bytes, i, raw)?;
        descriptors.push(desc);
        i = next;
    }
    Ok(descriptors)
}

fn parse_one_descriptor(
    bytes: &[u8],
    start: usize,
    raw: &str,
) -> Result<(Descriptor, usize), ScipError> {
    if bytes.get(start) == Some(&b'[') {
        let (name, end) = read_name_until(bytes, start + 1, b']', raw)?;
        return Ok((simple(name, DescriptorSuffix::TypeParameter), end + 1));
    }
    if bytes.get(start) == Some(&b'(') {
        let (name, end) = read_name_until(bytes, start + 1, b')', raw)?;
        return Ok((simple(name, DescriptorSuffix::Parameter), end + 1));
    }
    let (name, name_end) = read_name(bytes, start, raw)?;
    let suffix_byte = bytes
        .get(name_end)
        .copied()
        .ok_or_else(|| malformed(raw, "descriptor ended without suffix"))?;
    match suffix_byte {
        b'/' => Ok((simple(name, DescriptorSuffix::Namespace), name_end + 1)),
        b'#' => Ok((simple(name, DescriptorSuffix::Type), name_end + 1)),
        b':' => Ok((simple(name, DescriptorSuffix::Meta), name_end + 1)),
        b'!' => Ok((simple(name, DescriptorSuffix::Macro), name_end + 1)),
        b'.' => Ok((simple(name, DescriptorSuffix::Term), name_end + 1)),
        b'(' => parse_method(bytes, name, name_end, raw),
        _ => Err(malformed(raw, "unknown descriptor suffix")),
    }
}

fn parse_method(
    bytes: &[u8],
    name: String,
    name_end: usize,
    raw: &str,
) -> Result<(Descriptor, usize), ScipError> {
    let (disambig, paren_end) = if bytes.get(name_end + 1) == Some(&b')') {
        (String::new(), name_end + 1)
    } else {
        read_name_until(bytes, name_end + 1, b')', raw)?
    };
    if bytes.get(paren_end + 1) != Some(&b'.') {
        return Err(malformed(raw, "method missing trailing '.'"));
    }
    let disambiguator = if disambig.is_empty() {
        None
    } else {
        Some(disambig)
    };
    Ok((
        Descriptor {
            name,
            suffix: DescriptorSuffix::Method,
            disambiguator,
        },
        paren_end + 2,
    ))
}

fn simple(name: String, suffix: DescriptorSuffix) -> Descriptor {
    Descriptor {
        name,
        suffix,
        disambiguator: None,
    }
}

fn read_name(bytes: &[u8], start: usize, raw: &str) -> Result<(String, usize), ScipError> {
    if bytes.get(start) == Some(&b'`') {
        read_escaped(bytes, start + 1, raw)
    } else {
        read_simple(bytes, start, raw)
    }
}

fn read_simple(bytes: &[u8], start: usize, raw: &str) -> Result<(String, usize), ScipError> {
    let mut i = start;
    while i < bytes.len() && is_ident_char(bytes[i]) {
        i += 1;
    }
    if i == start {
        return Err(malformed(raw, "empty simple identifier"));
    }
    let name = String::from_utf8(bytes[start..i].to_vec())
        .map_err(|_| malformed(raw, "non-utf8 simple identifier"))?;
    Ok((name, i))
}

fn read_escaped(bytes: &[u8], start: usize, raw: &str) -> Result<(String, usize), ScipError> {
    let mut out = String::new();
    let mut i = start;
    while i < bytes.len() {
        if bytes[i] == b'`' {
            if bytes.get(i + 1) == Some(&b'`') {
                out.push('`');
                i += 2;
                continue;
            }
            return Ok((out, i + 1));
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    Err(malformed(raw, "unterminated escaped identifier"))
}

fn read_name_until(
    bytes: &[u8],
    start: usize,
    terminator: u8,
    raw: &str,
) -> Result<(String, usize), ScipError> {
    let (name, end) = read_name(bytes, start, raw)?;
    if bytes.get(end) != Some(&terminator) {
        return Err(malformed(raw, "missing closing bracket/paren"));
    }
    Ok((name, end))
}

const fn is_ident_char(b: u8) -> bool {
    matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'+' | b'-' | b'$')
}
