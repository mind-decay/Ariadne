//! SCIP symbol grammar parser + canonical form + stable 64-bit `SymbolId`.
//!
//! The grammar is the one documented inline in `scip.proto` at the pinned
//! SHA in `proto/SCIP_COMMIT`
//! [src: <https://github.com/sourcegraph/scip/blob/main/scip.proto> lines
//! 144-177]. We follow it strictly: single space is the field separator,
//! double space is the escape for a literal space inside a UTF-8 field,
//! double backtick is the escape for a literal backtick inside an
//! escaped-identifier. Names that fit `<simple-identifier>` and names
//! written with backtick escapes for the same content normalize to the
//! same `Descriptor`, which is the "equivalent forms hash equal"
//! invariant the tier plan calls out (step 14).
//!
//! `SymbolId` is `blake3(canonical-bytes)` truncated to the first 8 bytes
//! interpreted as little-endian `u64`. Truncating blake3 to 64 bits keeps
//! collision risk at ~`2^-32` for a 100K-file project, well inside the
//! v1 SLO budget [src: <https://github.com/BLAKE3-team/BLAKE3-specs/blob/master/blake3.pdf>
//! §6.5]. The hash is the only persisted identifier salsa/storage layers
//! see.

mod grammar;

use crate::errors::ScipError;

/// Descriptor suffix as defined in `scip.proto` `Descriptor.Suffix`. The
/// deprecated `Package` alias collapses into `Namespace` at parse time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum DescriptorSuffix {
    /// `/`. Namespace / package (alias `Package = 1` collapses here).
    Namespace,
    /// `#`. Type / class / struct.
    Type,
    /// `.` not followed by `(`. Term / function / value.
    Term,
    /// `name(disambig).`. Method.
    Method,
    /// `[name]`. Generic type parameter.
    TypeParameter,
    /// `(name)`. Method parameter.
    Parameter,
    /// `:`. Free-form meta descriptor.
    Meta,
    /// `!`. Macro.
    Macro,
    /// `local <id>`. Document-local symbol.
    Local,
}

impl DescriptorSuffix {
    /// Single byte tag used in the canonical hash input. Stable across
    /// process invocations.
    #[must_use]
    pub const fn tag(self) -> u8 {
        match self {
            Self::Namespace => 1,
            Self::Type => 2,
            Self::Term => 3,
            Self::Method => 4,
            Self::TypeParameter => 5,
            Self::Parameter => 6,
            Self::Meta => 7,
            Self::Local => 8,
            Self::Macro => 9,
        }
    }
}

/// One element of the descriptor chain (`<namespace>/`, `<type>#`, …).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Descriptor {
    /// Unescaped name. For escaped-identifiers (`` `…` ``) this is the
    /// content with backtick escapes resolved.
    pub name: String,
    /// Suffix kind.
    pub suffix: DescriptorSuffix,
    /// Method disambiguator (only set when `suffix == Method`).
    pub disambiguator: Option<String>,
}

/// Canonical, parse-once form of a SCIP symbol string.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CanonicalSymbol {
    /// Scheme (`scip-rust`, `rust-analyzer`, …) or `"local"` for local
    /// symbols.
    pub scheme: String,
    /// Package manager (`cargo`, `npm`, …). `None` for placeholders or
    /// local symbols.
    pub manager: Option<String>,
    /// Package name. `None` for placeholders or local symbols.
    pub package_name: Option<String>,
    /// Package version. `None` for placeholders or local symbols.
    pub version: Option<String>,
    /// Descriptor chain, root-first. Length ≥ 1.
    pub descriptors: Vec<Descriptor>,
}

/// 64-bit stable symbol identifier. Truncated blake3 over a deterministic
/// byte serialization of `CanonicalSymbol`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SymbolId(u64);

impl SymbolId {
    /// Raw `u64`.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// 16-char lowercase hex.
    #[must_use]
    pub fn to_hex(self) -> String {
        format!("{:016x}", self.0)
    }
}

impl CanonicalSymbol {
    /// Deterministic byte serialization fed to blake3. `0x1F` (ASCII unit
    /// separator) terminates each field to make boundaries unambiguous.
    fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(64);
        out.extend_from_slice(self.scheme.as_bytes());
        out.push(0x1F);
        out.extend_from_slice(self.manager.as_deref().unwrap_or("").as_bytes());
        out.push(0x1F);
        out.extend_from_slice(self.package_name.as_deref().unwrap_or("").as_bytes());
        out.push(0x1F);
        out.extend_from_slice(self.version.as_deref().unwrap_or("").as_bytes());
        out.push(0x1F);
        for d in &self.descriptors {
            out.push(d.suffix.tag());
            out.extend_from_slice(d.name.as_bytes());
            out.push(0x1F);
            if let Some(disambig) = &d.disambiguator {
                out.extend_from_slice(disambig.as_bytes());
            }
            out.push(0x1F);
        }
        out
    }

    /// Stable 64-bit identifier. Equal for any two `CanonicalSymbol`
    /// values that compare `==`.
    #[must_use]
    pub fn id(&self) -> SymbolId {
        let hash = blake3::hash(&self.canonical_bytes());
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&hash.as_bytes()[..8]);
        SymbolId(u64::from_le_bytes(buf))
    }
}

/// Parse a raw SCIP symbol string into [`CanonicalSymbol`]. Whitespace
/// escapes (double space → literal space) and backtick escapes inside
/// escaped-identifiers are resolved so equivalent encodings hash equal.
///
/// # Errors
/// Returns [`ScipError::MalformedSymbol`] when the input does not match
/// the grammar.
pub fn normalize_scip_symbol(raw: &str) -> Result<CanonicalSymbol, ScipError> {
    grammar::parse(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_symbol_parses() {
        let s = normalize_scip_symbol("local foo42").unwrap();
        assert_eq!(s.scheme, "local");
        assert_eq!(s.descriptors.len(), 1);
        assert_eq!(s.descriptors[0].name, "foo42");
        assert_eq!(s.descriptors[0].suffix, DescriptorSuffix::Local);
    }

    #[test]
    fn full_symbol_parses() {
        let s = normalize_scip_symbol("scip-rust cargo my_crate 1.2.3 my_mod/MyType#do_thing().")
            .unwrap();
        assert_eq!(s.scheme, "scip-rust");
        assert_eq!(s.manager.as_deref(), Some("cargo"));
        assert_eq!(s.package_name.as_deref(), Some("my_crate"));
        assert_eq!(s.version.as_deref(), Some("1.2.3"));
        assert_eq!(s.descriptors.len(), 3);
        assert_eq!(s.descriptors[0].suffix, DescriptorSuffix::Namespace);
        assert_eq!(s.descriptors[1].suffix, DescriptorSuffix::Type);
        assert_eq!(s.descriptors[2].suffix, DescriptorSuffix::Method);
    }

    #[test]
    fn placeholder_manager_collapses_to_none() {
        let s = normalize_scip_symbol("scip-rust . my_crate 1.0 m#").unwrap();
        assert_eq!(s.manager, None);
    }

    #[test]
    fn equivalent_name_forms_hash_equal() {
        let a = normalize_scip_symbol("s . p 1 `foo`#").unwrap();
        let b = normalize_scip_symbol("s . p 1 foo#").unwrap();
        assert_eq!(a.id(), b.id());
    }

    #[test]
    fn deterministic_across_invocations() {
        let raw = "scip-rust cargo my_crate 1.2.3 mod/Cls#m().";
        let first = normalize_scip_symbol(raw).unwrap().id();
        for _ in 0..10 {
            assert_eq!(normalize_scip_symbol(raw).unwrap().id(), first);
        }
    }
}
