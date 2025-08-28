use eframe::egui::{self, Align, Layout};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    sync::mpsc,
    time::Duration,
};

#[derive(Clone, Copy, PartialEq)]
enum Backend {
    Auto,
    Cpu,
    Cuda,
    Hip,
    Metal,
    Vulkan,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum DownloadKind {
    Runtime,
    Model,
}

#[derive(Debug)]
enum DownloadEvent {
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

// Streaming events for robust control of chat lifecycle
enum StreamEvent {
    Token(String),
    Done,
    Error(String),
}

#[derive(Default, Clone, Serialize, Deserialize)]
struct Msg {
    role: String,
    content: String,
}

// Minimal HF search structures
#[derive(Deserialize)]
struct HFModel {
    id: String,
}
#[derive(Deserialize)]
struct HFModelInfo {
    siblings: Vec<HFFile>,
}
#[derive(Deserialize, Clone)]
struct HFFile {
    rfilename: String,
    size: Option<u64>,
}

#[derive(Clone)]
struct DownloadedModel {
    file_name: String,
    path: PathBuf,
    size: Option<u64>,
}

struct App {
    // runtime + model
    backend: Backend,
    status: String,
    runtime_dir: PathBuf,
    model_dir: PathBuf,
    server_bin: Option<PathBuf>,
    server_child: Option<std::process::Child>,
    server_url: String,
    model_repo: String,
    model_file: String,
    model_path: Option<PathBuf>,
    // chat
    msgs: Vec<Msg>,
    input: String,
    editing: Option<usize>,
    // streaming bridge
    rx: Option<mpsc::Receiver<StreamEvent>>, // was Receiver<String>
    // downloads
    dl_rx: Option<mpsc::Receiver<DownloadEvent>>,
    runtime_progress: Option<(u64, Option<u64>, String)>,
    model_progress: Option<(u64, Option<u64>, String)>,
    // search
    search_query: String,
    search_results: Vec<String>,
    selected_model: Option<String>,
    files_for_selected: Vec<HFFile>,
    search_status: String,
    // server logs & readiness
    server_log: Vec<String>,
    log_rx: Option<mpsc::Receiver<String>>,
    server_ready: bool,
    loaded_model: Option<String>,
    served_model_id: Option<String>,
    // downloaded models list
    downloaded: Vec<DownloadedModel>,
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
        // Auto-detect existing runtime on startup
        let bin_dir = app.runtime_dir.join("llama-bin");
        if bin_dir.exists() {
            app.server_bin = find_server_bin(&bin_dir);
            if app.server_bin.is_some() {
                app.status = "Runtime ready".into();
            }
        }
        scan_downloaded_models(&mut app);
        app
    }
}

fn scan_downloaded_models(app: &mut App) {
    let mut list = Vec::new();
    if let Ok(read) = fs::read_dir(&app.model_dir) {
        for ent in read.flatten() {
            let p = ent.path();
            if p.is_file() {
                if let Some(name) = p
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
                {
                    if name.to_lowercase().ends_with(".gguf") {
                        let size = ent.metadata().ok().map(|m| m.len());
                        list.push(DownloadedModel {
                            file_name: name,
                            path: p.clone(),
                            size,
                        });
                    }
                }
            }
        }
    }
    // sort newest first (by modified) fallback to name
    list.sort_by(|a, b| {
        let ma = fs::metadata(&a.path).ok().and_then(|m| m.modified().ok());
        let mb = fs::metadata(&b.path).ok().and_then(|m| m.modified().ok());
        mb.cmp(&ma).then_with(|| a.file_name.cmp(&b.file_name))
    });
    app.downloaded = list;
}

fn has(cmd: &str) -> bool {
    which::which(cmd).is_ok()
}

fn guess_backend() -> Backend {
    #[cfg(target_os = "macos")]
    {
        return Backend::Metal;
    }
    #[cfg(target_os = "windows")]
    {
        if has("nvidia-smi") {
            return Backend::Cuda;
        }
        return Backend::Cpu;
    }
    #[cfg(target_os = "linux")]
    {
        if has("nvidia-smi") {
            Backend::Cuda
        } else if has("rocm-smi") {
            Backend::Hip
        } else {
            Backend::Vulkan
        }
    }
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}
#[derive(Deserialize)]
struct GhRelease {
    assets: Vec<GhAsset>,
}

fn want_asset_name(b: Backend) -> &'static [&'static str] {
    // prefer most-accelerated match; fall back to CPU for the OS
    #[cfg(target_os = "windows")]
    {
        match b {
            Backend::Cuda => &["win-cuda", "cudart-llama"], // both patterns exist
            Backend::Hip => &["win-hip-radeon"],
            _ => &["win-cpu-x64"],
        }
    }
    #[cfg(target_os = "macos")]
    {
        &["macos-arm64", "macos-x64"]
    }
    #[cfg(target_os = "linux")]
    {
        match b {
            Backend::Vulkan => &["ubuntu-vulkan-x64"],
            Backend::Cuda => &["cuda-ubuntu", "ubuntu-cuda", "cuda"], // safety net
            _ => &["ubuntu-x64"],
        }
    }
}

fn pick_asset_url(rel: &GhRelease, pats: &[&str]) -> Option<String> {
    for p in pats {
        if let Some(a) = rel.assets.iter().find(|a| a.name.contains(p)) {
            return Some(a.browser_download_url.clone());
        }
    }
    None
}

fn unzip(file: &PathBuf, dst: &PathBuf) -> anyhow::Result<()> {
    let f = fs::File::open(file)?;
    let mut z = zip::ZipArchive::new(f)?;
    for i in 0..z.len() {
        let mut e = z.by_index(i)?;
        let out = dst.join(e.name());
        if e.name().ends_with('/') {
            fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(p) = out.parent() {
            fs::create_dir_all(p)?;
        }
        let mut w = fs::File::create(&out)?;
        std::io::copy(&mut e, &mut w)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&out, fs::Permissions::from_mode(0o755)).ok();
        }
    }
    Ok(())
}

fn unzip_with_progress(
    zip_file: &PathBuf,
    dst: &PathBuf,
    tx: &mpsc::Sender<DownloadEvent>,
) -> anyhow::Result<()> {
    let f = fs::File::open(zip_file)?;
    let mut z = zip::ZipArchive::new(f)?;
    let total = z.len() as u64;
    for i in 0..z.len() {
        let mut e = z.by_index(i)?;
        let out = dst.join(e.name());
        if e.name().ends_with('/') {
            fs::create_dir_all(&out)?;
        } else {
            if let Some(p) = out.parent() {
                fs::create_dir_all(p)?;
            }
            let mut w = fs::File::create(&out)?;
            std::io::copy(&mut e, &mut w)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&out, fs::Permissions::from_mode(0o755)).ok();
            }
        }
        let _ = tx.send(DownloadEvent::Progress {
            kind: DownloadKind::Runtime,
            current: (i as u64) + 1,
            total: Some(total),
            stage: "unpack",
        });
    }
    Ok(())
}

fn human_size(b: u64) -> String {
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

fn spawn_runtime_download(
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
        let _ = tx.send(DownloadEvent::Done {
            kind: DownloadKind::Runtime,
            dest: None,
        });
    });
}

fn spawn_model_download(url: String, dest: PathBuf, tx: mpsc::Sender<DownloadEvent>) {
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

fn ensure_runtime(app: &mut App) -> anyhow::Result<()> {
    app.status = "Checking runtime…".into();
    let be = if app.backend == Backend::Auto {
        guess_backend()
    } else {
        app.backend
    };
    let rel: GhRelease = reqwest::blocking::Client::new()
        .get("https://api.github.com/repos/ggml-org/llama.cpp/releases/latest")
        .header("User-Agent", "llama-mini")
        .send()?
        .json()?;
    let url = pick_asset_url(&rel, want_asset_name(be))
        .ok_or_else(|| anyhow::anyhow!("No matching asset for backend"))?;
    let zip_path = app.runtime_dir.join("llama-runtime.zip");
    let bin_dir = app.runtime_dir.join("llama-bin");
    if !bin_dir.exists() {
        // spawn non-blocking download
        let (tx, rx) = mpsc::channel();
        app.dl_rx = Some(rx);
        app.runtime_progress = Some((0, None, "download".into()));
        spawn_runtime_download(url, zip_path, bin_dir, tx);
        app.status = "Downloading runtime…".into();
    } else {
        app.server_bin = find_server_bin(&bin_dir);
        if app.server_bin.is_none() {
            anyhow::bail!("llama-server not found")
        }
        app.status = "Runtime ready".into();
    }
    Ok(())
}

fn start_model_download(app: &mut App) -> anyhow::Result<()> {
    let repo = app.model_repo.trim().to_string();
    let file = app.model_file.trim().to_string();
    if repo.is_empty() || file.is_empty() {
        anyhow::bail!("Set model repo and file")
    }
    let dest = app.model_dir.join(&file);
    // If exists, don't redownload
    if dest.exists() {
        app.model_path = Some(dest.clone());
        app.status = "Model already downloaded".into();
        scan_downloaded_models(app);
        return Ok(());
    }
    let url = format!("https://huggingface.co/{repo}/resolve/main/{file}?download=true");
    let (tx, rx) = mpsc::channel();
    app.dl_rx = Some(rx);
    app.model_progress = Some((0, None, "download".into()));
    spawn_model_download(url, dest, tx);
    app.status = "Downloading model…".into();
    Ok(())
}

fn hf_search_models(q: &str) -> anyhow::Result<Vec<String>> {
    if q.trim().is_empty() {
        return Ok(vec![]);
    }
    let url = format!(
        "https://huggingface.co/api/models?search={}&limit=20&pipeline_tag=text-generation",
        urlencoding::encode(q)
    );
    let res: Vec<HFModel> = reqwest::blocking::get(url)?.json()?;
    Ok(res.into_iter().map(|m| m.id).collect())
}

fn hf_fetch_files(model: &str) -> anyhow::Result<Vec<HFFile>> {
    let url = format!(
        "https://huggingface.co/api/models/{}?expand[]=siblings",
        model
    );
    let info: HFModelInfo = reqwest::blocking::get(url)?.json()?;
    Ok(info
        .siblings
        .into_iter()
        .filter(|f| f.rfilename.to_lowercase().ends_with(".gguf"))
        .collect())
}

fn find_server_bin(dir: &PathBuf) -> Option<PathBuf> {
    let names = ["llama-server", "server", "llama-server.exe", "server.exe"];
    for n in names {
        let p = dir.join(n);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn ensure_model(app: &mut App) -> anyhow::Result<()> {
    app.status = "Downloading model…".into();
    let api = hf_hub::api::sync::Api::new()?;
    let path = api.model(app.model_repo.clone()).get(&app.model_file)?;
    app.model_path = Some(path);
    Ok(())
}

fn start_server(app: &mut App) -> anyhow::Result<()> {
    let exe = app
        .server_bin
        .clone()
        .ok_or_else(|| anyhow::anyhow!("no server"))?;
    let mdl = app
        .model_path
        .clone()
        .ok_or_else(|| anyhow::anyhow!("no model"))?;
    let ngl = match app.backend {
        Backend::Cuda | Backend::Hip | Backend::Metal | Backend::Vulkan | Backend::Auto => "99",
        _ => "0",
    };
    app.loaded_model = Some(mdl.to_string_lossy().to_string());
    app.served_model_id = None;
    app.server_ready = false;
    app.status = "Server starting…".into();

    let mut child = Command::new(exe)
        .args([
            "-m",
            mdl.to_string_lossy().as_ref(),
            "-ngl",
            ngl,
            "--port",
            "8080",
            "--host",
            "127.0.0.1",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Pipe logs to UI via channel
    let (tx, rx) = mpsc::channel();
    app.log_rx = Some(rx);
    if let Some(stdout) = child.stdout.take() {
        let txo = tx.clone();
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if let Ok(l) = line {
                    let _ = txo.send(format!("[OUT] {l}"));
                } else {
                    break;
                }
            }
        });
    }
    if let Some(stderr) = child.stderr.take() {
        let txe = tx.clone();
        std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                if let Ok(l) = line {
                    let _ = txe.send(format!("[ERR] {l}"));
                } else {
                    break;
                }
            }
        });
    }

    // Poll /v1/models until ready; capture model id
    let url = app.server_url.clone();
    let tx_ready = tx.clone();
    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::new();
        for _ in 0..60 {
            // ~30s
            match client.get(format!("{}/v1/models", url)).send() {
                Ok(resp) if resp.status().is_success() => {
                    let id = resp.json::<serde_json::Value>().ok().and_then(|v| {
                        v["data"]
                            .get(0)
                            .and_then(|x| x["id"].as_str())
                            .map(|s| s.to_string())
                    });
                    let _ = tx_ready.send("[READY] llama-server is ready".into());
                    if let Some(mid) = id {
                        let _ = tx_ready.send(format!("[MODEL] {mid}"));
                    }
                    break;
                }
                _ => std::thread::sleep(Duration::from_millis(500)),
            }
        }
    });

    app.server_child = Some(child);
    Ok(())
}

#[derive(Serialize)]
struct ChatReq {
    model: String,
    messages: Vec<Msg>,
    stream: bool,
    temperature: f32,
    max_tokens: i32,
}

fn stream_chat(url: &str, model: String, msgs: Vec<Msg>, tx: mpsc::Sender<StreamEvent>) {
    let body = serde_json::to_string(&ChatReq {
        model,
        messages: msgs,
        stream: true,
        temperature: 0.7,
        max_tokens: 1024,
    })
    .unwrap();
    let url = url.to_string();
    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::new();
        let post = client
            .post(format!("{url}/v1/chat/completions"))
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .body(body)
            .send();
        let mut resp = match post {
            Ok(r) => r,
            Err(e) => {
                let _ = tx.send(StreamEvent::Error(format!("request failed: {e}")));
                let _ = tx.send(StreamEvent::Done);
                return;
            }
        };
        let mut buf = String::new();
        let mut chunk = [0u8; 8192];
        loop {
            match resp.read(&mut chunk) {
                Ok(0) => {
                    let _ = tx.send(StreamEvent::Done);
                    break;
                }
                Ok(n) => {
                    buf.push_str(&String::from_utf8_lossy(&chunk[..n]));
                    while let Some(idx) = buf.find("\n\n") {
                        let mut line = buf[..idx].trim().to_string();
                        buf = buf[(idx + 2)..].to_string();
                        if let Some(rest) = line.strip_prefix("data:") {
                            line = rest.trim().into();
                        }
                        if line == "[DONE]" {
                            let _ = tx.send(StreamEvent::Done);
                            return;
                        }
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                            if let Some(t) = v["choices"][0]["delta"]["content"].as_str() {
                                if tx.send(StreamEvent::Token(t.into())).is_err() {
                                    return;
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(StreamEvent::Error(format!("read failed: {e}")));
                    let _ = tx.send(StreamEvent::Done);
                    break;
                }
            }
        }
    });
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // streaming updates (drain and clear on Done)
        if let Some(rx0) = self.rx.take() {
            let mut rx = rx0;
            let mut done = false;
            while let Ok(ev) = rx.try_recv() {
                match ev {
                    StreamEvent::Token(t) => {
                        if let Some(last) = self.msgs.last_mut() {
                            if last.role == "assistant" {
                                last.content.push_str(&t);
                            }
                        }
                    }
                    StreamEvent::Error(e) => {
                        self.status = format!("Chat err: {e}");
                    }
                    StreamEvent::Done => {
                        done = true;
                    }
                }
                ctx.request_repaint();
            }
            if !done {
                self.rx = Some(rx);
            } else {
                self.status = "Idle".into();
            }
        }
        // drain download events without borrowing self.dl_rx immutably
        if let Some(rx) = self.dl_rx.take() {
            let mut rx = rx; // take ownership
            let mut events = Vec::new();
            while let Ok(ev) = rx.try_recv() {
                events.push(ev);
            }
            // restore receiver
            self.dl_rx = Some(rx);
            for ev in events {
                match ev {
                    DownloadEvent::Done {
                        kind: DownloadKind::Model,
                        dest,
                    } => {
                        if let Some(p) = dest {
                            self.model_path = Some(p);
                        }
                        self.model_progress = None;
                        self.status = "Model ready".into();
                        scan_downloaded_models(self);
                    }
                    DownloadEvent::Done {
                        kind: DownloadKind::Runtime,
                        ..
                    } => {
                        let bin_dir = self.runtime_dir.join("llama-bin");
                        self.server_bin = find_server_bin(&bin_dir);
                        self.runtime_progress = None;
                        self.status = "Runtime ready".into();
                    }
                    DownloadEvent::Progress {
                        kind: DownloadKind::Runtime,
                        current,
                        total,
                        stage,
                    } => {
                        self.runtime_progress = Some((current, total, stage.to_string()));
                        self.status = format!(
                            "Runtime {stage}: {} / {}",
                            human_size(current),
                            total.map(human_size).unwrap_or_else(|| "?".into())
                        );
                    }
                    DownloadEvent::Progress {
                        kind: DownloadKind::Model,
                        current,
                        total,
                        stage,
                    } => {
                        self.model_progress = Some((current, total, stage.to_string()));
                        self.status = format!(
                            "Model {stage}: {} / {}",
                            human_size(current),
                            total.map(human_size).unwrap_or_else(|| "?".into())
                        );
                    }
                    DownloadEvent::Error {
                        kind: DownloadKind::Runtime,
                        err,
                    } => {
                        self.runtime_progress = None;
                        self.status = format!("Runtime err: {err}");
                    }
                    DownloadEvent::Error {
                        kind: DownloadKind::Model,
                        err,
                    } => {
                        self.model_progress = None;
                        self.status = format!("Model err: {err}");
                    }
                }
                ctx.request_repaint();
            }
        }

        // Drain server logs
        if let Some(lrx) = &self.log_rx {
            while let Ok(line) = lrx.try_recv() {
                if line.starts_with("[READY]") {
                    self.server_ready = true;
                    self.status = "Server ready".into();
                }
                if let Some(rest) = line.strip_prefix("[MODEL] ") {
                    self.served_model_id = Some(rest.to_string());
                }
                self.server_log.push(line);
                if self.server_log.len() > 2000 {
                    let drop = self.server_log.len() - 2000;
                    self.server_log.drain(0..drop);
                }
                ctx.request_repaint();
            }
        }

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Backend:");
                for (b, label) in [
                    (Backend::Auto, "Auto"),
                    (Backend::Cpu, "CPU"),
                    (Backend::Cuda, "CUDA"),
                    (Backend::Hip, "HIP"),
                    (Backend::Metal, "Metal"),
                    (Backend::Vulkan, "Vulkan"),
                ] {
                    ui.selectable_value(&mut self.backend, b, label);
                }
                if ui.button("Get runtime").clicked() {
                    let _ = ensure_runtime(self);
                }
                if let Some((cur, tot, stage)) = &self.runtime_progress {
                    let frac = tot.map(|t| *cur as f32 / t as f32).unwrap_or(0.0);
                    ui.add(
                        egui::ProgressBar::new(frac).text(format!("{stage} {}", human_size(*cur))),
                    );
                }
                ui.label(&self.status);
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Model repo:");
                ui.text_edit_singleline(&mut self.model_repo);
                ui.label("File:");
                ui.text_edit_singleline(&mut self.model_file);
                if ui.button("Download model").clicked() {
                    let _ = start_model_download(self);
                }
                if let Some((cur, tot, stage)) = &self.model_progress {
                    let frac = tot.map(|t| *cur as f32 / t as f32).unwrap_or(0.0);
                    ui.add(
                        egui::ProgressBar::new(frac).text(format!("{stage} {}", human_size(*cur))),
                    );
                }
            });
            ui.horizontal(|ui| {
                ui.label("Search HF:");
                ui.text_edit_singleline(&mut self.search_query);
                if ui.button("Search").clicked() {
                    match hf_search_models(&self.search_query) {
                        Ok(list) => {
                            self.search_results = list;
                            self.search_status = String::new();
                        }
                        Err(e) => {
                            self.search_status = format!("Search err: {e}");
                            self.search_results.clear();
                        }
                    }
                }
                if !self.search_status.is_empty() {
                    ui.label(&self.search_status);
                }
            });
            if !self.search_results.is_empty() {
                egui::ScrollArea::vertical()
                    .max_height(120.0)
                    .show(ui, |ui| {
                        for id in self.search_results.clone() {
                            ui.horizontal(|ui| {
                                ui.label(&id);
                                if ui.button("Open").clicked() {
                                    match hf_fetch_files(&id) {
                                        Ok(files) => {
                                            self.selected_model = Some(id.clone());
                                            self.files_for_selected = files;
                                        }
                                        Err(e) => {
                                            self.search_status = format!("Files err: {e}");
                                        }
                                    }
                                }
                            });
                        }
                    });
            }
            ui.separator();
            ui.collapsing("Downloaded models", |ui| {
                if self.downloaded.is_empty() {
                    ui.label("No models downloaded yet.");
                }
                egui::ScrollArea::vertical()
                    .max_height(160.0)
                    .show(ui, |ui| {
                        // clone to avoid borrow issues when mutating self inside
                        let items = self.downloaded.clone();
                        for item in items {
                            ui.group(|ui| {
                                ui.horizontal(|ui| {
                                    let size_txt = item.size.map(human_size).unwrap_or("?".into());
                                    ui.label(format!("{}  ({})", item.file_name, size_txt));
                                    if ui.button("Use").clicked() {
                                        self.model_file = item.file_name.clone();
                                        self.model_repo = "(local)".into();
                                        self.model_path = Some(item.path.clone());
                                        self.status = "Selected local model".into();
                                    }
                                    if ui.button("Delete").clicked() {
                                        let _ = fs::remove_file(&item.path);
                                        scan_downloaded_models(self);
                                    }
                                    if ui.button("Reveal").clicked() {
                                        // best-effort: open folder
                                        #[cfg(target_os = "windows")]
                                        {
                                            let _ = Command::new("explorer")
                                                .arg(item.path.parent().unwrap_or(&self.model_dir))
                                                .spawn();
                                        }
                                        #[cfg(target_os = "macos")]
                                        {
                                            let _ = Command::new("open")
                                                .arg(item.path.parent().unwrap_or(&self.model_dir))
                                                .spawn();
                                        }
                                        #[cfg(target_os = "linux")]
                                        {
                                            let _ = Command::new("xdg-open")
                                                .arg(item.path.parent().unwrap_or(&self.model_dir))
                                                .spawn();
                                        }
                                    }
                                });
                            });
                        }
                    });
            });
            if let Some(model_id) = self.selected_model.clone() {
                let files = self.files_for_selected.clone();
                ui.collapsing(format!("{model_id} files"), |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(160.0)
                        .show(ui, |ui| {
                            for f in files.clone() {
                                let size_txt = f.size.map(human_size).unwrap_or("?".into());
                                ui.horizontal(|ui| {
                                    ui.label(format!("{} ({})", f.rfilename, size_txt));
                                    if ui.button("Download").clicked() {
                                        self.model_repo = model_id.clone();
                                        self.model_file = f.rfilename.clone();
                                        let _ = start_model_download(self);
                                    }
                                });
                            }
                        });
                });
            }
            if let Some(m) = &self.loaded_model {
                ui.label(format!("Loaded model path: {m}"));
            }
            if let Some(mid) = &self.served_model_id {
                ui.label(format!("Server model id: {mid}"));
            }
            ui.horizontal(|ui| {
                if self.server_ready {
                    ui.colored_label(egui::Color32::GREEN, "● Ready");
                } else {
                    ui.colored_label(egui::Color32::YELLOW, "● Not ready");
                }
                if ui.button("Start server").clicked() {
                    if self.server_child.is_none() {
                        if let Err(e) = start_server(self) {
                            self.status = format!("Start err: {e}");
                        } else {
                            self.status = "Server starting…".into();
                        }
                    }
                }
                if ui.button("Stop").clicked() {
                    if let Some(mut c) = self.server_child.take() {
                        let _ = c.kill();
                        self.server_ready = false;
                    }
                }
            });
            ui.collapsing("Server logs", |ui| {
                egui::ScrollArea::vertical()
                    .max_height(200.0)
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for l in &self.server_log {
                            ui.monospace(l);
                        }
                    });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.with_layout(Layout::top_down(Align::Min), |ui| {
                let mut pending_truncate: Option<usize> = None;
                for (i, m) in self.msgs.iter_mut().enumerate() {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(if m.role == "user" { "You" } else { "Assistant" });
                            if ui.button("Edit").clicked() {
                                self.editing = Some(i);
                            }
                            if ui.button("Restart from here").clicked() {
                                pending_truncate = Some(i + 1);
                            }
                        });
                        if self.editing == Some(i) {
                            ui.text_edit_multiline(&mut m.content);
                            if ui.button("Apply").clicked() {
                                self.editing = None;
                            }
                        } else {
                            ui.label(&m.content);
                        }
                    });
                }
                if let Some(t) = pending_truncate {
                    self.msgs.truncate(t);
                }
                ui.separator();
                ui.text_edit_multiline(&mut self.input);
                ui.horizontal(|ui| {
                    let sending = self.rx.is_some();
                    if ui
                        .add_enabled(!sending, egui::Button::new("Send"))
                        .clicked()
                    {
                        if !self.server_ready {
                            self.status = "Server not ready yet".into();
                        }
                        // Prepare conversation: if last is assistant or empty, start a new user turn; if last is user, append to it
                        let input_text = self.input.trim().to_string();
                        if !input_text.is_empty() {
                            match self.msgs.last_mut() {
                                Some(last) if last.role == "user" => {
                                    if !last.content.is_empty() {
                                        last.content.push_str("\n\n");
                                    }
                                    last.content.push_str(&input_text);
                                }
                                _ => {
                                    self.msgs.push(Msg {
                                        role: "user".into(),
                                        content: input_text.clone(),
                                    });
                                }
                            }
                            // assistant stub for streaming
                            self.msgs.push(Msg {
                                role: "assistant".into(),
                                content: String::new(),
                            });
                            let (tx, rx) = mpsc::channel::<StreamEvent>();
                            self.rx = Some(rx);
                            let url = self.server_url.clone();
                            let msgs = self.msgs.clone();
                            let model = self
                                .served_model_id
                                .clone()
                                .unwrap_or_else(|| "local".into());
                            stream_chat(&url, model, msgs, tx);
                            self.input.clear();
                        }
                    }
                    if sending {
                        if ui.button("Cancel").clicked() {
                            // Drop receiver; sender thread will stop when it sees send errors
                            self.rx = None;
                            self.status = "Canceled".into();
                        }
                        ui.label("Generating…");
                    } else {
                        ui.label("Streaming; edit any message to branch.");
                    }
                });
            });
        });
    }
}

fn main() -> eframe::Result<()> {
    let native_opts = eframe::NativeOptions::default();
    eframe::run_native(
        "llama-mini",
        native_opts,
        Box::new(|_cc| Ok(Box::new(App::default()))),
    )
}
