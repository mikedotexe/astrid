//! Explicit operator CLI. No stdin protocol, export path, logging, or model route.
use anyhow::{Context as _, Result, bail, ensure};
use astrid_source_study::research::{MAX_PAGE_SIZE, ResearchReader, Section};
use std::{io::Write as _, path::PathBuf};

const USAGE: &str = "writing-research --state-directory OWNER_DIRECTORY --owner astrid|minime list [--page N] [--page-size 1..20]\nwriting-research --state-directory OWNER_DIRECTORY --owner astrid|minime show --draft dN --revision SNAPSHOT_SHA256 [passages|questions|references|stopping-point|annotations]";

fn run() -> Result<()> {
    let mut args = std::env::args_os().skip(1).peekable();
    if args.len() == 1 && args.peek().is_some_and(|arg| arg == "--help") {
        println!(
            "{USAGE}\nList status is current selection/finish state. not_selected does not establish an explicit parking choice or a later return."
        );
        return Ok(());
    }
    let mut directory = None;
    let mut owner = None;
    let command = loop {
        let arg = args.next().context(USAGE)?;
        match arg.to_str() {
            Some("--state-directory") if directory.is_none() => {
                directory = Some(PathBuf::from(args.next().context(USAGE)?));
            },
            Some("--owner") if owner.is_none() => {
                owner = Some(
                    args.next()
                        .context(USAGE)?
                        .into_string()
                        .map_err(|_| anyhow::anyhow!(USAGE))?,
                );
            },
            Some("list") => break "list",
            Some("show") => break "show",
            _ => bail!(USAGE),
        }
    };
    let reader = ResearchReader::new(directory.context(USAGE)?, owner.context(USAGE)?);
    let rest: Vec<String> = args
        .map(|arg| arg.into_string().map_err(|_| anyhow::anyhow!(USAGE)))
        .collect::<Result<_>>()?;
    let output = match command {
        "list" => {
            let mut page = None;
            let mut size = None;
            let mut args = rest.iter();
            while let Some(arg) = args.next() {
                let destination = match arg.as_str() {
                    "--page" if page.is_none() => &mut page,
                    "--page-size" if size.is_none() => &mut size,
                    _ => bail!(USAGE),
                };
                *destination = Some(
                    args.next()
                        .context(USAGE)?
                        .parse::<usize>()
                        .map_err(|_| anyhow::anyhow!(USAGE))?,
                );
            }
            serde_json::to_vec(&reader.list(page.unwrap_or(1), size.unwrap_or(MAX_PAGE_SIZE))?)?
        },
        "show" => {
            let mut draft = None;
            let mut revision = None;
            let mut section = None;
            let mut args = rest.iter();
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--draft" if draft.is_none() => draft = Some(args.next().context(USAGE)?),
                    "--revision" if revision.is_none() => {
                        revision = Some(args.next().context(USAGE)?);
                    },
                    name => {
                        ensure!(section.is_none(), "{USAGE}");
                        section = Some(match name {
                            "passages" => Section::Passages,
                            "questions" => Section::Questions,
                            "references" => Section::References,
                            "stopping-point" => Section::StoppingPoint,
                            "annotations" => Section::Annotations,
                            _ => bail!(USAGE),
                        });
                    },
                }
            }
            serde_json::to_vec(&reader.show(
                draft.context(USAGE)?,
                revision.context(USAGE)?,
                section.unwrap_or(Section::Passages),
            )?)?
        },
        _ => unreachable!(),
    };
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(&output)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            // No error chain, request value, file bytes, or selected prose is logged.
            eprintln!("writing-research: {error}");
            std::process::ExitCode::FAILURE
        },
    }
}
