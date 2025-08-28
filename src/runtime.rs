use crate::models::*;
use crate::download::*;
use crate::server::{find_server_bin, guess_backend, pick_asset_url, want_asset_name};
use crate::spawn::*;
use std::{
    fs,
    path::PathBuf,
    sync::mpsc,
};

pub fn ensure_runtime(app: &mut crate::app::App) -> anyhow::Result<()> {
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

pub fn start_model_download(app: &mut crate::app::App) -> anyhow::Result<()> {
    let repo = app.model_repo.trim().to_string();
    let file = app.model_file.trim().to_string();
    if repo.is_empty() || file.is_empty() {
        anyhow::bail!("Set model repo and file")
    }
    let dest = app.model_dir.join(&file);
    if dest.exists() {
        app.model_path = Some(dest.clone());
        app.status = "Model already downloaded".into();
        crate::scan::scan_downloaded_models(app);
        return Ok(());
    }
    let url = format!("https://huggingface.co/{repo}/resolve/main/{file}?download=true");
    let (tx, rx) = mpsc::channel();
    app.dl_rx = Some(rx);
    app.model_progress = Some((0, None, "download".into()));
    crate::model_download::spawn_model_download(url, dest, tx);
    app.status = "Downloading model…".into();
    Ok(())
}
