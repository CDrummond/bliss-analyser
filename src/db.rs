/**
 * Analyse music with Bliss
 *
 * Copyright (c) 2022 Craig Drummond <craig.p.drummond@gmail.com>
 * GPLv3 license.
 *
 **/

use bliss_audio::{Analysis, AnalysisIndex};
use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};
use std::process;
use crate::tags;

pub struct FileMetadata {
    pub rowid:usize,
    pub file:String,
    pub title:String,
    pub artist:String,
    pub album:String,
    pub genre:String,
    pub duration:u32
}

pub struct Metadata {
    pub title:String,
    pub artist:String,
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
                    match self.conn.execute("INSERT INTO Tracks (File, Title, Artist, Album, Genre, Duration, Ignore, Tempo, Zcr, MeanSpectralCentroid, StdDevSpectralCentroid, MeanSpectralRolloff, StdDevSpectralRolloff, MeanSpectralFlatness, StdDevSpectralFlatness, MeanLoudness, StdDevLoudness, Chroma1, Chroma2, Chroma3, Chroma4, Chroma5, Chroma6, Chroma7, Chroma8, Chroma9, Chroma10) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);",
                            params![db_path, meta.title, meta.artist, meta.album, meta.genre, meta.duration, 0,
                            analysis[AnalysisIndex::Tempo], analysis[AnalysisIndex::Zcr], analysis[AnalysisIndex::MeanSpectralCentroid], analysis[AnalysisIndex::StdDeviationSpectralCentroid], analysis[AnalysisIndex::MeanSpectralRolloff],
                            analysis[AnalysisIndex::StdDeviationSpectralRolloff], analysis[AnalysisIndex::MeanSpectralFlatness], analysis[AnalysisIndex::StdDeviationSpectralFlatness], analysis[AnalysisIndex::MeanLoudness], analysis[AnalysisIndex::StdDeviationLoudness],
                            analysis[AnalysisIndex::Chroma1], analysis[AnalysisIndex::Chroma2], analysis[AnalysisIndex::Chroma3], analysis[AnalysisIndex::Chroma4], analysis[AnalysisIndex::Chroma5],
                            analysis[AnalysisIndex::Chroma6], analysis[AnalysisIndex::Chroma7], analysis[AnalysisIndex::Chroma8], analysis[AnalysisIndex::Chroma9], analysis[AnalysisIndex::Chroma10]]) {
                        Ok(_) => { },
                        Err(e) => { log::error!("Failed to insert '{}' into database. {}", path, e); }
                    }
                } else {
                    match self.conn.execute("UPDATE Tracks SET Title=?, Artist=?, Album=?, Genre=?, Duration=?, Tempo=?, Zcr=?, MeanSpectralCentroid=?, StdDevSpectralCentroid=?, MeanSpectralRolloff=?, StdDevSpectralRolloff=?, MeanSpectralFlatness=?, StdDevSpectralFlatness=?, MeanLoudness=?, StdDevLoudness=?, Chroma1=?, Chroma2=?, Chroma3=?, Chroma4=?, Chroma5=?, Chroma6=?, Chroma7=?, Chroma8=?, Chroma9=?, Chroma10=? WHERE rowid=?);",
                            params![meta.title, meta.artist, meta.album, meta.genre, meta.duration,
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
        log::info!("Num old tracks: {}", to_remove.len());
        if !dry_run {
            for t in to_remove {
                match self.conn.execute("DELETE FROM Tracks WHERE File = ?;", params![t]) {
                    Ok(_) => { },
                    Err(_) => { }
                }
            }
        }
    }

    pub fn update_tags(&self, mpath:&PathBuf) {
        let mut stmt = self.conn.prepare("SELECT rowid, File, Title, Artist, Album, Genre, Duration FROM Tracks;").unwrap();
        let track_iter = stmt.query_map([], |row| {
            Ok(FileMetadata {
                rowid: row.get(0)?,
                file: row.get(1)?,
                title: row.get(2)?,
                artist: row.get(3)?,
                album: row.get(4)?,
                genre: row.get(5)?,
                duration: row.get(6)?,
            })
        }).unwrap();

        for tr in track_iter {
            let dtags = tr.unwrap();
            let path = String::from(mpath.join(&dtags.file).to_string_lossy());
            let ftags = tags::read(&path);
            if ftags.duration!=dtags.duration || ftags.title!=dtags.title || ftags.artist!=dtags.artist || ftags.album!=dtags.album || ftags.genre!=dtags.genre {
                match self.conn.execute("UPDATE Tracks SET Title=?, Artist=?, Album=?, Genre=?, Duration=? WHERE rowid=?);",
                                        params![ftags.title, ftags.artist, ftags.album, ftags.genre, ftags.duration, dtags.rowid]) {
                    Ok(_) => { },
                    Err(e) => { log::error!("Failed to update tags of '{}'. {}", dtags.file, e); }
                }
            }
        }
    }
 }
