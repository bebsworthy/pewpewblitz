//! Offline operator CLI for the lobby's profile database: backups and published-map moderation.
//!
//! `maps` commands open the live WAL database beside a running lobby; the lobby re-reads approved
//! maps on a bounded interval, so a decision reaches queue rotation without a restart.

use brawler::{
    lobby::PublishedMapId,
    profiles::{ProfileStorage, PublishedMapAction, PublishedMapRecord, PublishedMapStatus},
};
use std::path::PathBuf;

const USAGE: &str = "usage:
  brawler-profile-admin backup --database <path> --output <path>
  brawler-profile-admin maps list --database <path> [--state pending|approved|rejected|removed|all]
  brawler-profile-admin maps approve <id> --database <path>
  brawler-profile-admin maps reject <id> --reason <text> --database <path>
  brawler-profile-admin maps remove <id> [--reason <text>] --database <path>";

fn main() {
    if let Err(error) = run() {
        eprintln!("brawler-profile-admin: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut arguments = std::env::args().skip(1);
    match arguments.next().as_deref() {
        Some("backup") => backup(arguments),
        Some("maps") => maps(arguments),
        _ => Err(USAGE.into()),
    }
}

#[derive(Default)]
struct Options {
    database: Option<PathBuf>,
    output: Option<PathBuf>,
    state: Option<String>,
    reason: Option<String>,
    positional: Vec<String>,
}

fn parse_options(arguments: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut options = Options::default();
    let mut arguments = arguments.peekable();
    while let Some(argument) = arguments.next() {
        let mut value = || {
            arguments
                .next()
                .ok_or_else(|| format!("{argument} requires a value"))
        };
        match argument.as_str() {
            "--database" => options.database = Some(PathBuf::from(value()?)),
            "--output" => options.output = Some(PathBuf::from(value()?)),
            "--state" => options.state = Some(value()?),
            "--reason" => options.reason = Some(value()?),
            flag if flag.starts_with("--") => return Err(format!("unknown flag {flag}\n{USAGE}")),
            _ => options.positional.push(argument),
        }
    }
    Ok(options)
}

fn backup(arguments: impl Iterator<Item = String>) -> Result<(), String> {
    let options = parse_options(arguments)?;
    if !options.positional.is_empty() || options.state.is_some() || options.reason.is_some() {
        return Err(USAGE.into());
    }
    let database = options.database.ok_or("missing --database")?;
    let output = options.output.ok_or("missing --output")?;
    brawler::profiles::backup_database(&database, &output).map_err(|error| error.to_string())
}

fn maps(mut arguments: impl Iterator<Item = String>) -> Result<(), String> {
    let command = arguments.next().ok_or(USAGE)?;
    let options = parse_options(arguments)?;
    if options.output.is_some() {
        return Err(USAGE.into());
    }
    let database = options.database.clone().ok_or("missing --database")?;
    let mut storage = ProfileStorage::open(&database).map_err(|error| error.to_string())?;
    if command == "list" {
        if !options.positional.is_empty() || options.reason.is_some() {
            return Err(USAGE.into());
        }
        let status = match options.state.as_deref().unwrap_or("pending") {
            "all" => None,
            label => Some(
                PublishedMapStatus::from_label(label)
                    .ok_or_else(|| format!("unknown --state {label}"))?,
            ),
        };
        let records = storage
            .list_published_maps(status)
            .map_err(|error| error.to_string())?;
        println!(
            "id\tstate\tgame_type\tname\tauthor\taccount\tsize\tplacements\tsubmitted_unix_ms\treason"
        );
        for record in &records {
            println!("{}", list_row(record));
        }
        return Ok(());
    }
    if options.state.is_some() {
        return Err(USAGE.into());
    }
    let [id] = options.positional.as_slice() else {
        return Err(USAGE.into());
    };
    let published_map_id = id
        .parse::<u64>()
        .ok()
        .and_then(PublishedMapId::new)
        .ok_or_else(|| format!("invalid published map id {id}"))?;
    let action = match (command.as_str(), options.reason) {
        ("approve", None) => PublishedMapAction::Approve,
        ("reject", Some(reason)) => PublishedMapAction::Reject { reason },
        ("reject", None) => return Err("reject requires --reason".into()),
        ("remove", reason) => PublishedMapAction::Remove { reason },
        _ => return Err(USAGE.into()),
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    let record = storage
        .decide_published_map(
            published_map_id,
            &action,
            u64::try_from(now).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
    println!(
        "published map {} \"{}\" is now {}",
        record.published_map_id.get(),
        record.name,
        record.status.label()
    );
    Ok(())
}

fn list_row(record: &PublishedMapRecord) -> String {
    let recipe = postcard::from_bytes::<brawler::map::MapRecipe>(&record.recipe).ok();
    let (size, placements) = recipe.map_or_else(
        || ("?".to_string(), "?".to_string()),
        |recipe| {
            (
                format!("{}x{}", recipe.dimensions.width, recipe.dimensions.height),
                recipe.placements.len().to_string(),
            )
        },
    );
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        record.published_map_id.get(),
        record.status.label(),
        record.game_type_id.as_str(),
        record.name,
        record.author_display_name,
        record.account_id,
        size,
        placements,
        record.submitted_at_unix_ms,
        record.reason.as_deref().unwrap_or("")
    )
}
