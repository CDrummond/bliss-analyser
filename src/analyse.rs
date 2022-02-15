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
use lofty::{Accessor, Probe};
use std::convert::TryInto;
use std::path::{Path, PathBuf};
use crate::db;

const DONT_ANALYSE:&str = ".nomusic";

fn get_file_list(db:&mut db::Db, mpath:& Path, path: &Path, to_add:&mut Vec<String>) {
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
                                    get_file_list(db, mpath, &entry.path(), to_add);
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
                                                                to_add.push(String::from(pb.to_string_lossy()));
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

pub fn read_metadata(track:&String) -> db::Metadata {
    let mut meta = db::Metadata{
        title:String::new(),
        artist:String::new(),
        album:String::new(),
        genre:String::new(),
        duration:180
    };
    let path = Path::new(track);
    match Probe::open(path) {
        Ok(probe) => {
            match probe.read(true) {
                Ok(file) => {
                    let tag = match file.primary_tag() {
                        Some(primary_tag) => primary_tag,
                        None => file.first_tag().expect("Error: No tags found!"),
                    };

                    meta.title=tag.title().unwrap_or("").to_string();
                    meta.artist=tag.artist().unwrap_or("").to_string();
                    meta.album=tag.album().unwrap_or("").to_string();
                    meta.genre=tag.genre().unwrap_or("").to_string();
                    meta.duration=file.properties().duration().as_secs() as u32;
                },
                Err(_) => { }
            }
        },
        Err(_) => { }
    }
    meta
}

pub fn analyse_new_files(db:&db::Db, mpath: &Path, to_add:Vec<String>) -> Result<()> {
    let total = to_add.len();
    let pb = ProgressBar::new(total.try_into().unwrap());
    let style = ProgressStyle::default_bar()
        .template("[{elapsed_precise}] {bar:40} {pos:>7}/{len:7} {wide_msg}")
        .progress_chars("##-");
    pb.set_style(style);

    let results = analyze_paths_streaming(to_add)?;
    let mut analysed = 0;
    let mut failed = 0;
    let mut tag_error = 0;

    for (path, result) in results {
        pb.set_message(format!("Analysing {}", path));
        match result {
            Ok(track) => {
                let meta = read_metadata(&path);
                let pb = PathBuf::from(path);
                if meta.title.is_empty() && meta.artist.is_empty() && meta.album.is_empty() && meta.genre.is_empty() {
                    tag_error += 1;
                }
                match pb.strip_prefix(mpath) {
                    Ok(stripped) => {
                        let spb = stripped.to_path_buf();
                        let sname = String::from(spb.to_string_lossy());
                        db.add_track(&sname, &meta, &track.analysis);
                    },
                    Err(_) => { }
                }
                analysed += 1;
            },
            Err(_) => {
                failed += 1;
            }
        };
        pb.inc(1);
    }
    pb.finish_with_message(format!("{} Analyzed. {} Failure(s). {} Tag error(s).", analysed, failed, tag_error));
    Ok(())
}

pub fn analyse_files(db_path: &str, mpath: &Path, path: &Path, dry_run:bool, keep_old:bool) {
    let mut to_add:Vec<String> = Vec::new();
    let mut db = db::Db::new(&String::from(db_path));

    db.init();
    get_file_list(&mut db, mpath, path, &mut to_add);
    log::info!("Num new tracks: {}", to_add.len());
    if !keep_old {
        db.remove_old(mpath, dry_run);
    }
    if !dry_run {
        to_add.sort();
        match analyse_new_files(&db, mpath, to_add) {
            Ok(_) => { },
            Err(_) => { }
        }
    }

    db.close();
}
