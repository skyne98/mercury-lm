use crate::models::{DownloadEvent, DownloadKind};
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
    sync::mpsc,
};

pub fn spawn_model_download(url: String, dest: PathBuf, tx: mpsc::Sender<DownloadEvent>) {
    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::new();
        let r = client.get(&url).send();
        let mut resp = match r {
            Ok(r) => r,
            Err(e) => {
                let _ = tx.send(DownloadEvent::Error {
                    kind: DownloadKind::Model,
                    err: e.to_string(),
                });
                return;
            }
        };
        let total = resp.content_length();
        if let Some(p) = dest.parent() {
            let _ = fs::create_dir_all(p);
        }
        let mut file = match fs::File::create(&dest) {
            Ok(f) => f,
            Err(e) => {
                let _ = tx.send(DownloadEvent::Error {
                    kind: DownloadKind::Model,
                    err: e.to_string(),
                });
                return;
            }
        };
        let mut buf = [0u8; 64 * 1024];
        let mut downloaded: u64 = 0;
        loop {
            match resp.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if let Err(e) = file.write_all(&buf[..n]) {
                        let _ = tx.send(DownloadEvent::Error {
                            kind: DownloadKind::Model,
                            err: e.to_string(),
                        });
                        return;
                    }
                    downloaded += n as u64;
                    let _ = tx.send(DownloadEvent::Progress {
                        kind: DownloadKind::Model,
                        current: downloaded,
                        total,
                        stage: "download",
                    });
                }
                Err(e) => {
                    let _ = tx.send(DownloadEvent::Error {
                        kind: DownloadKind::Model,
                        err: e.to_string(),
                    });
                    return;
                }
            }
        }
        let _ = tx.send(DownloadEvent::Done {
            kind: DownloadKind::Model,
            dest: Some(dest),
        });
    });
}
