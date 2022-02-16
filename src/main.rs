/**
 * Analyse music with Bliss
 *
 * Copyright (c) 2022 Craig Drummond <craig.p.drummond@gmail.com>
 * GPLv3 license.
 *
 **/
use argparse::{ArgumentParser, Store};
use std::path::PathBuf;
use std::process;
mod analyse;
mod db;
mod tags;

fn main() {
    let mut db_path = "bliss.db".to_string();
    let mut logging = "warn".to_string();
    let mut music_path = ".".to_string();
    let mut keep_old:bool = false;
    let mut dry_run:bool = false;
    let mut tags_only:bool = false;

    {
        // arg_parse.refer 'borrows' db_path, etc, and can only have one
        // borrow per scope, hence this section is enclosed in { }
        let mut arg_parse = ArgumentParser::new();
        arg_parse.set_description("Bliss Mixer");
        arg_parse.refer(&mut music_path).add_option(&["-m", "--music"], Store, "Music folder");
        arg_parse.refer(&mut db_path).add_option(&["-d", "--db"], Store, "Database location");
        arg_parse.refer(&mut logging).add_option(&["-l", "--logging"], Store, "Log level (trace, debug, info, warn, error)");
        arg_parse.refer(&mut keep_old).add_option(&["-k", "--keep-old"], Store, "Don't remove tracks from DB if they don't exist");
        arg_parse.refer(&mut dry_run).add_option(&["-r", "--dry-run"], Store, "Dry run, only show what needs to be done");
        arg_parse.refer(&mut tags_only).add_option(&["-t", "--tags-only"], Store, "Re-read tags");
        /*
        TODO:
        -i --ignore Update ignore column
        -t --tags Re-read tags
        */
        arg_parse.parse_args_or_exit();
    }

    if logging.eq_ignore_ascii_case("trace") || logging.eq_ignore_ascii_case("debug") || logging.eq_ignore_ascii_case("info") || logging.eq_ignore_ascii_case("warn") || logging.eq_ignore_ascii_case("error") {
        env_logger::init_from_env(env_logger::Env::default().filter_or("XXXXXXXX", logging));
    } else {
        env_logger::init_from_env(env_logger::Env::default().filter_or("XXXXXXXX", "ERROR"));
        log::error!("Invalid log level ({}) supplied", logging);
        process::exit(-1);
    }

    if db_path.len() < 3 {
        log::error!("Invalid DB path ({}) supplied", db_path);
        process::exit(-1);
    }

    let path = PathBuf::from(&db_path);
    if path.exists() && !path.is_file() {
        log::error!("DB path ({}) is not a file", db_path);
        process::exit(-1);
    }

    let mpath = PathBuf::from(&music_path);
    if !mpath.exists() {
        log::error!("Music path ({}) does not exist", music_path);
        process::exit(-1);
    }
    if !mpath.is_dir() {
        log::error!("Music path ({}) is not a directory", music_path);
        process::exit(-1);
    }

    if tags_only {
        analyse::read_tags(&db_path, &mpath);
    }
    analyse::analyse_files(&db_path, &mpath, dry_run, keep_old);
}
