//! Prefix commands: `!play some song` → command `play` with arguments.
//!
//! [`parse`] splits one message; [`Commands`] maps names and aliases (e.g.
//! a Turkish and an English name) to your own command type.
//!
//! ```
//! use era_bot_sdk::command::{Commands, parse};
//!
//! let command = parse("!", "!play  never gonna \"give you\" up").unwrap();
//! assert_eq!(command.name, "play");
//! assert_eq!(command.rest, "never gonna \"give you\" up");
//! assert_eq!(command.args, ["never", "gonna", "give you", "up"]);
//!
//! #[derive(Debug, Clone, Copy, PartialEq, Eq)]
//! enum Cmd { Play, Skip }
//! let commands = Commands::new("!")
//!     .add(Cmd::Play, &["çal", "play", "p"])
//!     .add(Cmd::Skip, &["geç", "skip"]);
//! let (cmd, parsed) = commands.parse("!ÇAL lofi").unwrap();
//! assert_eq!((cmd, parsed.rest), (Cmd::Play, "lofi"));
//! assert!(commands.parse("!dance").is_none());
//! ```

use std::collections::HashMap;

/// One parsed command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command<'a> {
    /// The command name after the prefix, lowercased (`"play"` for `!Play`).
    pub name: String,
    /// The text after the name with surrounding whitespace removed, exactly
    /// as typed (a search query, a URL, ...).
    pub rest: &'a str,
    /// [`Command::rest`] split at whitespace; `"double quotes"` keep spaces
    /// together.
    pub args: Vec<String>,
}

impl Command<'_> {
    /// The `index`th argument, if present.
    #[must_use]
    pub fn arg(&self, index: usize) -> Option<&str> {
        self.args.get(index).map(String::as_str)
    }
}

/// Parses `content` as a command with `prefix` (e.g. `"!"`). `None` when the
/// message does not start with the prefix or has no name after it.
#[must_use]
pub fn parse<'a>(prefix: &str, content: &'a str) -> Option<Command<'a>> {
    let body = content.trim_start().strip_prefix(prefix)?;
    let name_end = body.find(char::is_whitespace).unwrap_or(body.len());
    let (name, rest) = body.split_at(name_end);
    if name.is_empty() {
        return None;
    }
    let rest = rest.trim();
    Some(Command {
        name: name.to_lowercase(),
        rest,
        args: split_args(rest),
    })
}

/// Splits at whitespace; text in double quotes stays one argument (the
/// quotes are removed). An unclosed quote runs to the end.
#[must_use]
pub fn split_args(text: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut has_arg = false;
    for c in text.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                has_arg = true;
            }
            c if c.is_whitespace() && !quoted => {
                if has_arg {
                    args.push(std::mem::take(&mut current));
                    has_arg = false;
                }
            }
            c => {
                current.push(c);
                has_arg = true;
            }
        }
    }
    if has_arg {
        args.push(current);
    }
    args
}

/// A set of commands with aliases, mapped to your own type `C`.
#[derive(Debug, Clone)]
pub struct Commands<C> {
    prefix: String,
    names: HashMap<String, C>,
}

impl<C: Copy> Commands<C> {
    /// An empty set for `prefix` (e.g. `"!"`).
    #[must_use]
    pub fn new(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
            names: HashMap::new(),
        }
    }

    /// Adds `command` under each of `names` (matched case-insensitively).
    #[must_use]
    pub fn add(mut self, command: C, names: &[&str]) -> Self {
        for name in names {
            self.names.insert(name.to_lowercase(), command);
        }
        self
    }

    /// The prefix.
    #[must_use]
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    /// The command a message invokes, with its parsed form. `None` for
    /// messages without the prefix or with an unknown name; use [`parse`]
    /// to tell those apart.
    #[must_use]
    pub fn parse<'a>(&self, content: &'a str) -> Option<(C, Command<'a>)> {
        let command = parse(&self.prefix, content)?;
        let found = *self.names.get(&command.name)?;
        Some((found, command))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_names_and_arguments() {
        let command = parse("!", "  !Ses 150 ").unwrap();
        assert_eq!(command.name, "ses");
        assert_eq!(command.rest, "150");
        assert_eq!(command.arg(0), Some("150"));
        assert_eq!(command.arg(1), None);

        let bare = parse("!", "!geç").unwrap();
        assert_eq!(bare.name, "geç");
        assert_eq!(bare.rest, "");
        assert!(bare.args.is_empty());

        // URLs stay intact in `rest`.
        let url = parse("!", "!play https://youtu.be/abc?t=1&x=2").unwrap();
        assert_eq!(url.rest, "https://youtu.be/abc?t=1&x=2");
    }

    #[test]
    fn rejects_non_commands() {
        assert_eq!(parse("!", "hello !play"), None);
        assert_eq!(parse("!", "!"), None);
        assert_eq!(parse("!", "! play"), None);
        assert_eq!(parse("!", ""), None);
        assert_eq!(parse("?", "!play"), None);
    }

    #[test]
    fn longer_prefixes_and_unicode() {
        let command = parse("panda ", "panda ÇAL Barış Manço").unwrap();
        assert_eq!(command.name, "çal");
        assert_eq!(command.args, ["Barış", "Manço"]);
    }

    #[test]
    fn quotes_group_arguments() {
        assert_eq!(split_args(r#"a "b c" d"#), ["a", "b c", "d"]);
        assert_eq!(split_args(r#""""#), [""]);
        assert_eq!(split_args(r#"x "open quote"#), ["x", "open quote"]);
        assert_eq!(split_args("  \t "), Vec::<String>::new());
        assert_eq!(split_args(r#"say"hi""#), ["sayhi"]);
    }

    #[test]
    fn command_sets_map_aliases() {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum Cmd {
            Play,
            Help,
        }
        let commands = Commands::new("!")
            .add(Cmd::Play, &["Çal", "play"])
            .add(Cmd::Help, &["yardım", "help"]);
        assert_eq!(commands.prefix(), "!");
        assert_eq!(commands.parse("!çal x").map(|(c, _)| c), Some(Cmd::Play));
        assert_eq!(commands.parse("!PLAY x").map(|(c, _)| c), Some(Cmd::Play));
        assert_eq!(commands.parse("!Yardım").map(|(c, _)| c), Some(Cmd::Help));
        assert_eq!(commands.parse("!nope").map(|(c, _)| c), None);
        assert_eq!(commands.parse("play").map(|(c, _)| c), None);
    }
}
