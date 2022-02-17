/**
 * Analyse music with Bliss
 *
 * Copyright (c) 2022 Craig Drummond <craig.p.drummond@gmail.com>
 * GPLv3 license.
 *
 **/

use lofty::{Accessor, ItemKey, Probe};
use std::path::Path;
use crate::db;

pub fn read(track:&String) -> db::Metadata {
    let mut meta = db::Metadata{
        title:String::new(),
        artist:String::new(),
        album:String::new(),
        album_artist:String::new(),
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
                    meta.album_artist=tag.get_string(&ItemKey::AlbumArtist).unwrap_or("").to_string();
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