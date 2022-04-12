use crate::cue;
use crate::db;
use crate::tags;
/**
 * Analyse music with Bliss
 *
 * Copyright (c) 2022 Craig Drummond <craig.p.drummond@gmail.com>
 * GPLv3 license.
 *
 **/
use anyhow::Result;
use bliss_audio::{analyze_paths, BlissResult, Song};
use hhmmss::Hhmmss;
use if_chain::if_chain;
use indicatif::{ProgressBar, ProgressStyle};
use num_cpus;
use std::convert::TryInto;
use std::fs;
use std::fs::{DirEntry, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::Duration;
use subprocess::{Exec, NullFile};
use tempdir::TempDir;

const DONT_ANALYSE: &str = ".notmusic";
const MAX_ERRORS_TO_SHOW: usize = 100;
const MAX_TAG_ERRORS_TO_SHOW: usize = 50;
const VALID_EXTENSIONS: [&str; 5] = ["m4a", "mp3", "ogg", "flac", "opus"];

fn get_file_list(
    db: &mut db::Db,
    mpath: &Path,
    path: &Path,
    track_paths: &mut Vec<String>,
    cue_tracks: &mut Vec<cue::CueTrack>,
) {
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

fn check_dir_entry(
    db: &mut db::Db,
    mpath: &Path,
    entry: DirEntry,
    track_paths: &mut Vec<String>,
    cue_tracks: &mut Vec<cue::CueTrack>,
) {
    let pb = entry.path();
    if pb.is_dir() {
        let check = pb.join(DONT_ANALYSE);
        if check.exists() {
            log::info!(
                "Skipping '{}', found '{}'",
                pb.to_string_lossy(),
                DONT_ANALYSE
            );
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
                let mut cue_file = pb.clone();
                cue_file.set_extension("cue");
                if cue_file.exists() {
                    // Found a CUE file, try to parse and then check if tracks exists in DB
                    let this_cue_tracks = cue::parse(&pb, &cue_file);
                    for track in this_cue_tracks {
                        if let Ok(tstripped) = track.track_path.strip_prefix(mpath) {
                            let sname = String::from(tstripped.to_string_lossy());

                            if let Ok(id) = db.get_rowid(&sname) {
                                if id<=0 {
                                    cue_tracks.push(track);
                                }
                            }
                        }
                    }
                } else {
                    let sname = String::from(stripped.to_string_lossy());
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

    log::info!("Analysing new tracks");
    for (path, result) in analyze_paths(track_paths) {
        let pbuff = PathBuf::from(&path);
        let stripped = pbuff.strip_prefix(mpath).unwrap();
        let spbuff = stripped.to_path_buf();
        let sname = String::from(spbuff.to_string_lossy());
        progress.set_message(format!("{}", sname));
        match result {
            Ok(track) => {
                let cpath = String::from(path);
                let meta = tags::read(&cpath);
                if meta.is_empty() {
                    tag_error.push(sname.clone());
                }

                db.add_track(&sname, &meta, &track.analysis);
                analysed += 1;
            }
            Err(e) => {
                failed.push(format!("{} - {}", sname, e));
            }
        };

        progress.inc(1);
    }

    progress.finish_with_message(format!(
        "{} Analysed. {} Failure(s).",
        analysed,
        failed.len()
    ));
    if !failed.is_empty() {
        let total = failed.len();
        failed.truncate(MAX_ERRORS_TO_SHOW);

        log::error!("Failed to analyse the following track(s):");
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

        log::error!("Failed to read tags of the following track(s):");
        for err in tag_error {
            log::error!("  {}", err);
        }
        if total > MAX_TAG_ERRORS_TO_SHOW {
            log::error!("  + {} other(s)", total - MAX_TAG_ERRORS_TO_SHOW);
        }
    }
    Ok(())
}

pub fn analyze_cue_tracks(
    tracks: Vec<cue::CueTrack>,
) -> mpsc::IntoIter<(cue::CueTrack, BlissResult<Song>)> {
    let num_cpus = num_cpus::get();
    let last_track_duration = Duration::new(cue::LAST_TRACK_DURATION, 0);

    #[allow(clippy::type_complexity)]
    let (tx, rx): (
        Sender<(cue::CueTrack, BlissResult<Song>)>,
        Receiver<(cue::CueTrack, BlissResult<Song>)>,
    ) = mpsc::channel();
    if tracks.is_empty() {
        return rx.into_iter();
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
            let dir = TempDir::new("bliss");
            if let Err(e) = dir {
                log::error!("Failed to create temp folder. {}", e);
                return;
            }

            let mut idx = 0;
            let dir = dir.unwrap();
            for mut cue_track in owned_chunk {
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
                let cmd = Exec::cmd("ffmpeg")
                    .arg("-i")
                    .arg(&audio_path)
                    .arg("-ss")
                    .arg(&cue_track.start.hhmmss())
                    .arg("-t")
                    .arg(&cue_track.duration.hhmmss())
                    .arg("-c")
                    .arg("copy")
                    .arg(String::from(tmp_file.to_string_lossy()))
                    .stderr(NullFile)
                    .join();

                if let Err(e) = cmd {
                    log::error!("Failed to call ffmpeg. {}", e);
                }

                if !cfg!(windows) {
                    // ffmpeg seeks to break echo on terminal? 'stty echo' restores...
                    let _ = Exec::cmd("stty").arg("echo").join();
                }

                if tmp_file.exists() {
                    log::debug!("Analyzing '{}'", track_path);
                    let song = Song::from_path(&tmp_file);
                    if cue_track.duration >= last_track_duration {
                        // Last track, so read duration from temp file
                        let meta = tags::read(&String::from(tmp_file.to_string_lossy()));
                        cue_track.duration = Duration::new(meta.duration as u64, 0);
                    }

                    tx_thread.send((cue_track, song)).unwrap();
                    let _ = fs::remove_file(tmp_file);
                } else {
                    log::error!("Failed to create temp file");
                }
            }
        });
        handles.push(child);
    }

    rx.into_iter()
}

pub fn analyse_new_cue_tracks(
    db: &db::Db,
    mpath: &PathBuf,
    cue_tracks: Vec<cue::CueTrack>,
) -> Result<()> {
    let total = cue_tracks.len();
    let progress = ProgressBar::new(total.try_into().unwrap()).with_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:25}] {percent:>3}% {pos:>6}/{len:6} {wide_msg}")
            .progress_chars("=> "),
    );

    let mut analysed = 0;
    let mut failed: Vec<String> = Vec::new();

    log::info!("Analysing new cue tracks");
    for (track, result) in analyze_cue_tracks(cue_tracks) {
        let stripped = track.track_path.strip_prefix(mpath).unwrap();
        let spbuff = stripped.to_path_buf();
        let sname = String::from(spbuff.to_string_lossy());
        progress.set_message(format!("{}", sname));
        match result {
            Ok(song) => {
                let meta = db::Metadata {
                    title: track.title,
                    artist: track.artist,
                    album_artist: track.album_artist,
                    album: track.album,
                    genre: track.genre,
                    duration: track.duration.as_secs() as u32,
                };

                db.add_track(&sname, &meta, &song.analysis);
                analysed += 1;
            }
            Err(e) => {
                failed.push(format!("{} - {}", sname, e));
            }
        };
        progress.inc(1);
    }
    progress.finish_with_message(format!(
        "{} Analysed. {} Failure(s).",
        analysed,
        failed.len()
    ));
    if !failed.is_empty() {
        let total = failed.len();
        failed.truncate(MAX_ERRORS_TO_SHOW);

        log::error!("Failed to analyse the following track(s):");
        for err in failed {
            log::error!("  {}", err);
        }
        if total > MAX_ERRORS_TO_SHOW {
            log::error!("  + {} other(s)", total - MAX_ERRORS_TO_SHOW);
        }
    }
    Ok(())
}

pub fn analyse_files(
    db_path: &str,
    mpaths: &Vec<PathBuf>,
    dry_run: bool,
    keep_old: bool,
    max_num_tracks: usize,
) {
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
        let mut cue_tracks: Vec<cue::CueTrack> = Vec::new();

        if mpaths.len() > 1 {
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
            if max_num_tracks > 0 {
                if track_paths.len() > track_count_left {
                    log::info!("Only analysing {} tracks", track_count_left);
                    track_paths.truncate(track_count_left);
                }
                track_count_left -= track_paths.len();
            }
            if max_num_tracks > 0 {
                if track_count_left == 0 {
                    cue_tracks.clear();
                } else {
                    if cue_tracks.len() > track_count_left {
                        log::info!("Only analysing {} cue tracks", track_count_left);
                        cue_tracks.truncate(track_count_left);
                    }
                    track_count_left -= track_paths.len();
                }
            }

            if !track_paths.is_empty() {
                match analyse_new_files(&db, &mpath, track_paths) {
                    Ok(_) => {}
                    Err(e) => {
                        log::error!("Analysis returned error: {}", e);
                    }
                }
            } else {
                log::info!("No new tracks to analyse");
            }

            if !cue_tracks.is_empty() {
                if let Err(e) = analyse_new_cue_tracks(&db, &mpath, cue_tracks) {
                    log::error!("Cue analysis returned error: {}", e);
                }
            }

            if max_num_tracks > 0 && track_count_left <= 0 {
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
    let mut lines = reader.lines();
    while let Some(Ok(line)) = lines.next() {
        if !line.is_empty() && !line.starts_with("#") {
            db.set_ignore(&line);
        }
    }

    db.close();
}
