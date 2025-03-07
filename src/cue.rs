/**
 * Analyse music with Bliss
 *
 * Copyright (c) 2022-2025 Craig Drummond <craig.p.drummond@gmail.com>
 * GPLv3 license.
 *
 **/

extern crate rcue;

#[cfg(not(feature = "libav"))]
use crate::db;
#[cfg(not(feature = "libav"))]
use rcue::parser::parse_from_file;
use std::path::PathBuf;
#[cfg(not(feature = "libav"))]
use std::time::Duration;

#[cfg(not(feature = "libav"))]
pub const LAST_TRACK_DURATION:u64 = 60*60*24;
#[cfg(not(feature = "libav"))]
const GENRE:&str = "GENRE";

#[cfg(feature = "libav")]
#[derive(Clone)]
pub struct CueTrack {
    pub track_path:PathBuf
}

#[cfg(not(feature = "libav"))]
#[derive(Clone)]
pub struct CueTrack {
    pub audio_path:PathBuf,
    pub track_path:PathBuf,
    pub title:String,
    pub artist:String,
    pub album:String,
    pub album_artist:String,
    pub genre:String,
    pub start:Duration,
    pub duration:Duration
}

#[cfg(not(feature = "libav"))]
pub fn parse(audio_path:&PathBuf, cue_path:&PathBuf) -> Vec<CueTrack> {
    let mut resp:Vec<CueTrack> = Vec::new();

    match parse_from_file(&cue_path.to_string_lossy(), false) {
        Ok(cue) => {
            let album = cue.title.unwrap_or(String::new());
            let album_artist = cue.performer.unwrap_or(String::new());
            let mut genre = String::new();
            for comment in cue.comments {
                if comment.0.eq(GENRE) {
                    genre = comment.1;
                }
            }
            if 1 == cue.files.len() {
                for file in cue.files {
                    for track in file.tracks {
                        match track.indices.get(0) {
                            Some((_, start)) => {
                                let mut track_path = audio_path.clone();
                                let ext = audio_path.extension().unwrap().to_string_lossy();
                                track_path.set_extension(format!("{}{}{}", ext, db::CUE_MARKER, resp.len()+1));
                                let mut ctrack = CueTrack {
                                    audio_path: audio_path.clone(),
                                    track_path: track_path,
                                    title: track.title.unwrap_or(String::new()),
                                    artist: track.performer.unwrap_or(String::new()),
                                    album_artist: album_artist.clone(),
                                    album: album.clone(),
                                    genre: genre.clone(),
                                    start: start.clone(),
                                    duration: Duration::new(LAST_TRACK_DURATION, 0),
                                };
                                if ctrack.artist.is_empty() && !ctrack.album_artist.is_empty() {
                                    ctrack.artist = ctrack.album_artist.clone();
                                }
                                if ctrack.album.is_empty() {
                                    let mut path = audio_path.clone();
                                    path.set_extension("");
                                    match path.file_name() {
                                        Some(n) => { ctrack.album = String::from(n.to_string_lossy()); }
                                        None => { }
                                    }
                                }
                                resp.push(ctrack);
                            },
                            None => { }
                        }
                    }
                }
            }
        },
        Err(e) => { log::error!("Failed to parse '{}'. {}", cue_path.to_string_lossy(), e);}
    }

    for i in 0..(resp.len()-1) {
        let mut next_start = Duration::new(0, 0);
        if let Some(next) = resp.get(i+1) {
            next_start = next.start;
        }
        if let Some(elem) = resp.get_mut(i) {
            (*elem).duration = next_start - elem.start;
        }
    }
    resp
}