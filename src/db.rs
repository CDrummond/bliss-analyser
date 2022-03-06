/**
 * Analyse music with Bliss
 *
 * Copyright (c) 2022 Craig Drummond <craig.p.drummond@gmail.com>
 * GPLv3 license.
 *
 **/

use bliss_audio::{Analysis, AnalysisIndex};
use indicatif::{ProgressBar, ProgressStyle};
use rusqlite::{Connection, params};
use std::convert::TryInto;
use std::path::{Path, PathBuf};
use std::process;
use crate::tags;

pub struct FileMetadata {
    pub rowid:usize,
    pub file:String,
    pub title:Option<String>,
    pub artist:Option<String>,
    pub album_artist:Option<String>,
    pub album:Option<String>,
    pub genre:Option<String>,
    pub duration:u32
}

pub struct Metadata {
    pub title:String,
    pub artist:String,
    pub album_artist:String,
    pub album:String,
    pub genre:String,
    pub duration:u32
}

pub struct Db {
    pub conn: Connection
}

impl Db {
    pub fn new(path: &String) -> Self {
        Self {
            conn: Connection::open(path).unwrap(),
        }
    }

    pub fn init(&self) {
        match self.conn.execute(
            "CREATE TABLE IF NOT EXISTS Tracks (
                File text primary key,
                Title text,
                Artist text,
                Album text,
                AlbumArtist text,
                Genre text,
                Duration integer,
                Ignore integer,
                Tempo real,
                Zcr real,
                MeanSpectralCentroid real,
                StdDevSpectralCentroid real,
                MeanSpectralRolloff real,
                StdDevSpectralRolloff real,
                MeanSpectralFlatness real,
                StdDevSpectralFlatness real,
                MeanLoudness real,
                StdDevLoudness real,
                Chroma1 real,
                Chroma2 real,
                Chroma3 real,
                Chroma4 real,
                Chroma5 real,
                Chroma6 real,
                Chroma7 real,
                Chroma8 real,
                Chroma9 real,
                Chroma10 real
            );",[]) {
            Ok(_) => { },
            Err(_) => {
                log::error!("Failed to create DB table");
                process::exit(-1);
            }
        }
        match self.conn.execute("CREATE UNIQUE INDEX IF NOT EXISTS Tracks_idx ON Tracks(File)", []) {
            Ok(_) => { },
            Err(_) => {
                log::error!("Failed to create DB index");
                process::exit(-1);
            }
        }
    }

    pub fn close(self) {
        match self.conn.close() {
            Ok(_) => { },
            Err(_) => { }
        }
    }

    pub fn get_rowid(&self, path: &String) -> Result<usize, rusqlite::Error> {
        let mut db_path = path.clone();
        if cfg!(windows) {
            db_path = db_path.replace("\\", "/");
        }
        let mut stmt = self.conn.prepare("SELECT rowid FROM Tracks WHERE File=:path;")?;
        let track_iter = stmt.query_map(&[(":path", &db_path)], |row| {
            Ok(row.get(0)?)
        }).unwrap();
        let mut rowid:usize = 0;
        for tr in track_iter {
            rowid = tr.unwrap();
            break;
        }
        Ok(rowid)
    }

    pub fn add_track(&self, path: &String, meta: &Metadata, analysis:&Analysis) {
        let mut db_path = path.clone();
        if cfg!(windows) {
            db_path = db_path.replace("\\", "/");
        }
        match self.get_rowid(&path) {
            Ok(id) => {
                if id<=0 {
                    match self.conn.execute("INSERT INTO Tracks (File, Title, Artist, AlbumArtist, Album, Genre, Duration, Ignore, Tempo, Zcr, MeanSpectralCentroid, StdDevSpectralCentroid, MeanSpectralRolloff, StdDevSpectralRolloff, MeanSpectralFlatness, StdDevSpectralFlatness, MeanLoudness, StdDevLoudness, Chroma1, Chroma2, Chroma3, Chroma4, Chroma5, Chroma6, Chroma7, Chroma8, Chroma9, Chroma10) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);",
                            params![db_path, meta.title, meta.artist, meta.album_artist, meta.album, meta.genre, meta.duration, 0,
                            analysis[AnalysisIndex::Tempo], analysis[AnalysisIndex::Zcr], analysis[AnalysisIndex::MeanSpectralCentroid], analysis[AnalysisIndex::StdDeviationSpectralCentroid], analysis[AnalysisIndex::MeanSpectralRolloff],
                            analysis[AnalysisIndex::StdDeviationSpectralRolloff], analysis[AnalysisIndex::MeanSpectralFlatness], analysis[AnalysisIndex::StdDeviationSpectralFlatness], analysis[AnalysisIndex::MeanLoudness], analysis[AnalysisIndex::StdDeviationLoudness],
                            analysis[AnalysisIndex::Chroma1], analysis[AnalysisIndex::Chroma2], analysis[AnalysisIndex::Chroma3], analysis[AnalysisIndex::Chroma4], analysis[AnalysisIndex::Chroma5],
                            analysis[AnalysisIndex::Chroma6], analysis[AnalysisIndex::Chroma7], analysis[AnalysisIndex::Chroma8], analysis[AnalysisIndex::Chroma9], analysis[AnalysisIndex::Chroma10]]) {
                        Ok(_) => { },
                        Err(e) => { log::error!("Failed to insert '{}' into database. {}", path, e); }
                    }
                } else {
                    match self.conn.execute("UPDATE Tracks SET Title=?, Artist=?, AlbumArtist=?, Album=?, Genre=?, Duration=?, Tempo=?, Zcr=?, MeanSpectralCentroid=?, StdDevSpectralCentroid=?, MeanSpectralRolloff=?, StdDevSpectralRolloff=?, MeanSpectralFlatness=?, StdDevSpectralFlatness=?, MeanLoudness=?, StdDevLoudness=?, Chroma1=?, Chroma2=?, Chroma3=?, Chroma4=?, Chroma5=?, Chroma6=?, Chroma7=?, Chroma8=?, Chroma9=?, Chroma10=? WHERE rowid=?);",
                            params![meta.title, meta.artist, meta.album_artist, meta.album, meta.genre, meta.duration,
                            analysis[AnalysisIndex::Tempo], analysis[AnalysisIndex::Zcr], analysis[AnalysisIndex::MeanSpectralCentroid], analysis[AnalysisIndex::StdDeviationSpectralCentroid], analysis[AnalysisIndex::MeanSpectralRolloff],
                            analysis[AnalysisIndex::StdDeviationSpectralRolloff], analysis[AnalysisIndex::MeanSpectralFlatness], analysis[AnalysisIndex::StdDeviationSpectralFlatness], analysis[AnalysisIndex::MeanLoudness], analysis[AnalysisIndex::StdDeviationLoudness],
                            analysis[AnalysisIndex::Chroma1], analysis[AnalysisIndex::Chroma2], analysis[AnalysisIndex::Chroma3], analysis[AnalysisIndex::Chroma4], analysis[AnalysisIndex::Chroma5],
                            analysis[AnalysisIndex::Chroma6], analysis[AnalysisIndex::Chroma7], analysis[AnalysisIndex::Chroma8], analysis[AnalysisIndex::Chroma9], analysis[AnalysisIndex::Chroma10], id]) {
                        Ok(_) => { },
                        Err(e) => { log::error!("Failed to update '{}' in database. {}", path, e); }
                    }
                }
            },
            Err(_) => { }
        }
    }

    pub fn remove_old(&self, mpath:&Path, dry_run:bool) {
        log::info!("Looking for non-existant tracks");
        let mut stmt = self.conn.prepare("SELECT File FROM Tracks;").unwrap();
        let track_iter = stmt.query_map([], |row| {
            Ok((row.get(0)?,))
        }).unwrap();
        let mut to_remove:Vec<String> = Vec::new();
        for tr in track_iter {
            let mut db_path:String = tr.unwrap().0;
            if cfg!(windows) {
                db_path = db_path.replace("/", "\\");
            }
            let path = mpath.join(PathBuf::from(db_path.clone()));

            if !path.exists() {
                to_remove.push(db_path);
            }
        }
        log::info!("Num non-existant tracks: {}", to_remove.len());
        if !dry_run {
            for t in to_remove {
                match self.conn.execute("DELETE FROM Tracks WHERE File = ?;", params![t]) {
                    Ok(_) => { },
                    Err(_) => { }
                }
            }
        }
    }

    pub fn get_track_count(&self) -> usize {
        let mut stmt = self.conn.prepare("SELECT COUNT(*) FROM Tracks;").unwrap();
        let track_iter = stmt.query_map([], |row| {
            Ok(row.get(0)?)
        }).unwrap();
        let mut count:usize = 0;
        for tr in track_iter {
            count = tr.unwrap();
            break;
        }
        count
    }

    pub fn update_tags(&self, mpath:&PathBuf) {
        let total = self.get_track_count();
        if total>0 {
            let pb = ProgressBar::new(total.try_into().unwrap());
            let style = ProgressStyle::default_bar()
                .template("[{elapsed_precise}] [{bar:25}] {percent:>3}% {pos:>6}/{len:6} {wide_msg}")
                .progress_chars("=> ");
            pb.set_style(style);
            let mut stmt = self.conn.prepare("SELECT rowid, File, Title, Artist, AlbumArtist, Album, Genre, Duration FROM Tracks ORDER BY File ASC;").unwrap();
            let track_iter = stmt.query_map([], |row| {
                Ok(FileMetadata {
                    rowid: row.get(0)?,
                    file: row.get(1)?,
                    title: row.get(2)?,
                    artist: row.get(3)?,
                    album_artist: row.get(4)?,
                    album: row.get(5)?,
                    genre: row.get(6)?,
                    duration: row.get(7)?,
                })
            }).unwrap();

            let mut updated = 0;
            for tr in track_iter {
                let dbtags = tr.unwrap();
                let dtags = Metadata{
                    title:dbtags.title.unwrap_or(String::new()),
                    artist:dbtags.artist.unwrap_or(String::new()),
                    album_artist:dbtags.album_artist.unwrap_or(String::new()),
                    album:dbtags.album.unwrap_or(String::new()),
                    genre:dbtags.genre.unwrap_or(String::new()),
                    duration:dbtags.duration
                };
                pb.set_message(format!("{}", dbtags.file));
                let path = String::from(mpath.join(&dbtags.file).to_string_lossy());
                let ftags = tags::read(&path);
                if ftags.title.is_empty() && ftags.artist.is_empty() && ftags.album_artist.is_empty() && ftags.album.is_empty() && ftags.genre.is_empty() {
                    log::error!("Failed to read tags of '{}'", dbtags.file);
                } else if ftags.duration!=dtags.duration || ftags.title!=dtags.title || ftags.artist!=dtags.artist || ftags.album_artist!=dtags.album_artist || ftags.album!=dtags.album || ftags.genre!=dtags.genre {
                    match self.conn.execute("UPDATE Tracks SET Title=?, Artist=?, AlbumArtist=?, Album=?, Genre=?, Duration=? WHERE rowid=?;",
                                            params![ftags.title, ftags.artist, ftags.album_artist, ftags.album, ftags.genre, ftags.duration, dbtags.rowid]) {
                        Ok(_) => { updated += 1; },
                        Err(e) => { log::error!("Failed to update tags of '{}'. {}", dbtags.file, e); }
                    }
                }
                pb.inc(1);
            }
            pb.finish_with_message(format!("{} Updated.", updated))
        }
    }

    pub fn clear_ignore(&self) {
        match self.conn.execute("UPDATE Tracks SET Ignore=0;", []) {
            Ok(_) => { },
            Err(e) => { log::error!("Failed clear Ignore column. {}", e); }
        }
    }

    pub fn set_ignore(&self, line:&str) {
        log::info!("Ignore: {}", line);
        if line.starts_with("SQL:") {
            let sql = &line[4..];
            match self.conn.execute(&format!("UPDATE Tracks Set Ignore=1 WHERE {}", sql), []) {
                Ok(_) => { },
                Err(e) => { log::error!("Failed set Ignore column for '{}'. {}", line, e); }
            }
        } else {
            match self.conn.execute(&format!("UPDATE Tracks SET Ignore=1 WHERE File LIKE \"{}%\"", line), []) {
                Ok(_) => { },
                Err(e) => { log::error!("Failed set Ignore column for '{}'. {}", line, e); }
            }
        }
    }
 }