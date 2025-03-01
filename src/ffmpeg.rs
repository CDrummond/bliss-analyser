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
use std::fs;
use std::path::Path;
use std::process::Command;
use md5;

pub struct FFmpegCmdDecoder;

impl DecoderTrait for FFmpegCmdDecoder {
    fn decode(path: &Path) -> BlissResult<PreAnalyzedSong> {
        let mut decoded_song = PreAnalyzedSong::default();
        let digest = md5::compute(path.to_str().unwrap_or("null").as_bytes().to_vec());
        let tmp_path = format!("/tmp/{:x}.wav", digest);
        let _ = Command::new("ffmpeg").arg("-i").arg(path).arg("-ar").arg("22050").arg("-ac").arg("1").arg("-c:a").arg("pcm_f32le").arg(tmp_path.clone()).output();
        let cloned_path = tmp_path.clone();
        let wav_file = Path::new(&cloned_path);
        if wav_file.exists() {
            let mut reader = hound::WavReader::open(tmp_path).unwrap();
            decoded_song.sample_array = reader.samples::<f32>().flatten().collect();
            let _ = fs::remove_file(wav_file);
        }
        Ok(decoded_song)
    }
}