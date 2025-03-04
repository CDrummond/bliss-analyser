/**
 * Analyse music with Bliss
 *
 * Copyright (c) 2022-2025 Craig Drummond <craig.p.drummond@gmail.com>
 * GPLv3 license.
 *
 **/

use std::fs::File;
use std::io::BufReader;
use std::process;
use substring::Substring;
use ureq;

fn fail(msg: &str) {
    log::error!("{}", msg);
    process::exit(-1);
}

pub fn stop_mixer(lms_host: &String, json_port: u16) {
    let stop_req = "{\"id\":1, \"method\":\"slim.request\",\"params\":[\"\",[\"blissmixer\",\"stop\"]]}";

    log::info!("Asking plugin to stop mixer");
    let req = ureq::post(&format!("http://{}:{}/jsonrpc.js", lms_host, json_port)).send_string(&stop_req);
    if let Err(e) = req {
        log::error!("Failed to ask plugin to stop mixer. {}", e);
    }
}

pub fn upload_db(db_path: &String, lms_host: &String, json_port: u16) {
    // First tell LMS to restart the mixer in upload mode
    let start_req = "{\"id\":1, \"method\":\"slim.request\",\"params\":[\"\",[\"blissmixer\",\"start-upload\"]]}";
    let mut port: u16 = 0;

    log::info!("Requesting LMS plugin to allow uploads");

    match ureq::post(&format!("http://{}:{}/jsonrpc.js", lms_host, json_port)).send_string(&start_req) {
        Ok(resp) => match resp.into_string() {
            Ok(text) => match text.find("\"port\":") {
                Some(s) => {
                    let txt = text.to_string().substring(s + 7, text.len()).to_string();
                    match txt.find("}") {
                        Some(e) => {
                            let p = txt.substring(0, e);
                            let test = p.parse::<u16>();
                            match test {
                                Ok(val) => { port = val; }
                                Err(_) => { fail("Could not parse resp (cast)"); }
                            }
                        }
                        None => { fail("Could not parse resp (closing)"); }
                    }
                }
                None => { fail("Could not parse resp (no port)"); }
            }
            Err(_) => fail("No text?"),
        }
        Err(e) => { fail(&format!("Failed to ask LMS plugin to allow upload. {}", e)); }
    }

    if port == 0 {
        fail("Invalid port");
    }

    // Now we have port number, do the actual upload...
    log::info!("Uploading {}", db_path);
    match File::open(db_path) {
        Ok(file) => match file.metadata() {
            Ok(meta) => {
                let buffered_reader = BufReader::new(file);
                log::info!("Length: {}", meta.len());
                match ureq::put(&format!("http://{}:{}/upload", lms_host, port))
                    .set("Content-Length", &meta.len().to_string())
                    .set("Content-Type", "application/octet-stream")
                    .send(buffered_reader) {
                    Ok(_) => {
                        log::info!("Database uploaded");
                        stop_mixer(lms_host, json_port);
                    }
                    Err(e) => { fail(&format!("Failed to upload database. {}", e)); }
                }
            }
            Err(e) => { fail(&format!("Failed to open database. {}", e)); }
        }
        Err(e) => { fail(&format!("Failed to open database. {}", e)); }
    }
}
