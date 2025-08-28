use crate::models::{DownloadEvent, DownloadKind};
use crate::unzip::*;
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
    sync::mpsc,
};

pub fn spawn_runtime_download(
    url: String,
    zip_path: PathBuf,
    bin_dir: PathBuf,
    tx: mpsc::Sender<DownloadEvent>,
) {
    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::new();
        let resp = match client.get(&url).send() {
            Ok(r) => r,
            Err(e) => {
                let _ = tx.send(DownloadEvent::Error {
                    kind: DownloadKind::Runtime,
                    err: e.to_string(),
                });
                return;
            }
        };
        let total = resp.content_length();
        let mut reader = resp;
        let mut file = match fs::File::create(&zip_path) {
            Ok(f) => f,
            Err(e) => {
                let _ = tx.send(DownloadEvent::Error {
                    kind: DownloadKind::Runtime,
                    err: e.to_string(),
                });
                return;
            }
        };
        let mut buf = [0u8; 64 * 1024];
        let mut downloaded: u64 = 0;
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if let Err(e) = file.write_all(&buf[..n]) {
                        let _ = tx.send(DownloadEvent::Error {
                            kind: DownloadKind::Runtime,
                            err: e.to_string(),
                        });
                        return;
                    }
                    downloaded += n as u64;
                    let _ = tx.send(DownloadEvent::Progress {
                        kind: DownloadKind::Runtime,
                        current: downloaded,
                        total,
                        stage: "download",
                    });
                }
                Err(e) => {
                    let _ = tx.send(DownloadEvent::Error {
                        kind: DownloadKind::Runtime,
                        err: e.to_string(),
                    });
                    return;
                }
            }
        }
        // Unpack
        if !bin_dir.exists() {
            let _ = fs::create_dir_all(&bin_dir);
        }
        if let Err(e) = unzip_with_progress(&zip_path, &bin_dir, &tx) {
            let _ = tx.send(DownloadEvent::Error {
                kind: DownloadKind::Runtime,
                err: e.to_string(),
            });
            return;
        }
    });
}
