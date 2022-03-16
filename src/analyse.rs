/**
 * Analyse music with Bliss
 *
 * Copyright (c) 2022 Craig Drummond <craig.p.drummond@gmail.com>
 * GPLv3 license.
 *
 **/

use anyhow::{Result};
use bliss_audio::{library::analyze_paths_streaming, BlissResult, Song};
use hhmmss::Hhmmss;
use indicatif::{ProgressBar, ProgressStyle};
use std::convert::TryInto;
use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::time::Duration;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use subprocess::{Exec, NullFile};
use tempdir::TempDir;
use num_cpus;
use crate::cue;
use crate::db;
use crate::tags;

const DONT_ANALYSE:&str = ".notmusic";
const MAX_ERRORS_TO_SHOW:usize = 100;
const MAX_TAG_ERRORS_TO_SHOW:usize = 50;

fn get_file_list(db:&mut db::Db, mpath:&PathBuf, path:&PathBuf, track_paths:&mut Vec<String>, cue_tracks:&mut Vec<cue::CueTrack>) {
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
                                    log::info!("Skipping '{}', found '{}'", pb.to_string_lossy(), DONT_ANALYSE);
                                } else {
                                    get_file_list(db, mpath, &entry.path(), track_paths, cue_tracks);
                                }
                            } else if entry.path().is_file() {
                                let e = pb.extension();
                                if e.is_some() {
                                    let ext = e.unwrap().to_string_lossy();
                                    if ext=="m4a" || ext=="mp3" || ext=="ogg" || ext=="flac" || ext=="opus" {
                                        match pb.strip_prefix(mpath) {
                                            Ok(stripped) => {
                                                let mut cue_file = pb.clone();
                                                cue_file.set_extension("cue");
                                                if cue_file.exists() {
                                                    // Found a CUE file, try to parse and then check if tracks exists in DB
                                                    let this_cue_tracks = cue::parse(&pb, &cue_file);
                                                    for track in this_cue_tracks {
                                                        match track.track_path.strip_prefix(mpath) {
                                                            Ok(tstripped) => {
                                                                let spb = tstripped.to_path_buf();
                                                                let sname = String::from(spb.to_string_lossy());
                                                                match db.get_rowid(&sname) {
                                                                    Ok(id) => {
                                                                        if id<=0 {
                                                                            cue_tracks.push(track.clone());
                                                                        }
                                                                    },
                                                                    Err(_) => { }
                                                                }
                                                            },
                                                            Err(_) => { }
                                                        }
                                                    }
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
        .template("[{elapsed_precise}] [{bar:25}] {percent:>3}% {pos:>6}/{len:6} {wide_msg}")
        .progress_chars("=> ");
    pb.set_style(style);

    let results = analyze_paths_streaming(track_paths)?;
    let mut analysed = 0;
    let mut failed:Vec<String> = Vec::new();
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
            Err(e) => {
                failed.push(format!("{} - {}", sname, e));
            }
        };
        pb.inc(1);
    }
    pb.finish_with_message(format!("{} Analysed. {} Failure(s).", analysed, failed.len()));
    if !failed.is_empty() {
        let total = failed.len();
        failed.truncate(MAX_ERRORS_TO_SHOW);

        log::error!("Failed to analyse the folling track(s):");
        for err in failed {
            log::error!("  {}", err);
        }
        if total>MAX_ERRORS_TO_SHOW {
            log::error!("  + {} other(s)", total - MAX_ERRORS_TO_SHOW);
        }
    }
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

pub fn analyze_cue_streaming(tracks: Vec<cue::CueTrack>,) -> BlissResult<Receiver<(cue::CueTrack, BlissResult<Song>)>> {
    let num_cpus = num_cpus::get();
    let last_track_duration = Duration::new(cue::LAST_TRACK_DURATION, 0);

    #[allow(clippy::type_complexity)]
    let (tx, rx): (
        Sender<(cue::CueTrack, BlissResult<Song>)>,
        Receiver<(cue::CueTrack, BlissResult<Song>)>,
    ) = mpsc::channel();
    if tracks.is_empty() {
        return Ok(rx);
    }

    let mut handles = Vec::new();
    let mut chunk_length = tracks.len() / num_cpus;
    if chunk_length == 0 {
        chunk_length = tracks.len();
    } else if chunk_length == 1 && tracks.len() > num_cpus {
        chunk_length = 2;
    }

    for chunk in tracks.chunks(chunk_length) {
        let tx_thread = tx.clone();
        let owned_chunk = chunk.to_owned();
        let child = thread::spawn(move || {
            let mut idx = 0;
            match &TempDir::new("bliss") {
                Ok(dir) => {
                    for cue_track in owned_chunk {
                        let audio_path = String::from(cue_track.audio_path.to_string_lossy());
                        let ext = cue_track.audio_path.extension();
                        let track_path = String::from(cue_track.track_path.to_string_lossy());
                        let mut tmp_file = PathBuf::from(dir.path());
                        if ext.is_some() {
                            tmp_file.push(format!("{}.{}", idx, ext.unwrap().to_string_lossy()));
                        } else {
                            tmp_file.push(format!("{}.flac", idx));
                        }
                        idx += 1;

                        log::debug!("Extracting '{}'", track_path);
                        match Exec::cmd("ffmpeg").arg("-i").arg(&audio_path)
                                                    .arg("-ss").arg(&cue_track.start.hhmmss())
                                                    .arg("-t").arg(&cue_track.duration.hhmmss())
                                                    .arg("-c").arg("copy")
                                                    .arg(String::from(tmp_file.to_string_lossy()))
                                                    .stderr(NullFile)
                                                    .join() {
                            Ok(_) => { },
                            Err(e) => { log::error!("Failed to call ffmpeg. {}", e); }
                        }

                        if ! cfg!(windows) {
                            // ffmpeg seeks to break echo on terminal? 'stty echo' restores...
                            match Exec::cmd("stty").arg("echo").join() {
                                Ok(_) => { },
                                Err(_) => { }
                            }
                        }

                        if tmp_file.exists() {
                            log::debug!("Analyzing '{}'", track_path);
                            let song = Song::new(&tmp_file);
                            if cue_track.duration>=last_track_duration {
                                // Last track, so read duration from temp file
                                let mut cloned = cue_track.clone();
                                let meta = tags::read(&String::from(tmp_file.to_string_lossy()));
                                cloned.duration = Duration::new(meta.duration as u64, 0);
                                tx_thread.send((cloned, song)).unwrap();
                            } else {
                                tx_thread.send((cue_track, song)).unwrap();
                            }
                            match fs::remove_file(tmp_file) {
                                Ok(_) => { },
                                Err(_) => { }
                            }
                        } else {
                            log::error!("Failed to create temp file");
                        }
                    }
                },
                Err(e) => { log::error!("Failed to create temp folder. {}", e); }
            }
        });
        handles.push(child);
    }

    Ok(rx)
}

pub fn analyse_new_cue_tracks(db:&db::Db, mpath: &PathBuf, cue_tracks:Vec<cue::CueTrack>) -> Result<()> {
    let total = cue_tracks.len();
    let pb = ProgressBar::new(total.try_into().unwrap());
    let style = ProgressStyle::default_bar()
        .template("[{elapsed_precise}] [{bar:25}] {percent:>3}% {pos:>6}/{len:6} {wide_msg}")
        .progress_chars("=> ");
    pb.set_style(style);

    let results = analyze_cue_streaming(cue_tracks)?;
    let mut analysed = 0;
    let mut failed:Vec<String> = Vec::new();

    log::info!("Analysing new cue tracks");
    for (track, result) in results {
        let stripped = track.track_path.strip_prefix(mpath).unwrap();
        let spbuff = stripped.to_path_buf();
        let sname = String::from(spbuff.to_string_lossy());
        pb.set_message(format!("{}", sname));
        match result {
            Ok(song) => {
                let meta = db::Metadata {
                    title:track.title,
                    artist:track.artist,
                    album_artist:track.album_artist,
                    album:track.album,
                    genre:track.genre,
                    duration:track.duration.as_secs() as u32
                };

                db.add_track(&sname, &meta, &song.analysis);
                analysed += 1;
            },
            Err(e) => {
                failed.push(format!("{} - {}", sname, e));
            }
        };
        pb.inc(1);
    }
    pb.finish_with_message(format!("{} Analysed. {} Failure(s).", analysed, failed.len()));
    if !failed.is_empty() {
        let total = failed.len();
        failed.truncate(MAX_ERRORS_TO_SHOW);

        log::error!("Failed to analyse the folling track(s):");
        for err in failed {
            log::error!("  {}", err);
        }
        if total>MAX_ERRORS_TO_SHOW {
            log::error!("  + {} other(s)", total - MAX_ERRORS_TO_SHOW);
        }
    }
    Ok(())
}

pub fn analyse_files(db_path: &str, mpaths: &Vec<PathBuf>, dry_run:bool, keep_old:bool, max_num_tracks:usize) {
    let mut db = db::Db::new(&String::from(db_path));
    let mut track_count_left = max_num_tracks;

    db.init();

    if !keep_old {
        db.remove_old(mpaths, dry_run);
    }

    for path in mpaths {
        let mpath = path.clone();
        let cur = path.clone();
        let mut track_paths:Vec<String> = Vec::new();
        let mut cue_tracks:Vec<cue::CueTrack> = Vec::new();

        if mpaths.len()>1 {
            log::info!("Looking for new tracks in {}", mpath.to_string_lossy());
        } else {
            log::info!("Looking for new tracks");
        }
        get_file_list(&mut db, &mpath, &cur, &mut track_paths, &mut cue_tracks);
        track_paths.sort();
        log::info!("Num new tracks: {}", track_paths.len());
        if !cue_tracks.is_empty() {
            log::info!("Num new cue tracks: {}", cue_tracks.len());
        }
        if dry_run {
            if !track_paths.is_empty() || !cue_tracks.is_empty() {
                log::info!("The following need to be analysed:");
                for track in track_paths {
                    log::info!("  {}", track);
                }
                for track in cue_tracks {
                    log::info!("  {}", track.track_path.to_string_lossy());
                }
            }
        } else {
            if max_num_tracks>0 {
                if track_paths.len()>track_count_left {
                    log::info!("Only analysing {} tracks", track_count_left);
                    track_paths.truncate(track_count_left);
                }
                track_count_left -= track_paths.len();
            }
            if max_num_tracks>0 {
                if track_count_left == 0 {
                    cue_tracks.clear();
                } else {
                    if cue_tracks.len()>track_count_left {
                        log::info!("Only analysing {} cue tracks", track_count_left);
                        cue_tracks.truncate(track_count_left);
                    }
                    track_count_left -= track_paths.len();
                }
            }

            if !track_paths.is_empty() {
                match analyse_new_files(&db, &mpath, track_paths) {
                    Ok(_) => { },
                    Err(e) => { log::error!("Analysis returned error: {}", e); }
                }
            } else {
                log::info!("No new tracks to analyse");
            }

            if !cue_tracks.is_empty() {
                match analyse_new_cue_tracks(&db, &mpath, cue_tracks) {
                    Ok(_) => { },
                    Err(e) => { log::error!("Cue analysis returned error: {}", e); }
                }
            }

            if max_num_tracks>0 && track_count_left<=0 {
                log::info!("Track limit reached");
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
    for (_index, line) in reader.lines().enumerate() {
        let line = line.unwrap();
        if !line.is_empty() && !line.starts_with("#") {
            db.set_ignore(&line);
        }
    }
    db.close();
}
