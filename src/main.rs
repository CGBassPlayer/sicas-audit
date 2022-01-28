use anyhow::{anyhow, Result};
use std::{fs::File, io::Read, path::Path, str::FromStr};
use std::ffi::OsStr;
use clap::{Parser, AppSettings, Subcommand};
use configparser::ini::Ini;
use log::LevelFilter;
use simple_logger::SimpleLogger;
use zip::{ZipArchive};


const EMPTY_STRING: &str = "";

#[derive(Parser)]
#[clap(author, version)]
#[clap(global_setting(AppSettings::UseLongFormatForHelpSubcommand))]
struct Args {
    /// Name of the jar file
    #[clap(short, long)]
    jar: String,

    /// Show debug information
    #[clap(short, long)]
    verbose: bool,

    /// Configuration file location
    #[clap(short, long, default_value = "config.ini")]
    config: String,

    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Display contents of the archived file
    Show {
        /// Name of the audit trail file
        file: Option<String>
    },
    /// List all within the archive. This can be customized in the configuration file
    List,
    /// Edit a file within the archive
    Edit {
        /// Name of the file from the archive. If no file is provided, the value in the configuration file is used
        file: Option<String>
    },
    /// Remove a file from the archive
    Delete {
        /// Name of the file from the archive
        file: String
    },
}

fn main() -> Result<()> {
    let args: Args = Args::parse();
    if !Path::new(&args.jar).exists() {
        return Err(anyhow!("Unable to open JAR file: {:?}", args.jar));
    }

    let mut config = Ini::new();
    let _ = config.load(&args.config)
        .expect("Unable to load configuration");
    init_simple_logger(&args, &config);

    match args.command {
        Commands::Show { file } => {
            log::debug!("Retrieving archive file name");
            let file = file.unwrap_or_else(|| config.get("AUDIT", "AUDIT_FILE")
                .unwrap_or_else(|| "AUDIT_TRAIL".to_string()));

            log::debug!("Retrieving file contents");
            let audit_trail = retrieve_archive_file_contents(&args.jar, &file)?;

            log::debug!("printing file contents");
            println!("{}", audit_trail);
        }
        Commands::List => {
            log::debug!("Listing files in archive");
            let ignored_str = config.get("AUDIT", "IGNORED_FILES").unwrap_or_else(|| EMPTY_STRING.to_string());
            let ignored_files = ignored_str.split(", ").collect::<Vec<&str>>();
            let archive_files = traverse_archive_file(&args.jar, ignored_files)?;

            log::debug!("archive file count: {}", archive_files.len());
            println!("{:#?}", archive_files);
        }
        Commands::Edit { file } => {
            let edit_file = file.unwrap_or_else(|| config.get("AUDIT", "AUDIT_FILE")
                .unwrap_or_else(|| "AUDIT_TRAIL".to_string()));
            log::debug!("Editing {:?}", edit_file);
            let file_contents = retrieve_archive_file_contents(&args.jar, &edit_file)?;
            let edited = edit::edit(&file_contents)?;

            if file_contents.eq(&edited) {
                println!("No Changes were made. Exiting...");
                return Ok(());
            }

            log::info!("Updating {}", &args.jar);
        }
        Commands::Delete { file } => {
            println!("Deleting {}", file);
        }
    }

    Ok(())
}

fn init_simple_logger(args: &Args, config: &Ini) {
    let logging_level = config.get("LOGGING", "LOG_LEVEL")
        .map_or_else(|| LevelFilter::Info, |lvl| LevelFilter::from_str(lvl.as_str()).unwrap());

    let mut simple_logger = SimpleLogger::new()
        .with_colors(true)
        .with_level(logging_level);

    if args.verbose {
        simple_logger = simple_logger.with_level(LevelFilter::Debug);
    }

    simple_logger
        .init()
        .unwrap();
}

fn retrieve_archive_file_contents(jar: &str, archive_file_name: &str) -> Result<String> {
    let jar_file = File::open(jar)?;
    let mut archive = ZipArchive::new(jar_file)?;
    let mut archive_file = archive.by_name(archive_file_name)?;
    let mut file_contents = String::new();

    archive_file.read_to_string(&mut file_contents)?;
    Ok(file_contents)
}

fn traverse_archive_file(jar: &str, ignored_files: Vec<&str>) -> Result<Vec<String>> {
    let jar_file = File::open(jar)?;
    let mut archive = ZipArchive::new(jar_file)?;
    let mut archive_files = Vec::new();

    'outer: for index in 0..archive.len() {
        let file = archive.by_index(index)?;
        for ignored_file in &ignored_files {
            if file.is_dir() || ignored_file.ends_with('/') && file.name().contains(ignored_file) {
                continue 'outer;
            } else if file.is_file() {
                if ignored_file.starts_with('.') {
                    let file_extension = get_file_extension(file.name());
                    if file_extension.eq_ignore_ascii_case(ignored_file) {
                        continue 'outer;
                    }
                } else {
                    let file_name = get_file_name(file.name())
                        .unwrap_or(EMPTY_STRING);

                    if file_name.starts_with(ignored_file) {
                        continue 'outer;
                    }
                }
            }
        }

        archive_files.push(file.name().to_owned());
    }

    Ok(archive_files)
}

fn get_file_name(file_path: &str) -> Option<&str> {
    Path::new(file_path)
        .file_name()
        .and_then(OsStr::to_str)
}

fn get_file_extension(file_path: &str) -> &str {
    file_path
        .rfind('.')
        .map(|idx| &file_path[idx..])
        .filter(|ext| ext.chars().skip(1).all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or(EMPTY_STRING)
}