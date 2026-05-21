//! Language tag and its on-wire string form.
//!
//! Custom `Serialize`/`Deserialize` impls keep `Lang` `Copy` and let the
//! `Other(&'static str)` variant round-trip through postcard. Unknown
//! `other:<name>` suffixes are rehydrated via `Box::leak`, bounded by the
//! set of distinct unknown languages a single Ariadne process meets at
//! runtime.

use std::fmt;

use serde::de::{Error as DeError, Unexpected};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Language tag attached to files and symbols.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Lang {
    /// TypeScript.
    TypeScript,
    /// JavaScript.
    JavaScript,
    /// Python.
    Python,
    /// Rust.
    Rust,
    /// Go.
    Go,
    /// Java.
    Java,
    /// Kotlin.
    Kotlin,
    /// C#.
    CSharp,
    /// C.
    C,
    /// C++.
    Cpp,
    /// Any other tree-sitter grammar; carries its `tree-sitter-<lang>` name.
    Other(&'static str),
}

impl Lang {
    /// Stable string tag used as the on-disk / on-wire representation.
    /// `Other(s)` is encoded as `"other:<s>"`.
    #[must_use]
    pub fn tag(&self) -> String {
        match self {
            Self::TypeScript => "typescript".to_owned(),
            Self::JavaScript => "javascript".to_owned(),
            Self::Python => "python".to_owned(),
            Self::Rust => "rust".to_owned(),
            Self::Go => "go".to_owned(),
            Self::Java => "java".to_owned(),
            Self::Kotlin => "kotlin".to_owned(),
            Self::CSharp => "csharp".to_owned(),
            Self::C => "c".to_owned(),
            Self::Cpp => "cpp".to_owned(),
            Self::Other(s) => format!("other:{s}"),
        }
    }

    /// Inverse of [`Lang::tag`]. The `"other:<name>"` form rehydrates to
    /// `Other(&'static str)` by leaking the suffix into the binary's static
    /// segment.
    #[must_use]
    pub fn from_tag(tag: &str) -> Option<Self> {
        Some(match tag {
            "typescript" => Self::TypeScript,
            "javascript" => Self::JavaScript,
            "python" => Self::Python,
            "rust" => Self::Rust,
            "go" => Self::Go,
            "java" => Self::Java,
            "kotlin" => Self::Kotlin,
            "csharp" => Self::CSharp,
            "c" => Self::C,
            "cpp" => Self::Cpp,
            other => {
                let suffix = other.strip_prefix("other:")?;
                Self::Other(Box::leak(suffix.to_owned().into_boxed_str()))
            }
        })
    }
}

impl Serialize for Lang {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.tag())
    }
}

impl<'de> Deserialize<'de> for Lang {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl serde::de::Visitor<'_> for Visitor {
            type Value = Lang;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a Lang tag string")
            }
            fn visit_str<E: DeError>(self, v: &str) -> Result<Lang, E> {
                Lang::from_tag(v).ok_or_else(|| E::invalid_value(Unexpected::Str(v), &"a Lang tag"))
            }
        }
        deserializer.deserialize_str(Visitor)
    }
}
