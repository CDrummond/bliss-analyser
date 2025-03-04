/**
 * Analyse music with Bliss
 *
 * Copyright (c) 2022-2025 Craig Drummond <craig.p.drummond@gmail.com>
 * GPLv3 license.
 *
 **/

use crate::db;
use lofty::{Accessor, AudioFile, ItemKey, ItemValue, Tag, TagExt, TaggedFileExt, TagItem};
use regex::Regex;
use std::path::Path;
use substring::Substring;
use bliss_audio::{Analysis, AnalysisIndex};

const MAX_GENRE_VAL: usize = 192;
const NUM_ANALYSIS_VALS: usize = 20;
const ANALYSIS_TAG:ItemKey = ItemKey::Comment;
const ANALYSIS_TAG_START: &str = "BLISS_ANALYSIS";
const ANALYSIS_TAG_VER: u16 = 1;

pub fn write_analysis(track: &String, analysis: &Analysis) {
    let value = format!("{},{},{:.24},{:.24},{:.24},{:.24},{:.24},{:.24},{:.24},{:.24},{:.24},{:.24},{:.24},{:.24},{:.24},{:.24},{:.24},{:.24},{:.24},{:.24},{:.24},{:.24}", ANALYSIS_TAG_START, ANALYSIS_TAG_VER,
                        analysis[AnalysisIndex::Tempo], analysis[AnalysisIndex::Zcr], analysis[AnalysisIndex::MeanSpectralCentroid], analysis[AnalysisIndex::StdDeviationSpectralCentroid], analysis[AnalysisIndex::MeanSpectralRolloff],
                        analysis[AnalysisIndex::StdDeviationSpectralRolloff], analysis[AnalysisIndex::MeanSpectralFlatness], analysis[AnalysisIndex::StdDeviationSpectralFlatness], analysis[AnalysisIndex::MeanLoudness], analysis[AnalysisIndex::StdDeviationLoudness],
                        analysis[AnalysisIndex::Chroma1], analysis[AnalysisIndex::Chroma2], analysis[AnalysisIndex::Chroma3], analysis[AnalysisIndex::Chroma4], analysis[AnalysisIndex::Chroma5],
                        analysis[AnalysisIndex::Chroma6], analysis[AnalysisIndex::Chroma7], analysis[AnalysisIndex::Chroma8], analysis[AnalysisIndex::Chroma9], analysis[AnalysisIndex::Chroma10]);

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

        tag.push(TagItem::new(ANALYSIS_TAG, ItemValue::Text(value)));
        let _ = tag.save_to_path(Path::new(track));
    }
}

pub fn read(track: &String, read_analysis: bool) -> db::Metadata {
    let mut meta = db::Metadata {
        duration: 180,
        ..db::Metadata::default()
    };

    if let Ok(file) = lofty::read_from_path(Path::new(track)) {
        let tag = match file.primary_tag() {
            Some(primary_tag) => primary_tag,
            None => file.first_tag().expect("Error: No tags found!"),
        };

        meta.title = tag.title().unwrap_or_default().to_string();
        meta.artist = tag.artist().unwrap_or_default().to_string();
        meta.album = tag.album().unwrap_or_default().to_string();
        meta.album_artist = tag.get_string(&ItemKey::AlbumArtist).unwrap_or_default().to_string();
        meta.genre = tag.genre().unwrap_or_default().to_string();

        // Check whether MP3 has numeric genre, and if so covert to text
        if file.file_type().eq(&lofty::FileType::Mpeg) {
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
                                                meta.genre =
                                                    lofty::id3::v1::GENRES[idx].to_string();
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
            let entries = tag.get_strings(&ANALYSIS_TAG);
            for entry in entries {
                if entry.len()>(ANALYSIS_TAG_START.len()+(NUM_ANALYSIS_VALS*8)) && entry.starts_with(ANALYSIS_TAG_START) {
                    let parts = entry.split(",");
                    let mut index = 0;
                    let mut vals = [0.; NUM_ANALYSIS_VALS];
                    for part in parts {
                        if 0==index {
                            if part!=ANALYSIS_TAG_START {
                                break;
                            }
                        } else if 1==index {
                            match part.parse::<u16>() {
                                Ok(ver) => {
                                    if ver!=ANALYSIS_TAG_VER {
                                        break;
                                    }
                                },
                                Err(_) => {
                                    break;
                                }
                            }
                        } else if (index - 2) < NUM_ANALYSIS_VALS {
                            match part.parse::<f32>() {
                                Ok(val) => {
                                    vals[index - 2] = val;
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
                    if index == (NUM_ANALYSIS_VALS+2) {
                        meta.analysis = Some(Analysis::new(vals));
                    }
                    break;
                }
            }
        }
    }

    meta
}
