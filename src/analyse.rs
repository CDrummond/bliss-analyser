/**
 * Analyse music with Bliss
 *
 * Copyright (c) 2022 Craig Drummond <craig.p.drummond@gmail.com>
 * GPLv3 license.
 *
 **/

use crate::db;
use crate::tags;
use anyhow::Result;
use bliss_audio::{analyze_paths};
use if_chain::if_chain;
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashSet;
use std::convert::TryInto;
use std::fs::{DirEntry, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

const DONT_ANALYSE: &str = ".notmusic";
const MAX_ERRORS_TO_SHOW: usize = 100;
const MAX_TAG_ERRORS_TO_SHOW: usize = 50;
const VALID_EXTENSIONS: [&str; 5] = ["m4a", "mp3", "ogg", "flac", "opus"];

fn get_file_list(db: &mut db::Db, mpath: &Path, path: &Path, track_paths: &mut Vec<String>) {
    if !path.is_dir() {
        return;
    }

    if let Ok(items) = path.read_dir() {
        for item in items {
            if let Ok(entry) = item {
                check_dir_entry(db, mpath, entry, track_paths);
            }
        }
    }
}

fn check_dir_entry(db: &mut db::Db, mpath: &Path, entry: DirEntry, track_paths: &mut Vec<String>) {
    let pb = entry.path();
    if pb.is_dir() {
        let check = pb.join(DONT_ANALYSE);
        if check.exists() {
            log::info!("Skipping '{}', found '{}'", pb.to_string_lossy(), DONT_ANALYSE);
        } else {
            get_file_list(db, mpath, &pb, track_paths);
        }
    } else if pb.is_file() {
        if_chain! {
            if let Some(ext) = pb.extension();
            let ext = ext.to_string_lossy();
            if VALID_EXTENSIONS.contains(&&*ext);
            if let Ok(stripped) = pb.strip_prefix(mpath);
            then {
                let sname = String::from(stripped.to_string_lossy());
                let mut cue_file = pb.clone();
                cue_file.set_extension("cue");
                if cue_file.exists() {
                    // For cue files, check if first track is in DB
                    let mut cue_track_path = pb.clone();
                    let ext = pb.extension().unwrap().to_string_lossy();
                    cue_track_path.set_extension(format!("{}{}1", ext, db::CUE_MARKER));
                    if let Ok(cue_track_stripped) = cue_track_path.strip_prefix(mpath) {
                        let cue_track_sname = String::from(cue_track_stripped.to_string_lossy());
                        if let Ok(id) = db.get_rowid(&cue_track_sname) {
                            if id<=0 {
                                track_paths.push(String::from(cue_file.to_string_lossy()));
                            }
                        }
                    }
                } else {
                    if let Ok(id) = db.get_rowid(&sname) {
                        if id<=0 {
                            track_paths.push(String::from(pb.to_string_lossy()));
                        }
                    }
                }
            }
        }
    }
}

pub fn analyse_new_files(db: &db::Db, mpath: &PathBuf, track_paths: Vec<String>) -> Result<()> {
    let total = track_paths.len();
    let progress = ProgressBar::new(total.try_into().unwrap()).with_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:25}] {percent:>3}% {pos:>6}/{len:6} {wide_msg}")
            .progress_chars("=> "),
    );

    let mut analysed = 0;
    let mut failed: Vec<String> = Vec::new();
    let mut tag_error: Vec<String> = Vec::new();
    let mut reported_cue:HashSet<String> = HashSet::new();

    log::info!("Analysing new files");
    for (path, result) in analyze_paths(track_paths) {
        let pbuff = PathBuf::from(&path);
        let stripped = pbuff.strip_prefix(mpath).unwrap();
        let spbuff = stripped.to_path_buf();
        let sname = String::from(spbuff.to_string_lossy());
        progress.set_message(format!("{}", sname));
        let mut inc_progress = true; // Only want to increment progress once for cue tracks
        match result {
            Ok(track) => {
                let cpath = String::from(path);
                match track.cue_info {
                    Some(cue) => {
                        match track.track_number {
                            Some(track_num) => {
                                let t_num = track_num.parse::<i32>().unwrap();
                                if reported_cue.contains(&cpath) {
                                    inc_progress = false;
                                } else {
                                    analysed += 1;
                                    reported_cue.insert(cpath);
                                }
                                let meta = db::Metadata {
                                    title: track.title.unwrap_or_default().to_string(),
                                    artist: track.artist.unwrap_or_default().to_string(),
                                    album: track.album.unwrap_or_default().to_string(),
                                    album_artist: track.album_artist.unwrap_or_default().to_string(),
                                    genre: track.genre.unwrap_or_default().to_string(),
                                    duration: track.duration.as_secs() as u32
                                };

                                // Remove prefix from audio_file_path
                                let pbuff = PathBuf::from(&cue.audio_file_path);
                                let stripped = pbuff.strip_prefix(mpath).unwrap();
                                let spbuff = stripped.to_path_buf();
                                let sname = String::from(spbuff.to_string_lossy());

                                let db_path = format!("{}{}{}", sname, db::CUE_MARKER, t_num);
                                db.add_track(&db_path, &meta, &track.analysis);
                            }
                            None => { failed.push(format!("{} - No track number?", sname)); }
                        }
                    }
                    None => {
                        // Use lofty to read tags here, and not bliss's, so that if update
                        // tags is ever used they are from the same source.
                        let mut meta = tags::read(&cpath);
                        if meta.is_empty() {
                            // Lofty failed? Try from bliss...
                            meta.title = track.title.unwrap_or_default().to_string();
                            meta.artist = track.artist.unwrap_or_default().to_string();
                            meta.album = track.album.unwrap_or_default().to_string();
                            meta.album_artist = track.album_artist.unwrap_or_default().to_string();
                            meta.genre = track.genre.unwrap_or_default().to_string();
                            meta.duration = track.duration.as_secs() as u32;
                        }
                        if meta.is_empty() {
                            tag_error.push(sname.clone());
                        }
                        db.add_track(&sname, &meta, &track.analysis);
                        analysed += 1;
                    }
                }
            }
            Err(e) => { failed.push(format!("{} - {}", sname, e)); }
        };

        if inc_progress {
            progress.inc(1);
        }
    }

    progress.finish_with_message("Finished!");
    log::info!("{} Analysed. {} Failure(s).", analysed, failed.len());
    if !failed.is_empty() {
        let total = failed.len();
        failed.truncate(MAX_ERRORS_TO_SHOW);

        log::error!("Failed to analyse the following file(s):");
        for err in failed {
            log::error!("  {}", err);
        }
        if total > MAX_ERRORS_TO_SHOW {
            log::error!("  + {} other(s)", total - MAX_ERRORS_TO_SHOW);
        }
    }
    if !tag_error.is_empty() {
        let total = tag_error.len();
        tag_error.truncate(MAX_TAG_ERRORS_TO_SHOW);

        log::error!("Failed to read tags of the following file(s):");
        for err in tag_error {
            log::error!("  {}", err);
        }
        if total > MAX_TAG_ERRORS_TO_SHOW {
            log::error!("  + {} other(s)", total - MAX_TAG_ERRORS_TO_SHOW);
        }
    }
    Ok(())
}

pub fn analyse_files(db_path: &str, mpaths: &Vec<PathBuf>, dry_run: bool, keep_old: bool, max_num_tracks: usize) {
    let mut db = db::Db::new(&String::from(db_path));
    let mut track_count_left = max_num_tracks;

    db.init();

    if !keep_old {
        db.remove_old(mpaths, dry_run);
    }

    for path in mpaths {
        let mpath = path.clone();
        let cur = path.clone();
        let mut track_paths: Vec<String> = Vec::new();

        if mpaths.len() > 1 {
            log::info!("Looking for new files in {}", mpath.to_string_lossy());
        } else {
            log::info!("Looking for new files");
        }
        get_file_list(&mut db, &mpath, &cur, &mut track_paths);
        track_paths.sort();
        log::info!("Num new files: {}", track_paths.len());

        if dry_run {
            if !track_paths.is_empty() {
                log::info!("The following need to be analysed:");
                for track in track_paths {
                    log::info!("  {}", track);
                }
            }
        } else {
            if max_num_tracks > 0 {
                if track_paths.len() > track_count_left {
                    log::info!("Only analysing {} files", track_count_left);
                    track_paths.truncate(track_count_left);
                }
                track_count_left -= track_paths.len();
            }

            if !track_paths.is_empty() {
                match analyse_new_files(&db, &mpath, track_paths) {
                    Ok(_) => { }
                    Err(e) => { log::error!("Analysis returned error: {}", e); }
                }
            } else {
                log::info!("No new files to analyse");
            }

            if max_num_tracks > 0 && track_count_left <= 0 {
                log::info!("File limit reached");
                break;
            }
        }
    }

    db.close();
}

pub fn read_tags(db_path: &str, mpaths: &Vec<PathBuf>) {
    let db = db::Db::new(&String::from(db_path));
    db.init();
    db.update_tags(&mpaths);
    db.close();
}

pub fn update_ignore(db_path: &str, ignore_path: &PathBuf) {
    let file = File::open(ignore_path).unwrap();
    let reader = BufReader::new(file);
    let db = db::Db::new(&String::from(db_path));
    db.init();

    db.clear_ignore();
    let mut lines = reader.lines();
    while let Some(Ok(line)) = lines.next() {
        if !line.is_empty() && !line.starts_with("#") {
            db.set_ignore(&line);
        }
    }

    db.close();
}
