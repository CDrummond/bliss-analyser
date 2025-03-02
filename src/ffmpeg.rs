/**
 * Analyse music with Bliss
 *
 * Copyright (c) 2022-2025 Craig Drummond <craig.p.drummond@gmail.com>
 * GPLv3 license.
 *
 **/

use bliss_audio::decoder::Decoder as DecoderTrait;
use bliss_audio::decoder::PreAnalyzedSong;
use bliss_audio::BlissResult;
use byteorder::{LittleEndian, ReadBytesExt};
use std::path::Path;
use std::process::{Command, Stdio};
use std::io;
pub struct FFmpegCmdDecoder;

impl DecoderTrait for FFmpegCmdDecoder {
    fn decode(path: &Path) -> BlissResult<PreAnalyzedSong> {
        let mut decoded_song = PreAnalyzedSong::default();
        if let Ok(mut child) = Command::new("ffmpeg")
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
            let stdout = child.stdout.as_mut().expect("Failed to capture stdout");
            let mut reader = io::BufReader::new(stdout);
            let mut buffer: Vec<f32> = Vec::new();

            while let Ok(sample) = reader.read_f32::<LittleEndian>() {
                buffer.push(sample);
            }

            if let Ok(_) = child.wait() {
                decoded_song.sample_array = buffer;
            }
        }

        Ok(decoded_song)
    }
}