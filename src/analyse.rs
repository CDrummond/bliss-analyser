/**
 * Analyse music with Bliss
 *
 * Copyright (c) 2022-2023 Craig Drummond <craig.p.drummond@gmail.com>
 * GPLv3 license.
 *
 **/

use crate::cue;
use crate::db;
use crate::ffmpeg;
use crate::tags;
use anyhow::Result;
use bliss_audio::{decoder::Decoder, BlissResult, Song};
use hhmmss::Hhmmss;
use if_chain::if_chain;
use indicatif::{ProgressBar, ProgressStyle};
use std::convert::TryInto;
use std::fs::{DirEntry, File};
use std::io::{BufRead, BufReader};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::Duration;
use num_cpus;

const DONT_ANALYSE: &str = ".notmusic";
const MAX_ERRORS_TO_SHOW: usize = 100;
const MAX_TAG_ERRORS_TO_SHOW: usize = 50;
const VALID_EXTENSIONS: [&str; 6] = ["m4a", "mp3", "ogg", "flac", "opus", "wv"];

fn get_file_list(db: &mut db::Db, mpath: &Path, path: &Path, track_paths: &mut Vec<String>, cue_tracks:&mut Vec<cue::CueTrack>) {
    if !path.is_dir() {
        return;
    }

    if let Ok(items) = path.read_dir() {
        for item in items {
            if let Ok(entry) = item {
                check_dir_entry(db, mpath, entry, track_paths, cue_tracks);
            }
        }
    }
}

fn check_dir_entry(db: &mut db::Db, mpath: &Path, entry: DirEntry, track_paths: &mut Vec<String>, cue_tracks:&mut Vec<cue::CueTrack>) {
    let pb = entry.path();
    if pb.is_dir() {
        let check = pb.join(DONT_ANALYSE);
        if check.exists() {
            log::info!("Skipping '{}', found '{}'", pb.to_string_lossy(), DONT_ANALYSE);
        } else {
            get_file_list(db, mpath, &pb, track_paths, cue_tracks);
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
                                let this_cue_tracks = cue::parse(&pb, &cue_file);
                                for track in this_cue_tracks {
                                    cue_tracks.push(track.clone());
                                }
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

pub fn analyse_new_files(db: &db::Db, mpath: &PathBuf, track_paths: Vec<String>, max_threads: usize) -> Result<()> {
    let total = track_paths.len();
    let progress = ProgressBar::new(total.try_into().unwrap()).with_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:25}] {percent:>3}% {pos:>6}/{len:6} {wide_msg}")
            .progress_chars("=> "),
    );
    let cpu_threads: NonZeroUsize = match max_threads {
        0 => NonZeroUsize::new(num_cpus::get()).unwrap(),
        _ => NonZeroUsize::new(max_threads).unwrap(),
    };

    let mut analysed = 0;
    let mut failed: Vec<String> = Vec::new();
    let mut tag_error: Vec<String> = Vec::new();

    log::info!("Analysing new files");
    for (path, result) in <ffmpeg::FFmpegCmdDecoder as Decoder>::analyze_paths_with_cores(track_paths, cpu_threads) {
        let stripped = path.strip_prefix(mpath).unwrap();
        let spbuff = stripped.to_path_buf();
        let sname = String::from(spbuff.to_string_lossy());
        progress.set_message(format!("{}", sname));
        match result {
            Ok(track) => {
                let cpath = String::from(path.to_string_lossy());
                let meta = tags::read(&cpath);
                if meta.is_empty() {
                    tag_error.push(sname.clone());
                }
                db.add_track(&sname, &meta, &track.analysis);
                analysed += 1;
            }
            Err(e) => { failed.push(format!("{} - {}", sname, e)); }
        };

        progress.inc(1);
    }

    // Reset terminal, otherwise typed output does not show? Perhaps Linux only...
    if ! cfg!(windows) {
        match std::process::Command::new("stty").arg("sane").spawn() {
            Ok(_) => { },
            Err(_)    => { },
        };
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

pub fn analyze_cue_streaming(tracks: Vec<cue::CueTrack>,) -> BlissResult<Receiver<(cue::CueTrack, BlissResult<Song>)>> {
    let num_cpus = num_cpus::get();

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
            for cue_track in owned_chunk {
                let audio_path = format!("{}{}{}.00{}{}.00", cue_track.audio_path.to_string_lossy(), ffmpeg::TIME_SEP, cue_track.start.hhmmss(), ffmpeg::TIME_SEP, cue_track.duration.hhmmss());
                let track_path = String::from(cue_track.track_path.to_string_lossy());

                log::debug!("Analyzing '{}'", track_path);
                let song = <ffmpeg::FFmpegCmdDecoder as Decoder>::song_from_path(audio_path);
                tx_thread.send((cue_track, song)).unwrap();
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
    let last_track_duration = Duration::new(cue::LAST_TRACK_DURATION, 0);

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
                    duration:if track.duration>=last_track_duration { song.duration.as_secs() as u32 } else { track.duration.as_secs() as u32 }
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

pub fn analyse_files(db_path: &str, mpaths: &Vec<PathBuf>, dry_run: bool, keep_old: bool, max_num_tracks: usize, max_threads: usize) {
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
        let mut cue_tracks:Vec<cue::CueTrack> = Vec::new();

        if mpaths.len() > 1 {
            log::info!("Looking for new files in {}", mpath.to_string_lossy());
        } else {
            log::info!("Looking for new files");
        }
        get_file_list(&mut db, &mpath, &cur, &mut track_paths, &mut cue_tracks);
        track_paths.sort();
        log::info!("Num new files: {}", track_paths.len());
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
            if max_num_tracks > 0 {
                if track_paths.len() > track_count_left {
                    log::info!("Only analysing {} files", track_count_left);
                    track_paths.truncate(track_count_left);
                }
                track_count_left -= track_paths.len();
            }
            if max_num_tracks>0 {
                if track_count_left == 0 {
                    cue_tracks.clear();
                } /*else {
                    if cue_tracks.len()>track_count_left {
                        log::info!("Only analysing {} cue tracks", track_count_left);
                        cue_tracks.truncate(track_count_left);
                    }
                    track_count_left -= track_paths.len();
                }*/
            }

            if !track_paths.is_empty() {
                match analyse_new_files(&db, &mpath, track_paths, max_threads) {
                    Ok(_) => { }
                    Err(e) => { log::error!("Analysis returned error: {}", e); }
                }
            } else {
                log::info!("No new files to analyse");
            }

            if !cue_tracks.is_empty() {
                match analyse_new_cue_tracks(&db, &mpath, cue_tracks) {
                    Ok(_) => { },
                    Err(e) => { log::error!("Cue analysis returned error: {}", e); }
                }
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
