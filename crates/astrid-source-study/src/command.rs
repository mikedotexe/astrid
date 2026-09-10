use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum Command {
    Map { topic: String, page: usize },
    Find { query: String, page: usize },
    Open { source: String, line: usize },
    Resume { source: String },
    Continue,
    Relate { symbol: String, page: usize },
    Question(crate::QuestionCommand),
    Session { targets: Vec<(String, usize)> },
    Trace { target: String },
}

impl Command {
    /// Parse a source-study Action without changing case-sensitive paths.
    /// # Errors
    /// Returns an error for invalid line or page numbers.
    pub fn parse(input: &str) -> Result<Self> {
        let input = input.trim();
        let input = input.strip_prefix("SELF_STUDY").unwrap_or(input).trim();
        let (verb, rest) = input.split_once(' ').unwrap_or((input, ""));
        if verb.eq_ignore_ascii_case("REPLACE") {
            return Self::parse_replacement(rest);
        }
        match verb.to_ascii_uppercase().as_str() {
            "" | "CONTINUE" => Ok(Self::Continue),
            "QUESTION" => Ok(Self::Question(crate::QuestionCommand::parse(rest)?)),
            "RELATE" => {
                let (symbol, page) = page_suffix(rest)?;
                Ok(Self::Relate {
                    symbol: symbol.into(),
                    page,
                })
            },
            "TRACE" => Ok(Self::Trace {
                target: if rest.trim().is_empty() {
                    "LAST".into()
                } else {
                    rest.trim().into()
                },
            }),
            "SESSION" => {
                let targets = rest
                    .split(" | ")
                    .map(|target| {
                        if !target.trim().starts_with("OPEN ") {
                            bail!("session targets must be OPEN repository/path [line]");
                        }
                        match Self::parse(target)? {
                            Self::Open { source, line } => Ok((source, line)),
                            _ => bail!("session targets must be OPEN repository/path [line]"),
                        }
                    })
                    .collect::<Result<Vec<_>>>()?;
                if !(2..=3).contains(&targets.len()) {
                    bail!(
                        "use SESSION OPEN repository/path line | OPEN repository/path line [| OPEN repository/path line]; two or three chosen pages, one response"
                    );
                }
                Ok(Self::Session { targets })
            },
            "MAP" => {
                let (topic, page) = page_suffix(rest)?;
                Ok(Self::Map {
                    topic: topic.into(),
                    page,
                })
            },
            "FIND" => {
                let (query, page) = page_suffix(rest)?;
                if query.is_empty() {
                    bail!("use SELF_STUDY FIND <literal text> [--page N]");
                }
                Ok(Self::Find {
                    query: query.into(),
                    page,
                })
            },
            "RESUME" => Ok(Self::Resume {
                source: rest.into(),
            }),
            "OPEN" => {
                let (source, line) = match rest.rsplit_once(' ') {
                    Some((source, number)) if number.parse::<usize>().is_ok() => {
                        (source, number.parse::<usize>()?)
                    },
                    _ => (rest, 1),
                };
                if source.is_empty() || line == 0 {
                    bail!("use SELF_STUDY OPEN repository/path [one-based line]");
                }
                Ok(Self::Open {
                    source: source.into(),
                    line,
                })
            },
            _ => Ok(Self::Resume {
                source: input.into(),
            }),
        }
    }

    fn parse_replacement(rest: &str) -> Result<Self> {
        let inner = rest.trim();
        let (operation, argument) = inner.split_once(char::is_whitespace).unwrap_or((inner, ""));
        let argument = argument.trim();
        if !matches!(
            operation.to_ascii_uppercase().as_str(),
            "MAP"
                | "FIND"
                | "OPEN"
                | "RESUME"
                | "CONTINUE"
                | "RELATE"
                | "QUESTION"
                | "SESSION"
                | "TRACE"
        ) {
            bail!(
                "REPLACE requires one ordinary source-study operation; nested REPLACE and private WRITE are not source-study operations"
            );
        }
        if operation.eq_ignore_ascii_case("RESUME") && argument.is_empty() {
            bail!(
                "REPLACE RESUME requires a source path; use REPLACE CONTINUE to continue the current source"
            );
        }
        if operation.eq_ignore_ascii_case("CONTINUE") && !argument.is_empty() {
            bail!(
                "REPLACE CONTINUE takes no arguments; use REPLACE OPEN or REPLACE RESUME for a chosen source"
            );
        }
        Self::parse(&format!("{operation} {argument}"))
    }
}

fn page_suffix(text: &str) -> Result<(&str, usize)> {
    if let Some((text, number)) = text
        .strip_prefix("--page ")
        .map(|number| ("", number))
        .or_else(|| text.rsplit_once(" --page "))
    {
        let page = number.parse::<usize>()?;
        if page == 0 {
            bail!("navigation pages start at 1");
        }
        Ok((text.trim(), page))
    } else {
        Ok((text.trim(), 1))
    }
}
