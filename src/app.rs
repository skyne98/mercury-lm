use crate::models::*;
use crate::download::*;
use crate::server::{find_server_bin, guess_backend, pick_asset_url, want_asset_name};
use crate::scan::*;
use crate::runtime::*;
use std::{
    fs,
    path::PathBuf,
    sync::mpsc,
};

pub struct App {
    pub backend: Backend,
    pub status: String,
    pub runtime_dir: PathBuf,
    pub model_dir: PathBuf,
    pub server_bin: Option<PathBuf>,
    pub server_child: Option<std::process::Child>,
    pub server_url: String,
    pub model_repo: String,
    pub model_file: String,
    pub model_path: Option<PathBuf>,
    pub msgs: Vec<Msg>,
    pub input: String,
    pub editing: Option<usize>,
    pub rx: Option<mpsc::Receiver<StreamEvent>>,
    pub dl_rx: Option<mpsc::Receiver<DownloadEvent>>,
    pub runtime_progress: Option<(u64, Option<u64>, String)>,
    pub model_progress: Option<(u64, Option<u64>, String)>,
    pub search_query: String,
    pub search_results: Vec<String>,
    pub selected_model: Option<String>,
    pub files_for_selected: Vec<HFFile>,
    pub search_status: String,
    pub server_log: Vec<String>,
    pub log_rx: Option<mpsc::Receiver<String>>,
    pub server_ready: bool,
    pub loaded_model: Option<String>,
    pub served_model_id: Option<String>,
    pub downloaded: Vec<DownloadedModel>,
}

impl Default for App {
    fn default() -> Self {
        let dir = directories::ProjectDirs::from("dev", "mini", "llama-mini").unwrap();
        fs::create_dir_all(dir.data_dir()).ok();
        let model_dir = dir.data_dir().join("models");
        let _ = fs::create_dir_all(&model_dir);
        let mut app = Self {
            backend: Backend::Auto,
            status: "Idle".into(),
            runtime_dir: dir.data_dir().to_path_buf(),
            model_dir,
            server_bin: None,
            server_child: None,
            server_url: "http://127.0.0.1:8080".into(),
            model_repo: "TheBloke/Mistral-7B-Instruct-v0.2-GGUF".into(),
            model_file: "mistral-7b-instruct-v0.2.Q4_K_M.gguf".into(),
            model_path: None,
            msgs: vec![],
            input: String::new(),
            editing: None,
            rx: None,
            dl_rx: None,
            runtime_progress: None,
            model_progress: None,
            search_query: String::new(),
            search_results: vec![],
            selected_model: None,
            files_for_selected: vec![],
            search_status: String::new(),
            server_log: Vec::new(),
            log_rx: None,
            server_ready: false,
            loaded_model: None,
            served_model_id: None,
            downloaded: vec![],
        };
        let bin_dir = app.runtime_dir.join("llama-bin");
        if bin_dir.exists() {
            app.server_bin = find_server_bin(&bin_dir);
            if app.server_bin.is_some() {
                app.status = "Runtime ready".into();
            }
        }
        // Ensure model directory exists before scanning
        if !app.model_dir.exists() {
            let _ = fs::create_dir_all(&app.model_dir);
        }
        scan_downloaded_models(&mut app);
        let model_count = app.downloaded.len();
        if model_count > 0 {
            app.status = format!("Found {} downloaded model(s)", model_count);
        }
        app
    }
}
