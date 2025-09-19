/**
 * Analyse music with Bliss
 *
 * Copyright (c) 2022-2025 Craig Drummond <craig.p.drummond@gmail.com>
 * GPLv3 license.
 *
 **/

use crate::cue;
use crate::db;
#[cfg(feature = "ffmpeg")]
use crate::ffmpeg;
use crate::tags;
use anyhow::Result;
#[cfg(feature = "ffmpeg")]
use hhmmss::Hhmmss;
use if_chain::if_chain;
use indicatif::{ProgressBar, ProgressStyle};
#[cfg(not(feature = "ffmpeg"))]
use std::collections::HashSet;
use std::convert::TryInto;
use std::fs::DirEntry;
use std::num::{NonZero, NonZeroUsize};
use std::path::{Path, PathBuf};
#[cfg(feature = "ffmpeg")]
use std::sync::mpsc;
#[cfg(feature = "ffmpeg")]
use std::sync::mpsc::{Receiver, Sender};
#[cfg(feature = "ffmpeg")]
use std::thread;
#[cfg(feature = "ffmpeg")]
use std::time::Duration;
use num_cpus;
#[cfg(feature = "libav")]
use bliss_audio::decoder::ffmpeg::FFmpegDecoder as SongDecoder;
#[cfg(feature = "symphonia")]
use bliss_audio::decoder::symphonia::SymphoniaDecoder as SongDecoder;
#[cfg(feature = "ffmpeg")]
use bliss_audio::{BlissResult, Song, AnalysisOptions, decoder::Decoder};
#[cfg(not(feature = "ffmpeg"))]
use bliss_audio::{AnalysisOptions, decoder::Decoder};
use ureq;
use std::time::{SystemTime, UNIX_EPOCH};

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
    let json = &format!("{{\"id\":1, \"method\":\"slim.request\",\"params\":[\"\",[\"blissmixer\",\"analyser\",\"act:update\",\"msg:{}\"]]}}", text);
    log::info!("Sending notif to LMS: {}", text);
    let _ = ureq::post(&notifs.address).send_string(&json);
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

fn get_file_list(db: &mut db::Db, mpath: &Path, path: &Path, track_paths: &mut Vec<String>, cue_tracks:&mut Vec<cue::CueTrack>, file_count:&mut usize,
                 max_num_files: usize, tagged_file_count:&mut usize, dry_run: bool, notifs: &mut NotifInfo) {
    if !path.is_dir() {
        return;
    }

    send_notif(notifs, &format!("Dir: {}", path.to_string_lossy()));
    let mut items: Vec<_> = path.read_dir().unwrap().map(|r| r.unwrap()).collect();
    items.sort_by_key(|dir| dir.path());

    for item in items {
        check_dir_entry(db, mpath, item, track_paths, cue_tracks, file_count, max_num_files, tagged_file_count, dry_run, notifs);
        if max_num_files>0 && *file_count>=max_num_files {
            break;
        }
    }
}

fn check_dir_entry(db: &mut db::Db, mpath: &Path, entry: DirEntry, track_paths: &mut Vec<String>, cue_tracks:&mut Vec<cue::CueTrack>, file_count:&mut usize,
                   max_num_files: usize, tagged_file_count:&mut usize, dry_run: bool, notifs: &mut NotifInfo) {
    let pb = entry.path();
    if pb.is_dir() {
        let check = pb.join(DONT_ANALYSE);
        if check.exists() {
            log::info!("Skipping '{}', found '{}'", pb.to_string_lossy(), DONT_ANALYSE);
        } else if max_num_files<=0 || *file_count<max_num_files {
            get_file_list(db, mpath, &pb, track_paths, cue_tracks, file_count, max_num_files, tagged_file_count, dry_run, notifs);
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

                            #[cfg(not(feature = "ffmpeg"))]
                            if id<=0 {
                                track_paths.push(String::from(cue_file.to_string_lossy()));
                                *file_count+=1;
                            }

                            #[cfg(feature = "ffmpeg")]
                            if id<=0 {
                                let this_cue_tracks = cue::parse(&pb, &cue_file);
                                for track in this_cue_tracks {
                                    cue_tracks.push(track.clone());
                                }
                                *file_count+=1;
                            }

                        }
                    }
                } else {
                    if let Ok(id) = db.get_rowid(&sname) {
                        if id<=0 {
                            let mut tags_used = false;
                            let meta = tags::read(&String::from(pb.to_string_lossy()), true);
                            if !meta.is_empty() && !meta.analysis.is_none() {
                                if !dry_run {
                                    db.add_track(&sname, &meta.clone(), &meta.analysis.unwrap());
                                }
                                *tagged_file_count+=1;
                                tags_used = true;
                            }

                            if !tags_used {
                                track_paths.push(String::from(pb.to_string_lossy()));
                                *file_count+=1;
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

#[cfg(not(feature = "ffmpeg"))]
fn analyse_new_files(db: &db::Db, mpath: &PathBuf, track_paths: Vec<String>, max_threads: usize, write_tags: bool,
                     preserve_mod_times: bool, notifs: &mut NotifInfo) -> Result<()> {
    let total = track_paths.len();
    let progress = ProgressBar::new(total.try_into().unwrap()).with_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:25}] {percent:>3}% {pos:>6}/{len:6} {wide_msg}")
            .progress_chars("=> "),
    );
    let cpu_threads: NonZeroUsize = match max_threads {
        1111 => NonZeroUsize::new(num_cpus::get()).unwrap(),
        0    => NonZeroUsize::new(num_cpus::get()).unwrap(),
        _    => NonZeroUsize::new(max_threads).unwrap(),
    };

    let mut analysed = 0;
    let mut failed: Vec<String> = Vec::new();
    let mut tag_error: Vec<String> = Vec::new();
    let mut reported_cue:HashSet<String> = HashSet::new();
    let mut options:AnalysisOptions = AnalysisOptions::default();
    if max_threads==1111 && <NonZero<usize> as Into<usize>>::into(cpu_threads)>1 {
        options.number_cores = NonZero::new(<NonZero<usize> as Into<usize>>::into(cpu_threads) -1).unwrap();
    } else {
        options.number_cores = cpu_threads;
    }

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
            Err(e) => { failed.push(format!("{} - {}", sname, e)); }
        };

        if inc_progress {
            progress.inc(1);
            if notifs.enabled {
                let pc = (progress.position() as f64 * 100.0)/total as f64;
                send_notif(notifs, &format!("{:8.2}% {}", pc, sname));
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

#[cfg(feature = "ffmpeg")]
fn analyse_new_files(db: &db::Db, mpath: &PathBuf, track_paths: Vec<String>, max_threads: usize, write_tags: bool, 
                     preserve_mod_times: bool, lms_host: &String, json_port: u16, notifs: &mut NotifInfo) -> Result<()> {
    let total = track_paths.len();
    let progress = ProgressBar::new(total.try_into().unwrap()).with_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:25}] {percent:>    if max_threads==1111 && cpu_threads>1 {
        options.number_cores = cpu_threads -1;3}% {pos:>6}/{len:6} {wide_msg}")
            .progress_chars("=> "),
    );
    let cpu_threads: NonZeroUsize = match max_threads {
        1111 => NonZeroUsize::new(num_cpus::get()).unwrap(),
        0    => NonZeroUsize::new(num_cpus::get()).unwrap(),
        _    => NonZeroUsize::new(max_threads).unwrap(),
    };

    let mut analysed = 0;
    let mut failed: Vec<String> = Vec::new();
    let mut tag_error: Vec<String> = Vec::new();
    let mut options:AnalysisOptions = AnalysisOptions::default();
    if max_threads==1111 && <NonZero<usize> as Into<usize>>::into(cpu_threads)>1 {
        options.number_cores = NonZero::new(<NonZero<usize> as Into<usize>>::into(cpu_threads) -1).unwrap();
    } else {
        options.number_cores = cpu_threads;
    }

    log::info!("Analysing new files");
    for (path, result) in <ffmpeg::FFmpegCmdDecoder as Decoder>::analyze_paths_with_options(track_paths, options) {
        let stripped = path.strip_prefix(mpath).unwrap();
        let spbuff = stripped.to_path_buf();
        let sname = String::from(spbuff.to_string_lossy());
        progress.set_message(format!("{}", sname));
        match result {
            Ok(track) => {
                let cpath = String::from(path.to_string_lossy());
                let mut meta = tags::read(&cpath, false);
                if meta.is_empty() {
                    meta = ffmpeg::read_tags(&cpath);
                }
                if meta.is_empty() {
                    tag_error.push(sname.clone());
                }
                if write_tags {
                    tags::write_analysis(&cpath, &track.analysis, preserve_mod_times);
                }
                db.add_track(&sname, &meta, &track.analysis);
                analysed += 1;
            }
            Err(e) => { failed.push(format!("{} - {}", sname, e)); }
        };

        progress.inc(1);
        if terminate_analysis() {
            break
        }
    }

    // Reset terminal, otherwise typed output does not show? Perhaps Linux only...
    if ! cfg!(windows) {
        match std::process::Command::new("stty").arg("sane").spawn() {
            Ok(_) => { },
            Err(_)    => { },
        };
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

#[cfg(feature = "ffmpeg")]
fn analyze_cue_streaming(tracks: Vec<cue::CueTrack>,) -> BlissResult<Receiver<(cue::CueTrack, BlissResult<Song>)>> {
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

#[cfg(feature = "ffmpeg")]
fn analyse_new_cue_tracks(db:&db::Db, mpath: &PathBuf, cue_tracks:Vec<cue::CueTrack>) -> Result<()> {
    let total = cue_tracks.len();
    let progress = ProgressBar::new(total.try_into().unwrap()).with_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:25}] {percent:>3}% {pos:>6}/{len:6} {wide_msg}")
            .progress_chars("=> "),
    );

    let results = analyze_cue_streaming(cue_tracks)?;
    let mut analysed = 0;
    let mut failed:Vec<String> = Vec::new();
    let mut tag_error: Vec<String> = Vec::new();
    let last_track_duration = Duration::new(cue::LAST_TRACK_DURATION, 0);

    log::info!("Analysing new cue tracks");
    for (track, result) in results {
        let stripped = track.track_path.strip_prefix(mpath).unwrap();
        let spbuff = stripped.to_path_buf();
        let sname = String::from(spbuff.to_string_lossy());
        progress.set_message(format!("{}", sname));
        match result {
            Ok(song) => {
                let meta = db::Metadata {
                    title:track.title,
                    artist:track.artist,
                    album_artist:track.album_artist,
                    album:track.album,
                    genre:track.genre,
                    duration:if track.duration>=last_track_duration { song.duration.as_secs() as u32 } else { track.duration.as_secs() as u32 },
                    analysis: None
                };

                db.add_track(&sname, &meta, &song.analysis);
                analysed += 1;
            },
            Err(e) => {
                failed.push(format!("{} - {}", sname, e));
            }
        };
        progress.inc(1);
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
                     max_threads: usize, ignore_path: &PathBuf, write_tags: bool, preserve_mod_times: bool,
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
        let mut cue_tracks:Vec<cue::CueTrack> = Vec::new();
        let mut file_count:usize = 0;
        let mut tagged_file_count:usize = 0;

        log::info!("Looking for new files in {}", mpath.to_string_lossy());
        send_notif(&mut notifs, &format!("Looking for new files in {}", mpath.to_string_lossy()));

        get_file_list(&mut db, &mpath, &cur, &mut track_paths, &mut cue_tracks, &mut file_count, max_num_files, 
                      &mut tagged_file_count, dry_run, &mut notifs);
        track_paths.sort();
        log::info!("New untagged files: {}", track_paths.len());
        if !cue_tracks.is_empty() {
            log::info!("New cue tracks: {}", cue_tracks.len());
        }
        log::info!("New tagged files: {}", tagged_file_count);

        if !terminate_analysis() {
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
                if !track_paths.is_empty() {
                    match analyse_new_files(&db, &mpath, track_paths, max_threads, write_tags, preserve_mod_times, &mut notifs) {
                        Ok(_) => { changes_made = true; }
                        Err(e) => { log::error!("Analysis returned error: {}", e); }
                    }
                } else {
                    log::info!("No new files to analyse");
                    send_notif(&mut notifs, "No new files to analyse");
                }

                #[cfg(feature = "ffmpeg")]
                if !cue_tracks.is_empty() && !terminate_analysis() {
                    match analyse_new_cue_tracks(&db, &mpath, cue_tracks) {
                        Ok(_) => { changes_made = true; },
                        Err(e) => { log::error!("Cue analysis returned error: {}", e); }
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
