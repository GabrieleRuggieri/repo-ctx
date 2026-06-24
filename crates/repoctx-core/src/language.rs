//! Language detection from file extensions.

use std::path::Path;

/// Supported language identifiers for the v1 heuristic extractor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    /// Rust source files.
    Rust,
    /// TypeScript source files.
    TypeScript,
    /// JavaScript source files.
    JavaScript,
    /// Python source files.
    Python,
    /// Go source files.
    Go,
    /// Java source files.
    Java,
    /// Unknown or unsupported language.
    Unknown,
}

impl Language {
    /// Returns the canonical language id stored in the index.
    pub fn id(self) -> &'static str {
        match self {
            Language::Rust => "rust",
            Language::TypeScript => "typescript",
            Language::JavaScript => "javascript",
            Language::Python => "python",
            Language::Go => "go",
            Language::Java => "java",
            Language::Unknown => "unknown",
        }
    }

    /// Returns true when the language has a heuristic extractor.
    pub fn is_supported(self) -> bool {
        !matches!(self, Language::Unknown)
    }
}

/// Detects language from a file path extension.
///
/// # Arguments
///
/// * `path` - Repository-relative or absolute file path.
pub fn detect_language(path: &Path) -> Language {
    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => Language::Rust,
        Some("ts" | "tsx") => Language::TypeScript,
        Some("js" | "jsx" | "mjs" | "cjs") => Language::JavaScript,
        Some("py" | "pyi") => Language::Python,
        Some("go") => Language::Go,
        Some("java") => Language::Java,
        _ => Language::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_rust_and_python() {
        assert_eq!(detect_language(Path::new("src/main.rs")), Language::Rust);
        assert_eq!(detect_language(Path::new("app.py")), Language::Python);
    }
}
