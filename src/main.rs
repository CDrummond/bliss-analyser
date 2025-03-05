/**
 * Analyse music with Bliss
 *
 * Copyright (c) 2022-2025 Craig Drummond <craig.p.drummond@gmail.com>
 * GPLv3 license.
 *
 **/

use argparse::{ArgumentParser, Store, StoreTrue};
use chrono::Local;
use configparser::ini::Ini;
use dirs;
use log::LevelFilter;
use std::io::Write;
use std::path::PathBuf;
use std::process;
#[cfg(not(feature = "libav"))]
use which::which;
mod analyse;
mod cue;
mod db;
#[cfg(not(feature = "libav"))]
mod ffmpeg;
mod tags;
mod upload;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");
const TOP_LEVEL_INI_TAG: &str = "Bliss";

fn main() {
    let mut config_file = "config.ini".to_string();
    let mut db_path = "bliss.db".to_string();
    let mut logging = "info".to_string();
    let mut music_path = ".".to_string();
    let mut ignore_file = "ignore.txt".to_string();
    let mut keep_old: bool = false;
    let mut dry_run: bool = false;
    let mut task = "".to_string();
    let mut lms_host = "127.0.0.1".to_string();
    let mut lms_json_port:u16 = 9000;
    let mut max_num_files: usize = 0;
    let mut music_paths: Vec<PathBuf> = Vec::new();
    let mut max_threads: usize = 0;
    let mut use_tags = false;

    match dirs::home_dir() {
        Some(path) => {
            music_path = String::from(path.join("Music").to_string_lossy());
        }
        None => {}
    }

    {
        let config_file_help = format!("config file (default: {})", &config_file);
        let music_path_help = format!("Music folder (default: {})", &music_path);
        let db_path_help = format!("Database location (default: {})", &db_path);
        let logging_help = format!("Log level; trace, debug, info, warn, error. (default: {})", logging);
        let ignore_file_help = format!("File contains items to mark as ignored. (default: {})", ignore_file);
        let lms_host_help = format!("LMS hostname or IP address (default: {})", &lms_host);
        let lms_json_port_help = format!("LMS JSONRPC port (default: {})", &lms_json_port);
        let description = format!("Bliss Analyser v{}", VERSION);

        // arg_parse.refer 'borrows' db_path, etc, and can only have one
        // borrow per scope, hence this section is enclosed in { }
        let mut arg_parse = ArgumentParser::new();
        arg_parse.set_description(&description);
        arg_parse.refer(&mut config_file).add_option(&["-c", "--config"], Store, &config_file_help);
        arg_parse.refer(&mut music_path).add_option(&["-m", "--music"], Store, &music_path_help);
        arg_parse.refer(&mut db_path).add_option(&["-d", "--db"], Store, &db_path_help);
        arg_parse.refer(&mut logging).add_option(&["-l", "--logging"], Store, &logging_help);
        arg_parse.refer(&mut keep_old).add_option(&["-k", "--keep-old"], StoreTrue, "Don't remove files from DB if they don't exist (used with analyse task)");
        arg_parse.refer(&mut dry_run).add_option(&["-r", "--dry-run"], StoreTrue, "Dry run, only show what needs to be done (used with analyse task)");
        arg_parse.refer(&mut ignore_file).add_option(&["-i", "--ignore"], Store, &ignore_file_help);
        arg_parse.refer(&mut lms_host).add_option(&["-L", "--lms"], Store, &lms_host_help);
        arg_parse.refer(&mut lms_json_port).add_option(&["-J", "--json"], Store, &lms_json_port_help);
        arg_parse.refer(&mut max_num_files).add_option(&["-n", "--numfiles"], Store, "Maximum number of files to analyse");
        arg_parse.refer(&mut max_threads).add_option(&["-t", "--threads"], Store, "Maximum number of threads to use for analysis");
        arg_parse.refer(&mut use_tags).add_option(&["-T", "--tags"], StoreTrue, "Read/write analysis results from/to source files");
        arg_parse.refer(&mut task).add_argument("task", Store, "Task to perform; analyse, tags, ignore, upload, stopmixer.");
        arg_parse.parse_args_or_exit();
    }

    if !(logging.eq_ignore_ascii_case("trace") || logging.eq_ignore_ascii_case("debug") || logging.eq_ignore_ascii_case("info")
        || logging.eq_ignore_ascii_case("warn") || logging.eq_ignore_ascii_case("error")) {
        logging = String::from("info");
    }
    let bliss_level = if logging.eq_ignore_ascii_case("trace") { LevelFilter::Trace } else { LevelFilter::Error };
    let mut builder = env_logger::Builder::from_env(env_logger::Env::default().filter_or("XXXXXXXX", logging));
    builder.filter(Some("bliss_audio"), bliss_level);
    builder.format(|buf, record| {
        writeln!(buf, "[{} {:.1}] {}", Local::now().format("%Y-%m-%d %H:%M:%S"), record.level(), record.args())
    });
    builder.init();

    if task.is_empty() {
        log::error!("No task specified, please choose from; analyse, tags, ignore, upload");
        process::exit(-1);
    }

    if !task.eq_ignore_ascii_case("analyse") && !task.eq_ignore_ascii_case("tags") && !task.eq_ignore_ascii_case("ignore")
        && !task.eq_ignore_ascii_case("upload") && !task.eq_ignore_ascii_case("stopmixer") {
        log::error!("Invalid task ({}) supplied", task);
        process::exit(-1);
    }

    // Ensure ffmpeg is in PATH...
    #[cfg(not(feature = "libav"))]
    match which("ffmpeg") {
        Ok(_) => { }
        Err(_) => {
            log::error!("'ffmpeg' was not found! Please ensure this in your PATH");
            process::exit(-1);
        },
    }

    if !config_file.is_empty() {
        let path = PathBuf::from(&config_file);
        if path.exists() && path.is_file() {
            let mut config = Ini::new();
            match config.load(&config_file) {
                Ok(_) => {
                    let path_keys: [&str; 5] = ["music", "music_1", "music_2", "music_3", "music_4"];
                    for key in &path_keys {
                        match config.get(TOP_LEVEL_INI_TAG, key) {
                            Some(val) => { music_paths.push(PathBuf::from(&val)); }
                            None => { }
                        }
                    }
                    match config.get(TOP_LEVEL_INI_TAG, "db") {
                        Some(val) => { db_path = val; }
                        None => { }
                    }
                    match config.get(TOP_LEVEL_INI_TAG, "lms") {
                        Some(val) => { lms_host = val; }
                        None => { }
                    }
                    match config.get(TOP_LEVEL_INI_TAG, "json") {
                        Some(val) => { lms_json_port = val.parse::<u16>().unwrap(); }
                        None => { }
                    }
                    match config.get(TOP_LEVEL_INI_TAG, "ignore") {
                        Some(val) => { ignore_file = val; }
                        None => { }
                    }
                }
                Err(e) => {
                    log::error!("Failed to load config file. {}", e);
                    process::exit(-1);
                }
            }
        }
    }

    if music_paths.is_empty() {
        music_paths.push(PathBuf::from(&music_path));
    }

    if task.eq_ignore_ascii_case("stopmixer") {
        upload::stop_mixer(&lms_host, lms_json_port);
    } else {
        if db_path.len() < 3 {
            log::error!("Invalid DB path ({}) supplied", db_path);
            process::exit(-1);
        }

        let path = PathBuf::from(&db_path);
        if path.exists() && !path.is_file() {
            log::error!("DB path ({}) is not a file", db_path);
            process::exit(-1);
        }

        if task.eq_ignore_ascii_case("upload") {
            if path.exists() {
                upload::upload_db(&db_path, &lms_host, lms_json_port);
            } else {
                log::error!("DB ({}) does not exist", db_path);
                process::exit(-1);
            }
        } else {
            for mpath in &music_paths {
                if !mpath.exists() {
                    log::error!("Music path ({}) does not exist", mpath.to_string_lossy());
                    process::exit(-1);
                }
                if !mpath.is_dir() {
                    log::error!("Music path ({}) is not a directory", mpath.to_string_lossy());
                    process::exit(-1);
                }
            }

            if task.eq_ignore_ascii_case("tags") {
                analyse::read_tags(&db_path, &music_paths);
            } else if task.eq_ignore_ascii_case("ignore") {
                let ignore_path = PathBuf::from(&ignore_file);
                if !ignore_path.exists() {
                    log::error!("Ignore file ({}) does not exist", ignore_file);
                    process::exit(-1);
                }
                if !ignore_path.is_file() {
                    log::error!("Ignore file ({}) is not a file", ignore_file);
                    process::exit(-1);
                }
                analyse::update_ignore(&db_path, &ignore_path);
            } else {
                let ignore_path = PathBuf::from(&ignore_file);
                analyse::analyse_files(&db_path, &music_paths, dry_run, keep_old, max_num_files, max_threads, &ignore_path, use_tags);
            }
        }
    }
}
