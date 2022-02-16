/**
 * Analyse music with Bliss
 *
 * Copyright (c) 2022 Craig Drummond <craig.p.drummond@gmail.com>
 * GPLv3 license.
 *
 **/

use anyhow::{Result};
use bliss_audio::{library::analyze_paths_streaming};
use indicatif::{ProgressBar, ProgressStyle};
use std::convert::TryInto;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use crate::db;
use crate::tags;

const DONT_ANALYSE:&str = ".nomusic";
const MAX_TAG_ERRORS_TO_SHOW:usize = 25;

fn get_file_list(db:&mut db::Db, mpath:&PathBuf, path:&PathBuf, track_paths:&mut Vec<String>) {
    if path.is_dir() {
        match path.read_dir() {
            Ok(items) => {
                for item in items {
                    match item {
                        Ok(entry) => {
                            let pb = entry.path().to_path_buf();
                            if entry.path().is_dir() {
                                let mut check = pb.clone();
                                check.push(PathBuf::from(DONT_ANALYSE));
                                if check.exists() {
                                    log::error!("Skiping {}", pb.to_string_lossy());
                                } else {
                                    get_file_list(db, mpath, &entry.path(), track_paths);
                                }
                            } else if entry.path().is_file() {
                                let e = pb.extension();
                                if e.is_some() {
                                    let ext = e.unwrap().to_string_lossy();
                                    if ext=="m4a" || ext=="mp3" || ext=="ogg" || ext=="flac" || ext=="opus" {
                                        match pb.strip_prefix(mpath) {
                                            Ok(stripped) => {
                                                let mut cue = pb.clone();
                                                cue.set_extension("cue");
                                                if cue.exists() {
                                                    log::error!("Found CUE album '{}' - not currently handled!", pb.to_string_lossy());
                                                } else {
                                                    let spb = stripped.to_path_buf();
                                                    let sname = String::from(spb.to_string_lossy());
                                                    match db.get_rowid(&sname) {
                                                        Ok(id) => {
                                                            if id<=0 {
                                                                track_paths.push(String::from(pb.to_string_lossy()));
                                                            }
                                                        },
                                                        Err(_) => { }
                                                    }
                                                }
                                            },
                                            Err(_) => { }
                                        }
                                    }
                                }
                            }
                        },
                        Err(_) => { }
                    }
                }
            },
            Err(_) => { }
        }
    }
}

pub fn analyse_new_files(db:&db::Db, mpath: &PathBuf, track_paths:Vec<String>) -> Result<()> {
    let total = track_paths.len();
    let pb = ProgressBar::new(total.try_into().unwrap());
    let style = ProgressStyle::default_bar()
        .template("[{elapsed_precise}] [{bar:25}] {pos:>6}/{len:6} {percent:>3}% {wide_msg}")
        .progress_chars("=> ");
    pb.set_style(style);

    let results = analyze_paths_streaming(track_paths)?;
    let mut analysed = 0;
    let mut failed = 0;
    let mut tag_error:Vec<String> = Vec::new();

    log::info!("Analysing new tracks");
    for (path, result) in results {
        let pbuff = PathBuf::from(&path);
        let stripped = pbuff.strip_prefix(mpath).unwrap();
        let spbuff = stripped.to_path_buf();
        let sname = String::from(spbuff.to_string_lossy());
        pb.set_message(format!("{}", sname));
        match result {
            Ok(track) => {
                let cpath = String::from(path);
                let meta = tags::read(&cpath);
                if meta.title.is_empty() && meta.artist.is_empty() && meta.album.is_empty() && meta.genre.is_empty() {
                    tag_error.push(sname.clone());
                }

                db.add_track(&sname, &meta, &track.analysis);
                analysed += 1;
            },
            Err(_) => {
                failed += 1;
            }
        };
        pb.inc(1);
    }
    pb.finish_with_message(format!("{} Analysed. {} Failure(s).", analysed, failed));
    if !tag_error.is_empty() {
        let total = tag_error.len();
        tag_error.truncate(MAX_TAG_ERRORS_TO_SHOW);

        log::error!("Failed to read tags of the folling track(s):");
        for err in tag_error {
            log::error!("  {}", err);
        }
        if total>MAX_TAG_ERRORS_TO_SHOW {
            log::error!("  + {} other(s)", total - MAX_TAG_ERRORS_TO_SHOW);
        }
    }
    Ok(())
}

pub fn analyse_files(db_path: &str, mpath: &PathBuf, dry_run:bool, keep_old:bool) {
    let mut track_paths:Vec<String> = Vec::new();
    let mut db = db::Db::new(&String::from(db_path));
    let cur = PathBuf::from(mpath);

    db.init();
    log::info!("Looking for new tracks");
    get_file_list(&mut db, mpath, &cur, &mut track_paths);
    log::info!("Num new tracks: {}", track_paths.len());
    if !keep_old {
        db.remove_old(mpath, dry_run);
    }
    if !dry_run {
        if track_paths.len()>0 {
            match analyse_new_files(&db, mpath, track_paths) {
                Ok(_) => { },
                Err(_) => { }
            }
        } else {
            log::info!("No new tracks to analyse");
        }
    }

    db.close();
}

pub fn read_tags(db_path: &str, mpath: &PathBuf) {
    let db = db::Db::new(&String::from(db_path));
    db.init();
    db.update_tags(&mpath);
    db.close();
}

pub fn update_ignore(db_path: &str, ignore_path: &PathBuf) {
    let file = File::open(ignore_path).unwrap();
    let reader = BufReader::new(file);
    let db = db::Db::new(&String::from(db_path));
    db.init();

    db.clear_ignore();
    for (_index, line) in reader.lines().enumerate() {
        let line = line.unwrap();
        if !line.is_empty() && !line.starts_with("#") {
            db.set_ignore(&line);
        }
    }
    db.close();
}