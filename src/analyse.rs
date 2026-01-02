/**
 * Analyse music with Bliss
 *
 * Copyright (c) 2022-2026 Craig Drummond <craig.p.drummond@gmail.com>
 * GPLv3 license.
 *
 **/

use crate::db;
use crate::tags;
use anyhow::Result;
use if_chain::if_chain;
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashSet;
use std::convert::TryInto;
use filetime::FileTime;
use std::fs;
use std::fs::DirEntry;
use std::num::NonZero;
use std::path::{Path, PathBuf};
#[cfg(feature = "libav")]
use bliss_audio::decoder::ffmpeg::FFmpegDecoder as SongDecoder;
#[cfg(feature = "symphonia")]
use bliss_audio::decoder::symphonia::SymphoniaDecoder as SongDecoder;
use bliss_audio::{AnalysisOptions, decoder::Decoder};
use ureq;
use std::time::{SystemTime, UNIX_EPOCH};
use std::thread;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use serde_json::json;

const DONT_ANALYSE: &str = ".notmusic";
const MAX_ERRORS_TO_SHOW: usize = 100;
const MAX_TAG_ERRORS_TO_SHOW: usize = 50;
const MIN_NOTIF_TIME:u64 = 2;
const VALID_EXTENSIONS: [&str; 7] = ["m4a", "mp3", "ogg", "flac", "opus", "wv", "dsf"];

static mut TERMINATE_ANALYSIS_FLAG: bool = false;

struct NotifInfo {
    pub enabled: bool,
    pub address: String,
    pub last_send: u64,
    pub start_time: u64
}

fn terminate_analysis() -> bool {
    unsafe {
        return TERMINATE_ANALYSIS_FLAG
    }
}

fn handle_ctrl_c() {
    unsafe {
        TERMINATE_ANALYSIS_FLAG = true;
    }
}

fn send_notif_msg(notifs: &mut NotifInfo, text: &str) {
    let js = json!({"id":"1", "method":"slim.request", "params":["", ["blissmixer", "analyser", "act:update", format!("msg:{}", text)]]});
    log::info!("Sending notif to LMS: {}", text);
    let _ = ureq::post(&notifs.address).send_string(&js.to_string());
}

fn send_notif(notifs: &mut NotifInfo, text: &str) {
    if notifs.enabled {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).expect("time should go forward").as_secs();
        if now>=notifs.last_send+MIN_NOTIF_TIME {
            let dur = now - notifs.start_time;
            let msg = format!("[{:02}:{:02}:{:02}] {}", (dur/60)/60, (dur/60)%60, dur%60, text);
            send_notif_msg(notifs, &msg);
            notifs.last_send = now;
        }
    }
}

fn get_file_list(db: &mut db::Db, mpath: &Path, path: &Path, track_paths: &mut Vec<String>, file_count:&mut usize, max_num_files: usize,
                 dry_run: bool, notifs: &mut NotifInfo) {
    if !path.is_dir() {
        return;
    }

    send_notif(notifs, &format!("SCAN DIR {}", path.to_string_lossy()));
    let mut items: Vec<_> = path.read_dir().unwrap().map(|r| r.unwrap()).collect();
    items.sort_by_key(|dir| dir.path());

    for item in items {
        check_dir_entry(db, mpath, item, track_paths, file_count, max_num_files, dry_run, notifs);
        if max_num_files>0 && *file_count>=max_num_files {
            break;
        }
    }
}

fn check_dir_entry(db: &mut db::Db, mpath: &Path, entry: DirEntry, track_paths: &mut Vec<String>, file_count:&mut usize, max_num_files: usize,
                   dry_run: bool, notifs: &mut NotifInfo) {
    let pb = entry.path();
    if pb.is_dir() {
        let check = pb.join(DONT_ANALYSE);
        if check.exists() {
            log::info!("Skipping '{}', found '{}'", pb.to_string_lossy(), DONT_ANALYSE);
        } else if max_num_files<=0 || *file_count<max_num_files {
            get_file_list(db, mpath, &pb, track_paths, file_count, max_num_files, dry_run, notifs);
        }
    } else if pb.is_file() && (max_num_files<=0 || *file_count<max_num_files) {
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
                                *file_count+=1;
                            }
                        }
                    }
                } else {
                    if let Ok(id) = db.get_rowid(&sname) {
                        if id<=0 {
                            // Check this file is not in Failures table
                            let ts = db.get_failure_timestamp(&sname);
                            if ts<=0 {
                                // ...nope, not in there so analyse
                                track_paths.push(String::from(pb.to_string_lossy()));
                                *file_count+=1;
                            } else {
                                let path = String::from(pb.to_string_lossy());
                                let metadata = fs::metadata(&path).unwrap();
                                let mtime = FileTime::from_last_modification_time(&metadata).unix_seconds();
                                if mtime!=ts {
                                    // ...was in failures table, but timestamp has changed
                                    db.remove_from_failures(&sname);
                                    track_paths.push(path);
                                    *file_count+=1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn show_errors(failed: &mut Vec<String>, tag_error: &mut Vec<String>) {
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
}

#[derive(Clone, Default, PartialEq)]
pub struct Meta {
    pub file: String,
    pub meta: db::Metadata
}

fn read_tags(tracks: Vec<String>, max_threads: usize) -> Receiver<Meta> {
    #[allow(clippy::type_complexity)]
    let (tx, rx): (
        Sender<Meta>,
        Receiver<Meta>,
    ) = mpsc::channel();
    if tracks.is_empty() {
        return rx;
    }

    let mut handles = Vec::new();
    let mut chunk_length = tracks.len() / max_threads;
    if chunk_length == 0 {
        chunk_length = tracks.len();
    } else if chunk_length == 1 && tracks.len() > max_threads {
        chunk_length = 2;
    }

    for chunk in tracks.chunks(chunk_length) {
        let tx_thread = tx.clone();
        let owned_chunk = chunk.to_owned();
        let child = thread::spawn(move || {
            for track in owned_chunk {
                let _ = tx_thread.send(Meta {
                    file:track.clone(),
                    meta: tags::read(&track, true),
                });
            }
        });
        handles.push(child);
    }

    rx
}

fn check_for_tags(db: &db::Db, mpath: &PathBuf, track_paths: Vec<String>, max_threads: usize, notifs: &mut NotifInfo) -> Vec<String> {
    let mut untagged_paths:Vec<String> = Vec::new();
    let total = track_paths.len();
    let results = read_tags(track_paths, max_threads);
    let progress = ProgressBar::new(total.try_into().unwrap()).with_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:25}] {percent:>3}% {pos:>6}/{len:6} {wide_msg}")
            .progress_chars("=> "),
    );

    log::info!("Reading any existing analysis tags");
    send_notif(notifs, "Reading any existing analysis tags");
    for res in results {
        let path = PathBuf::from(&res.file);
        let stripped = path.strip_prefix(mpath).unwrap();
        let spbuff = stripped.to_path_buf();
        let sname = String::from(spbuff.to_string_lossy());
        progress.set_message(format!("{}", sname));
        if !res.meta.is_empty() && !res.meta.analysis.is_none() {
            db.add_track(&sname, &res.meta.clone(), &res.meta.analysis.unwrap());
        } else {
            untagged_paths.push(res.file);
        }
        if terminate_analysis() {
            break
        }
        progress.inc(1);
        if notifs.enabled {
            let pc = (progress.position() as f64 * 100.0)/total as f64;
            send_notif(notifs, &format!("READ TAGS {:8.2}% {}", pc, sname));
        }
    }
    if terminate_analysis() {
        progress.abandon_with_message("Terminated!");
    } else {
        progress.finish_with_message("Finished!");
    }
    untagged_paths
}

fn analyse_new_files(db: &db::Db, mpath: &PathBuf, track_paths: Vec<String>, max_threads: usize, write_tags: bool,
                     preserve_mod_times: bool, notifs: &mut NotifInfo) -> Result<()> {
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
    let mut options:AnalysisOptions = AnalysisOptions::default();
    options.number_cores = NonZero::new(max_threads).unwrap();

    send_notif(notifs, "Analysing new files");
    log::info!("Analysing new files");

    for (path, result) in SongDecoder::analyze_paths_with_options(track_paths, options) {
        let stripped = path.strip_prefix(mpath).unwrap();
        let spbuff = stripped.to_path_buf();
        let sname = String::from(spbuff.to_string_lossy());
        progress.set_message(format!("{}", sname));
        let mut inc_progress = true; // Only want to increment progress once for cue tracks
        match result {
            Ok(track) => {
                let cpath = String::from(path.to_string_lossy());
                match track.cue_info {
                    Some(cue) => {
                        match track.track_number {
                            Some(track_num) => {
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
                                    duration: track.duration.as_secs() as u32,
                                    analysis: None
                                };

                                // Remove prefix from audio_file_path
                                let pbuff = PathBuf::from(&cue.audio_file_path);
                                let stripped = pbuff.strip_prefix(mpath).unwrap();
                                let spbuff = stripped.to_path_buf();
                                let sname = String::from(spbuff.to_string_lossy());

                                let db_path = format!("{}{}{}", sname, db::CUE_MARKER, track_num);
                                db.add_track(&db_path, &meta, &track.analysis);
                            }
                            None => { failed.push(format!("{} - No track number?", sname)); }
                        }
                    }
                    None => {
                        // Use lofty to read tags here, and not bliss's, so that if update
                        // tags is ever used they are from the same source.
                        let mut meta = tags::read(&cpath, false);
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
                        if write_tags {
                            tags::write_analysis(&cpath, &track.analysis, preserve_mod_times);
                        }
                        db.add_track(&sname, &meta, &track.analysis);
                    }
                }
                analysed += 1;
            }
            Err(e) => {
                failed.push(format!("{} - {}", sname, e));
                let metadata = fs::metadata(path).unwrap();
                let mtime = FileTime::from_last_modification_time(&metadata);
                db.add_to_failures(&sname, mtime.unix_seconds(), &format!("{}", e));
            }
        };

        if inc_progress {
            progress.inc(1);
            if notifs.enabled {
                let pc = (progress.position() as f64 * 100.0)/total as f64;
                send_notif(notifs, &format!("ANALYSE {:8.2}% {}", pc, sname));
            }
        }
        if terminate_analysis() {
            break
        }
    }

    if terminate_analysis() {
        progress.abandon_with_message("Terminated!");
    } else {
        progress.finish_with_message("Finished!");
    }
    log::info!("{} Analysed. {} Failed.", analysed, failed.len());
    show_errors(&mut failed, &mut tag_error);
    Ok(())
}

pub fn analyse_files(db_path: &str, mpaths: &Vec<PathBuf>, dry_run: bool, keep_old: bool, max_num_files: usize, 
                     max_threads: usize, ignore_path: &PathBuf, read_tags: bool, write_tags: bool, preserve_mod_times: bool,
                     lms_host: &String, json_port: u16, send_notifs: bool) -> bool {
    let mut db = db::Db::new(&String::from(db_path));
    let mut notifs = NotifInfo {
        enabled: send_notifs,
        address: format!("http://{}:{}/jsonrpc.js", lms_host, json_port),
        last_send: 0,
        start_time: SystemTime::now().duration_since(UNIX_EPOCH).expect("time should go forward").as_secs(),
    };

    ctrlc::set_handler(move || {
        handle_ctrl_c();
    }).expect("Error setting Ctrl-C handler");

    db.init();

    if !keep_old {
        send_notif(&mut notifs, "Removing old files from DB");
        db.remove_old(mpaths, dry_run);
    }

    let mut changes_made = false;
    for path in mpaths {
        let mpath = path.clone();
        let cur = path.clone();
        let mut track_paths: Vec<String> = Vec::new();
        let mut file_count:usize = 0;

        log::info!("Looking for new files in {}", mpath.to_string_lossy());
        send_notif(&mut notifs, &format!("Looking for new files in {}", mpath.to_string_lossy()));

        get_file_list(&mut db, &mpath, &cur, &mut track_paths, &mut file_count, max_num_files, dry_run, &mut notifs);
        track_paths.sort();
        log::info!("New files: {}", track_paths.len());

        if !terminate_analysis() {
            if dry_run {
                if !track_paths.is_empty() {
                    log::info!("The following need to be analysed (or tags read):");
                    for track in track_paths {
                        log::info!("  {}", track);
                    }
                }
            } else {
                if !track_paths.is_empty() {
                    let untagged_paths = if read_tags { check_for_tags(&db, &mpath, track_paths, max_threads, &mut notifs) } else { track_paths };

                    if !untagged_paths.is_empty() {
                        log::info!("New untagged files: {}", untagged_paths.len());
                        match analyse_new_files(&db, &mpath, untagged_paths, max_threads, write_tags, preserve_mod_times, &mut notifs) {
                            Ok(_) => { changes_made = true; }
                            Err(e) => { log::error!("Analysis returned error: {}", e); }
                        }
                    } else {
                        log::info!("No new untagged files to analyse");
                        send_notif(&mut notifs, "No new files to analyse");
                    }
                }
            }
        }
    }

    db.close();
    if changes_made && ignore_path.exists() && ignore_path.is_file() {
        log::info!("Updating 'ignore' flags");
        send_notif(&mut notifs, "Updating ignore");
        db::update_ignore(&db_path, &ignore_path);
    }
    if send_notifs {
        send_notif_msg(&mut notifs, "FINISHED");
    }
    changes_made
}
