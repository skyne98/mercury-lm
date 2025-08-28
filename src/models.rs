use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq)]
pub enum Backend {
    Auto,
    Cpu,
    Cuda,
    Hip,
    Metal,
    Vulkan,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum DownloadKind {
    Runtime,
    Model,
}

#[derive(Debug)]
pub enum DownloadEvent {
    Progress {
        kind: DownloadKind,
        current: u64,
        total: Option<u64>,
        stage: &'static str,
    },
    Done {
        kind: DownloadKind,
        dest: Option<PathBuf>,
    },
    Error {
        kind: DownloadKind,
        err: String,
    },
}

#[derive(Debug)]
pub enum StreamEvent {
    Token(String),
    Done,
    Error(String),
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Msg {
    pub role: String,
    pub content: String,
}

#[derive(Deserialize)]
pub struct HFModel {
    pub id: String,
}

#[derive(Deserialize)]
pub struct HFModelInfo {
    pub siblings: Vec<HFFile>,
}

#[derive(Deserialize, Clone)]
pub struct HFFile {
    pub rfilename: String,
    pub size: Option<u64>,
}

#[derive(Clone)]
pub struct DownloadedModel {
    pub file_name: String,
    pub path: PathBuf,
    pub size: Option<u64>,
}

#[derive(Deserialize)]
pub struct GhAsset {
    pub name: String,
    pub browser_download_url: String,
}

#[derive(Deserialize)]
pub struct GhRelease {
    pub assets: Vec<GhAsset>,
}

#[derive(Serialize)]
pub struct ChatReq {
    pub model: String,
    pub messages: Vec<Msg>,
    pub stream: bool,
    pub temperature: f32,
    pub max_tokens: i32,
}
