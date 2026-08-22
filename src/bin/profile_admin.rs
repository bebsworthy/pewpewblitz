use std::path::PathBuf;

fn main() {
    if let Err(error) = run() {
        eprintln!("profile backup failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut arguments = std::env::args_os().skip(1);
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new("backup")) {
        return Err("usage: brawler-profile-admin backup --database <path> --output <path>".into());
    }
    let mut database: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--database") => database = arguments.next().map(PathBuf::from),
            Some("--output") => output = arguments.next().map(PathBuf::from),
            _ => {
                return Err(
                    "usage: brawler-profile-admin backup --database <path> --output <path>".into(),
                );
            }
        }
    }
    let database = database.ok_or_else(|| "missing --database".to_string())?;
    let output = output.ok_or_else(|| "missing --output".to_string())?;
    brawler::profiles::backup_database(&database, &output).map_err(|error| error.to_string())
}
