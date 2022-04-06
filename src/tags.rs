use crate::db;
/**
 * Analyse music with Bliss
 *
 * Copyright (c) 2022 Craig Drummond <craig.p.drummond@gmail.com>
 * GPLv3 license.
 *
 **/
use lofty::{Accessor, AudioFile, ItemKey};
use regex::Regex;
use std::path::Path;
use substring::Substring;

const MAX_GENRE_VAL: usize = 192;

pub fn read(track: &String) -> db::Metadata {
    let mut meta = db::Metadata {
        duration: 180,
        ..db::Metadata::default()
    };

    if let Ok(file) = lofty::read_from_path(Path::new(track), true) {
        let tag = match file.primary_tag() {
            Some(primary_tag) => primary_tag,
            None => file.first_tag().expect("Error: No tags found!"),
        };

        meta.title = tag.title().unwrap_or_default().to_string();
        meta.artist = tag.artist().unwrap_or_default().to_string();
        meta.album = tag.album().unwrap_or_default().to_string();
        meta.album_artist = tag
            .get_string(&ItemKey::AlbumArtist)
            .unwrap_or_default()
            .to_string();
        meta.genre = tag.genre().unwrap_or_default().to_string();

        // Check whether MP3 as numeric genre, and if so covert to text
        if file.file_type().eq(&lofty::FileType::MP3) {
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
                                        let test =
                                            genre.to_string().substring(1, end).parse::<u8>();

                                        if let Ok(val) = test {
                                            let idx: usize = val as usize;
                                            if idx < MAX_GENRE_VAL {
                                                meta.genre =
                                                    lofty::id3::v1::GENRES[idx].to_string();
                                            }
                                        }
                                    }
                                    None => {}
                                }
                            }
                        }
                    }
                }
                None => {}
            }
        }

        meta.duration = file.properties().duration().as_secs() as u32;
    }

    meta
}
