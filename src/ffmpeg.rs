/**
 * Analyse music with Bliss
 *
 * Copyright (c) 2022-2025 Craig Drummond <craig.p.drummond@gmail.com>
 * GPLv3 license.
 *
 **/

use crate::db;
use bliss_audio::decoder::Decoder as DecoderTrait;
use bliss_audio::decoder::PreAnalyzedSong;
use bliss_audio::{BlissError, BlissResult};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::io;
use std::io::Read;
use std::io::BufRead;
use std::io::BufReader;
use std::time::Duration;

pub const TIME_SEP:&str = "<TIME>";

pub struct FFmpegCmdDecoder;

fn handle_command(mut child: Child) -> BlissResult<PreAnalyzedSong> {
    let mut decoded_song = PreAnalyzedSong::default();
    let stdout = child.stdout.as_mut().expect("Failed to capture stdout");
    let mut reader = io::BufReader::new(stdout);
    let mut buffer: Vec<u8> = Vec::new();
    reader.read_to_end(&mut buffer).map_err(|e| {
        BlissError::DecodingError(format!("Could not read the decoded file into a buffer: {}", e))
    })?;

    decoded_song.sample_array = buffer
        .chunks_exact(4)
        .map(|x| {
            let mut a: [u8; 4] = [0; 4];
            a.copy_from_slice(x);
            f32::from_le_bytes(a)
        })
        .collect();
    let duration_seconds = decoded_song.sample_array.len() as f32 / 22050 as f32;
    decoded_song.duration = Duration::from_nanos((duration_seconds * 1e9_f32).round() as u64);
    Ok(decoded_song)
}

fn get_val(line: String) -> String {
    let parts = line.split("=");
    let mut resp:Vec<String> = Vec::new();
    let mut first =true;
    for part in parts {
        if !first {
            resp.push(String::from(part));
        }
        first = false
    }
    resp.join("=")
}

pub fn read_tags(path: &String) -> db::Metadata {
    let mut meta = db::Metadata {
        duration: 0,
        ..db::Metadata::default()
    };

    if let Ok(child) = Command::new("ffprobe")
                                .arg("-hide_banner")
                                .arg("-v").arg("quiet")
                                .arg("-show_entries").arg("format")
                                .arg(path)
                                .stdout(Stdio::piped())
                                .spawn() {
        let out = child.stdout.unwrap();
        let lines = BufReader::new(out).lines().filter_map(|line| line.ok());
        for line in lines {
            if line.starts_with("duration=") {
                let val = get_val(line);
                match val.parse::<f32>() {
                    Ok(v) => {
                        meta.duration = v as u32;
                    },
                    Err(_) => { }
                }
            } else if line.starts_with("TAG:title=") {
                meta.title = get_val(line);
            } else if line.starts_with("TAG:artist=") {
                meta.artist = get_val(line);
            } else if line.starts_with("TAG:album=") {
                meta.album = get_val(line);
            } else if line.starts_with("TAG:album_artist=") {
                meta.album_artist = get_val(line);
            } else if line.starts_with("TAG:genre=") {
                meta.genre = get_val(line);
            }
        }
    }
    meta
}

impl DecoderTrait for FFmpegCmdDecoder {
    fn decode(path: &Path) -> BlissResult<PreAnalyzedSong> {
        let binding = path.to_string_lossy();
        // First check if this is a CUE file track - which will have start and duration
        let mut parts = binding.split(TIME_SEP);
        if parts.clone().count()==3 {
            if let Ok(child) = Command::new("ffmpeg")
                                .arg("-hide_banner")
                                .arg("-loglevel").arg("panic")
                                .arg("-i").arg(parts.next().unwrap_or(""))
                                .arg("-ss").arg(parts.next().unwrap_or(""))
                                .arg("-t").arg(parts.next().unwrap_or(""))
                                .arg("-ar").arg("22050")
                                .arg("-ac").arg("1")
                                .arg("-c:a")
                                .arg("pcm_f32le")
                                .arg("-f").arg("f32le")
                                .arg("pipe:1")
                                .stdout(Stdio::piped())
                                .spawn() {
                return handle_command(child);
            }
        } else {
            if let Ok(child) = Command::new("ffmpeg")
                                .arg("-hide_banner")
                                .arg("-loglevel").arg("panic")
                                .arg("-i").arg(path)
                                .arg("-ar").arg("22050")
                                .arg("-ac").arg("1")
                                .arg("-c:a")
                                .arg("pcm_f32le")
                                .arg("-f").arg("f32le")
                                .arg("pipe:1")
                                .stdout(Stdio::piped())
                                .spawn() {
                return handle_command(child);
            }
        }

        Err(BlissError::DecodingError("ffmpeg command failed".to_string()))
    }
}