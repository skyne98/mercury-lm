use crate::models::*;
use crate::download::*;
use crate::server::{find_server_bin, guess_backend, pick_asset_url, want_asset_name};
use crate::scan::*;
use crate::runtime::*;
use std::{
    fs,
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
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

    // New automatic management fields
    pub settings: Settings,
    pub available_runtimes: Vec<RuntimeInfo>,
    pub current_runtime: Option<RuntimeInfo>,
    pub server_status: ServerStatus,
    pub last_activity: Instant,
    pub show_settings: bool,
}

impl Default for App {
    fn default() -> Self {
        let dir = directories::ProjectDirs::from("dev", "mini", "llama-mini").unwrap();
        fs::create_dir_all(dir.data_dir()).ok();
        let model_dir = dir.data_dir().join("models");
        let _ = fs::create_dir_all(&model_dir);

        // Load settings
        let settings_path = dir.data_dir().join("settings.json");
        let settings = if settings_path.exists() {
            match fs::read_to_string(&settings_path) {
                Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
                Err(_) => Settings::default(),
            }
        } else {
            Settings::default()
        };

        let mut app = Self {
            backend: Backend::Auto,
            status: "Initializing...".into(),
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

            // Initialize new fields
            settings,
            available_runtimes: vec![],
            current_runtime: None,
            server_status: ServerStatus::Stopped,
            last_activity: Instant::now(),
            show_settings: false,
        };

        // Auto-detect runtimes
        app.detect_runtimes();

        // Auto-load default runtime if available
        if let Some(default_name) = &app.settings.default_runtime {
            if let Some(runtime) = app.available_runtimes.iter().find(|r| r.name == *default_name) {
                app.current_runtime = Some(runtime.clone());
                app.server_bin = Some(runtime.path.clone());
                app.status = format!("Runtime ready: {}", runtime.name);
            }
        } else if !app.available_runtimes.is_empty() {
            // Auto-select first available runtime
            let runtime = app.available_runtimes[0].clone();
            app.current_runtime = Some(runtime.clone());
            app.server_bin = Some(runtime.path.clone());
            app.status = format!("Runtime ready: {}", runtime.name);
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

impl App {
    /// Detect available runtimes in the runtime directory
    pub fn detect_runtimes(&mut self) {
        self.available_runtimes.clear();
        let bin_dir = self.runtime_dir.join("llama-bin");

        if bin_dir.exists() {
            if let Some(server_bin) = find_server_bin(&bin_dir) {
                let runtime = RuntimeInfo {
                    name: "Local Runtime".to_string(),
                    path: server_bin,
                    version: "Unknown".to_string(),
                    backend: Backend::Auto,
                };
                self.available_runtimes.push(runtime);
            }
        }
    }

    /// Save current settings to disk
    pub fn save_settings(&self) -> anyhow::Result<()> {
        let dir = directories::ProjectDirs::from("dev", "mini", "llama-mini").unwrap();
        let settings_path = dir.data_dir().join("settings.json");
        let content = serde_json::to_string_pretty(&self.settings)?;
        fs::write(settings_path, content)?;
        Ok(())
    }

    /// Auto-start server if needed
    pub fn ensure_server_running(&mut self) {
        if !self.server_ready && matches!(self.server_status, ServerStatus::Stopped) && self.settings.auto_start_server {
            if self.server_child.is_none() {
                self.server_status = ServerStatus::Starting;
                self.status = "Auto-starting server...".into();
                if let Err(e) = crate::server::start_server(self) {
                    self.server_status = ServerStatus::Error(format!("Auto-start failed: {e}"));
                    self.status = format!("Auto-start err: {e}");
                }
            }
        }
    }

    /// Auto-stop server after inactivity
    pub fn check_server_timeout(&mut self) {
        if self.settings.auto_stop_server && self.server_ready {
            let timeout = Duration::from_secs(self.settings.server_timeout_minutes as u64 * 60);
            if self.last_activity.elapsed() > timeout {
                if let Some(mut child) = self.server_child.take() {
                    let _ = child.kill();
                    self.server_ready = false;
                    self.server_status = ServerStatus::Stopped;
                    self.status = "Server auto-stopped (inactive)".into();
                }
            }
        }
    }

    /// Update last activity timestamp
    pub fn mark_activity(&mut self) {
        self.last_activity = Instant::now();
    }
}
