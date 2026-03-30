pub mod config;
mod csharp;
mod go;
pub(crate) mod helpers;
mod java;
mod json_lang;
mod markdown;
mod python;
pub mod registry;
mod rust_lang;
pub mod symbols;
pub mod traits;
mod typescript;
mod yaml;

pub use registry::{ParseOutcome, ParserRegistry};
pub use symbols::SymbolExtractor;
pub use traits::{ImportKind, ImportResolver, LanguageParser, RawExport, RawImport};

/// Create a TypeScript/JS symbol extractor for testing.
pub fn typescript_symbol_extractor() -> impl SymbolExtractor {
    typescript::TypeScriptParser::new()
}

/// Create a Rust symbol extractor for testing.
pub fn rust_symbol_extractor() -> impl SymbolExtractor {
    rust_lang::RustParser::with_crate_name(None)
}

/// Create a Go symbol extractor for testing.
pub fn go_symbol_extractor() -> impl SymbolExtractor {
    go::GoSymbolExtractor
}

/// Create a Python symbol extractor for testing.
pub fn python_symbol_extractor() -> impl SymbolExtractor {
    python::PythonParser::new()
}

/// Create a C# symbol extractor for testing.
pub fn csharp_symbol_extractor() -> impl SymbolExtractor {
    csharp::CSharpParser
}

/// Create a Java symbol extractor for testing.
pub fn java_symbol_extractor() -> impl SymbolExtractor {
    java::JavaParser
}
