//! Pure, I/O-free parser for leading slash-commands in queued messages.
//!
//! Semantics (see also [`parse`]):
//! - Whitespace between commands is optional (`/a/b` and `/a /b` both work);
//!   leading whitespace before `/` is skipped.
//! - A command name is the longest `[A-Za-z0-9_]` run after `/`; any other
//!   character terminates the name and belongs to the remainder.
//! - Parsing stops at the first unknown (or empty) `/token`: its raw text and
//!   everything after it become the message content verbatim. If no command
//!   matched at all, the message is left untouched (`None`).
//! - The parser never validates step counts or side effects — occurrence
//!   order is preserved for the executor to expand via the registry.

use std::collections::HashSet;

/// Result of parsing leading commands out of a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCommands {
    /// Registered command names, in occurrence order (repeats allowed).
    pub invocations: Vec<String>,
    /// Message content after the last recognized command, whitespace-trimmed.
    /// When parsing stopped at an unknown "/token", the remainder starts with
    /// that token VERBATIM (it and everything after it is plain text).
    pub remainder: String,
}

/// Extract the leading command invocations from a queued message's content.
///
/// Returns `None` when the message carries no recognized command (it does not
/// start with `/` after trimming, or the first token is unknown) — the caller
/// must leave such messages byte-for-byte untouched. Returns `Some(..)` as
/// soon as at least one command matched.
///
/// # Examples
///
/// ```
/// use std::collections::HashSet;
/// use rhd_plugin_commands::parser::{parse, ParsedCommands};
///
/// let registry: HashSet<String> = ["a", "b"].iter().map(|s| s.to_string()).collect();
///
/// // Plain text carries no commands.
/// assert_eq!(parse("hello", &registry), None);
/// // Adjacent and whitespace-separated commands both chain.
/// assert_eq!(
///     parse("/a/b x", &registry),
///     Some(ParsedCommands { invocations: vec!["a".into(), "b".into()], remainder: "x".into() })
/// );
/// // An unknown token halts parsing and survives verbatim in the remainder.
/// assert_eq!(
///     parse("/a /unknown x", &registry),
///     Some(ParsedCommands { invocations: vec!["a".into()], remainder: "/unknown x".into() })
/// );
/// // Nothing is stripped unless the very first token is a command.
/// assert_eq!(parse("/unknown /a x", &registry), None);
/// ```
pub fn parse(content: &str, registry: &HashSet<String>) -> Option<ParsedCommands> {
    let chars: Vec<char> = content.chars().collect();
    let mut i = 0usize;
    let mut invocations: Vec<String> = Vec::new();

    loop {
        // 1. skip any whitespace before the next token
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() {
            break; // consumed everything
        }
        if chars[i] != '/' {
            break; // plain-text remainder starts here
        }

        // 2. read the command name: [A-Za-z0-9_]+
        let mut j = i + 1;
        while j < chars.len() && (chars[j].is_ascii_alphanumeric() || chars[j] == '_') {
            j += 1;
        }
        let name: String = chars[i + 1..j].iter().collect();

        // 3. empty or unknown name -> stop; remainder is the raw text from '/'
        if name.is_empty() || !registry.contains(&name) {
            if invocations.is_empty() {
                return None; // untouched message
            }
            let remainder: String = chars[i..].iter().collect();
            return Some(ParsedCommands {
                invocations,
                remainder: remainder.trim().to_string(),
            });
        }

        invocations.push(name);
        i = j; // continue right after the name
    }

    if invocations.is_empty() {
        return None;
    }
    let remainder: String = chars[i..].iter().collect();
    Some(ParsedCommands {
        invocations,
        remainder: remainder.trim().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Registry fixture matching the semantics table: commands `a` and `b`.
    fn registry() -> HashSet<String> {
        ["a", "b"].iter().map(|s| s.to_string()).collect()
    }

    fn parsed(names: &[&str], remainder: &str) -> Option<ParsedCommands> {
        Some(ParsedCommands {
            invocations: names.iter().map(|s| s.to_string()).collect(),
            remainder: remainder.to_string(),
        })
    }

    #[test]
    fn test_parse_plain_text() {
        // "hello" -> None (does not start with '/')
        assert_eq!(parse("hello", &registry()), None);
    }

    #[test]
    fn test_parse_single_command_with_content() {
        // "/a hello" -> ["a"], "hello"
        assert_eq!(parse("/a hello", &registry()), parsed(&["a"], "hello"));
    }

    #[test]
    fn test_parse_leading_whitespace() {
        // "  \n /a" -> ["a"], "" (leading whitespace is skipped)
        assert_eq!(parse("  \n /a", &registry()), parsed(&["a"], ""));
    }

    #[test]
    fn test_parse_adjacent_commands() {
        // "/a/b x" -> ["a","b"], "x"
        assert_eq!(parse("/a/b x", &registry()), parsed(&["a", "b"], "x"));
    }

    #[test]
    fn test_parse_whitespace_separated_commands() {
        // "/a /b" -> ["a","b"], ""
        assert_eq!(parse("/a /b", &registry()), parsed(&["a", "b"], ""));
    }

    #[test]
    fn test_parse_unknown_token_halts_and_survives_verbatim() {
        // "/a /unknown x" -> ["a"], "/unknown x" (parsing stopped, raw text kept)
        assert_eq!(
            parse("/a /unknown x", &registry()),
            parsed(&["a"], "/unknown x")
        );
        // Interior of the stopped remainder keeps its exact text; edges trim.
        assert_eq!(
            parse("/a  /unknown  x ", &registry()),
            parsed(&["a"], "/unknown  x")
        );
    }

    #[test]
    fn test_parse_first_token_unknown() {
        // "/unknown /a x" -> None (message untouched; /a is NOT executed)
        assert_eq!(parse("/unknown /a x", &registry()), None);
    }

    #[test]
    fn test_parse_trailing_slash_empty_name() {
        // "/a/" -> ["a"], "/" (empty name after slash counts as unknown token)
        assert_eq!(parse("/a/", &registry()), parsed(&["a"], "/"));
    }

    #[test]
    fn test_parse_lone_slash() {
        // "/" -> None
        assert_eq!(parse("/", &registry()), None);
    }

    #[test]
    fn test_parse_slash_inside_content() {
        // "/a b/c" -> ["a"], "b/c"
        assert_eq!(parse("/a b/c", &registry()), parsed(&["a"], "b/c"));
    }

    #[test]
    fn test_parse_repeated_command() {
        // Repeats are allowed and preserved in occurrence order.
        assert_eq!(parse("/a /a x", &registry()), parsed(&["a", "a"], "x"));
    }

    #[test]
    fn test_parse_preserves_remainder_internals() {
        // Interior whitespace intact, edges trimmed.
        assert_eq!(
            parse("/a  one   two ", &registry()),
            parsed(&["a"], "one   two")
        );
    }

    #[test]
    fn test_parse_multibyte_content() {
        // Char-based cursor, not byte slicing.
        assert_eq!(parse("/a привет", &registry()), parsed(&["a"], "привет"));
    }

    #[test]
    fn test_parse_name_terminates_at_foreign_char() {
        // Names end at the first non-[A-Za-z0-9_] char; the rest is remainder.
        assert_eq!(parse("/a.x", &registry()), parsed(&["a"], ".x"));
        assert_eq!(parse("/a- /b", &registry()), parsed(&["a"], "- /b"));
    }
}
