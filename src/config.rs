use anyhow::{Context, Result};
use std::path::Path;

/// Parsed configuration from a `.worktreelinks` file.
#[derive(Debug)]
pub struct Config {
    /// Glob patterns that select files/directories to link.
    pub patterns: Vec<String>,
}

impl Config {
    /// Read and parse a `.worktreelinks` file.
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;
        Ok(Self::parse(&content))
    }

    /// Parse the content of a `.worktreelinks` file.
    /// Lines starting with `#` are comments. Inline `#` is not stripped
    /// and is treated as part of the pattern (matching `.gitignore` semantics).
    fn parse(content: &str) -> Self {
        let patterns = content
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(|line| line.to_string())
            .collect();
        Config { patterns }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ignores_comments_and_blank_lines() {
        let input = r#"
# This is a comment
node_modules

.env
.env.*

# Build artifacts
.next/
dist/
        "#;
        let config = Config::parse(input);
        assert_eq!(
            config.patterns,
            vec!["node_modules", ".env", ".env.*", ".next/", "dist/"]
        );
    }

    #[test]
    fn parse_empty_file() {
        let config = Config::parse("");
        assert!(config.patterns.is_empty());
    }

    #[test]
    fn parse_only_comments() {
        let config = Config::parse("# comment\n# another");
        assert!(config.patterns.is_empty());
    }
}
