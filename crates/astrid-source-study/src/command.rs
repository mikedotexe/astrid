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
}

impl Command {
    /// Parse a source-study Action without changing case-sensitive paths.
    /// # Errors
    /// Returns an error for invalid line or page numbers.
    pub fn parse(input: &str) -> Result<Self> {
        let input = input.trim();
        let input = input.strip_prefix("SELF_STUDY").unwrap_or(input).trim();
        let (verb, rest) = input.split_once(' ').unwrap_or((input, ""));
        match verb.to_ascii_uppercase().as_str() {
            "" | "CONTINUE" => Ok(Self::Continue),
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
