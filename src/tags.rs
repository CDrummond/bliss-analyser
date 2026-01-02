/**
 * Analyse music with Bliss
 *
 * Copyright (c) 2022-2026 Craig Drummond <craig.p.drummond@gmail.com>
 * GPLv3 license.
 *
 **/

use crate::db;
use lofty::config::WriteOptions;
use lofty::file::FileType;
use lofty::prelude::{Accessor, AudioFile, ItemKey, TagExt, TaggedFileExt};
use lofty::tag::{ItemValue, Tag, TagItem};
use regex::Regex;
use std::fs::File;
use std::fs;
use std::path::Path;
use substring::Substring;
use std::time::SystemTime;
use bliss_audio::{Analysis, AnalysisIndex, FeaturesVersion};

const MAX_GENRE_VAL: usize = 192;
const NUM_ANALYSIS_VALS: usize = 23;
const ANALYSIS_TAG: &str = "BLISS_ANALYSIS";
const ANALYSIS_TAG_FORMAT_VER: u16 = 2;

fn fmt(val: f32) -> String {
    format!("{:.16}", val).trim_end_matches("0").to_string()
}

pub fn write_analysis(track: &String, analysis: &Analysis, preserve_mod_times: bool) -> bool {
    let value = format!("{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}", ANALYSIS_TAG_FORMAT_VER,
                        fmt(analysis[AnalysisIndex::Tempo]), fmt(analysis[AnalysisIndex::Zcr]), fmt(analysis[AnalysisIndex::MeanSpectralCentroid]), fmt(analysis[AnalysisIndex::StdDeviationSpectralCentroid]), fmt(analysis[AnalysisIndex::MeanSpectralRolloff]),
                        fmt(analysis[AnalysisIndex::StdDeviationSpectralRolloff]), fmt(analysis[AnalysisIndex::MeanSpectralFlatness]), fmt(analysis[AnalysisIndex::StdDeviationSpectralFlatness]), fmt(analysis[AnalysisIndex::MeanLoudness]), fmt(analysis[AnalysisIndex::StdDeviationLoudness]),
                        fmt(analysis[AnalysisIndex::Chroma1]), fmt(analysis[AnalysisIndex::Chroma2]), fmt(analysis[AnalysisIndex::Chroma3]), fmt(analysis[AnalysisIndex::Chroma4]), fmt(analysis[AnalysisIndex::Chroma5]),
                        fmt(analysis[AnalysisIndex::Chroma6]), fmt(analysis[AnalysisIndex::Chroma7]), fmt(analysis[AnalysisIndex::Chroma8]), fmt(analysis[AnalysisIndex::Chroma9]), fmt(analysis[AnalysisIndex::Chroma10]),
                        fmt(analysis[AnalysisIndex::Chroma11]), fmt(analysis[AnalysisIndex::Chroma12]), fmt(analysis[AnalysisIndex::Chroma13]));

    let mut written = false;
    if let Ok(mut file) = lofty::read_from_path(Path::new(track)) {
        let tag = match file.primary_tag_mut() {
            Some(primary_tag) => primary_tag,
            None => {
                if let Some(first_tag) = file.first_tag_mut() {
                    first_tag
                } else {
                    let tag_type = file.primary_tag_type();
                    file.insert_tag(Tag::new(tag_type));
                    file.primary_tag_mut().unwrap()
                }
            },
        };

        // Store analysis results
        let tag_key = ItemKey::Unknown(ANALYSIS_TAG.to_string());
        tag.remove_key(&tag_key);
        let lower_tag_key = ItemKey::Unknown(ANALYSIS_TAG.to_lowercase().to_string());
        tag.remove_key(&lower_tag_key);
        tag.insert_unchecked(TagItem::new(tag_key, ItemValue::Text(value)));

        // If we have any of the older analysis-in-comment tags, then remove these
        let entries = tag.get_strings(&ItemKey::Comment);
        let mut keep: Vec<ItemValue> = Vec::new();
        let mut have_old = false;
        for entry in entries {
            if entry.starts_with(ANALYSIS_TAG) {
                have_old = true;
            } else {
                keep.push(ItemValue::Text(entry.to_string()));
            }
        }
        if have_old {
            tag.remove_key(&ItemKey::Comment);
            for k in keep {
                tag.push(TagItem::new(ItemKey::Comment, k));
            }
        }

        let now = SystemTime::now();
        let mut mod_time = now;

        if preserve_mod_times {
            if let Ok(fmeta) = fs::metadata(track) {
                if let Ok(time) = fmeta.modified() {
                    mod_time = time;
                }
            }
        }
        if let Ok(_) = tag.save_to_path(Path::new(track), WriteOptions::default()) {
            if preserve_mod_times {
                if mod_time<now {
                    if let Ok(f) = File::open(track) {
                        let _ = f.set_modified(mod_time);
                    }
                }
            }
            written = true;
        }
    }
    written
}

fn read_analysis_string(tag_str: &str, start_tag_pos:usize, version_pos:usize) -> Option<Analysis> {
    let parts = tag_str.split(",");
    let mut index = 0;
    let mut num_read_vals = 0;
    let mut vals = [0.; NUM_ANALYSIS_VALS];
    let val_start_pos = version_pos+1;
    for part in parts {
        if index==start_tag_pos && start_tag_pos<version_pos {
            if part!=ANALYSIS_TAG {
                break;
            }
        } else if index==version_pos {
            match part.parse::<u16>() {
                Ok(ver) => {
                    if ver!=ANALYSIS_TAG_FORMAT_VER {
                        break;
                    }
                },
                Err(_) => {
                    break;
                }
            }
        } else if (index - val_start_pos) < NUM_ANALYSIS_VALS {
            match part.parse::<f32>() {
                Ok(val) => {
                    num_read_vals += 1;
                    vals[index - val_start_pos] = val;
                },
                Err(_) => {
                    break;
                }
            }
        } else {
            break;
        }
        index += 1;
    }
    if num_read_vals == NUM_ANALYSIS_VALS {
        return Some(Analysis::new(vals.to_vec(), FeaturesVersion::LATEST).expect("number of vals should be 23"));
    }
    None
}

pub fn read(track: &String, read_analysis: bool) -> db::Metadata {
    let mut meta = db::Metadata {
        duration: 180,
        ..db::Metadata::default()
    };

    if let Ok(file) = lofty::read_from_path(Path::new(track)) {
        let tag = match file.primary_tag() {
            Some(primary_tag) => primary_tag,
            None => {
                if let Some(first_tag) = file.first_tag() {
                    first_tag
                } else {
                    return meta;
                }
            }
        };

        meta.title = tag.title().unwrap_or_default().to_string();
        meta.artist = tag.artist().unwrap_or_default().to_string();
        meta.album = tag.album().unwrap_or_default().to_string();
        meta.album_artist = tag.get_string(&ItemKey::AlbumArtist).unwrap_or_default().to_string();

        // If file has multiple genre tags then read all.
        let genres = tag.get_strings(&ItemKey::Genre);
        let mut genre_list:Vec<String> = Vec::new();

        for genre in genres {
            genre_list.push(genre.to_string());
        }
        if genre_list.len()>1 {
            meta.genre = genre_list.join(";");
        } else {
            meta.genre = tag.genre().unwrap_or_default().to_string();
        }

        // Check whether MP3 has numeric genre, and if so covert to text
        if file.file_type().eq(&FileType::Mpeg) {
            match tag.genre() {
                Some(genre) => {
                    let test = genre.parse::<u8>();
                    match test {
                        Ok(val) => {
                            let idx: usize = val as usize;
                            if idx < MAX_GENRE_VAL {
                                meta.genre = lofty::id3::v1::GENRES[idx].to_string();
                            }
                        }
                        Err(_) => {
                            // Check for "(number)text"
                            let re = Regex::new(r"^\([0-9]+\)").unwrap();
                            if re.is_match(&genre) {
                                match genre.find(")") {
                                    Some(end) => {
                                        let test = genre.to_string().substring(1, end).parse::<u8>();

                                        if let Ok(val) = test {
                                            let idx: usize = val as usize;
                                            if idx < MAX_GENRE_VAL {
                                                meta.genre = lofty::id3::v1::GENRES[idx].to_string();
                                            }
                                        }
                                    }
                                    None => { }
                                }
                            }
                        }
                    }
                }
                None => { }
            }
        }

        meta.duration = file.properties().duration().as_secs() as u32;

        if read_analysis {
            match tag.get_string(&ItemKey::Unknown(ANALYSIS_TAG.to_string())) {
                Some(tag_str) => {
                    match read_analysis_string(tag_str, 100, 0) {
                        Some(analysis) => {
                            meta.analysis = Some(analysis);
                        }
                        None => { }
                    }
                }
                None => { }
            }

            if meta.analysis.is_none() {
                // Try lowercase
                match tag.get_string(&ItemKey::Unknown(ANALYSIS_TAG.to_lowercase().to_string())) {
                    Some(tag_str) => {
                        match read_analysis_string(tag_str, 100, 0) {
                            Some(analysis) => {
                                meta.analysis = Some(analysis);
                            }
                            None => { }
                        }
                    }
                    None => { }
                }
            }

            if meta.analysis.is_none() {
                // Try old, stored in comment
                let entries = tag.get_strings(&ItemKey::Comment);
                for entry in entries {
                    if entry.len()>(ANALYSIS_TAG.len()+(NUM_ANALYSIS_VALS*8)) && entry.starts_with(ANALYSIS_TAG) {
                        match read_analysis_string(entry, 0, 1) {
                            Some(analysis) => {
                                meta.analysis = Some(analysis);
                                break;
                            }
                            None => { }
                        }
                    }
                }
            }
        }
    }

    meta
}
