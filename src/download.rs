use crate::model_download::*;
use crate::models::{DownloadEvent, DownloadKind};
use crate::spawn::*;
use crate::unzip::*;
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
    sync::mpsc,
};

pub fn human_size(b: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let bf = b as f64;
    if bf >= GB {
        format!("{:.2} GB", bf / GB)
    } else if bf >= MB {
        format!("{:.2} MB", bf / MB)
    } else if bf >= KB {
        format!("{:.2} KB", bf / KB)
    } else {
        format!("{} B", b)
    }
}
