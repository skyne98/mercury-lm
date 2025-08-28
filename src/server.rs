use crate::models::*;
use crate::stream::*;
use std::{
    io::{BufRead, Read},
    path::PathBuf,
    sync::mpsc,
};

pub fn has(cmd: &str) -> bool {
    which::which(cmd).is_ok()
}

pub fn guess_backend() -> Backend {
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

pub fn want_asset_name(b: Backend) -> &'static [&'static str] {
    #[cfg(target_os = "windows")]
    {
        match b {
            Backend::Cuda => &["win-cuda", "cudart-llama"],
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
            Backend::Cuda => &["cuda-ubuntu", "ubuntu-cuda", "cuda"],
            _ => &["ubuntu-x64"],
        }
    }
}

pub fn pick_asset_url(rel: &GhRelease, pats: &[&str]) -> Option<String> {
    for p in pats {
        if let Some(a) = rel.assets.iter().find(|a| a.name.contains(p)) {
            return Some(a.browser_download_url.clone());
        }
    }
    None
}

pub fn find_server_bin(dir: &PathBuf) -> Option<PathBuf> {
    let names = ["llama-server", "server", "llama-server.exe", "server.exe"];
    for n in names {
        let p = dir.join(n);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

pub fn start_server(app: &mut crate::app::App) -> anyhow::Result<()> {
    let exe = app
        .server_bin
        .clone()
        .ok_or_else(|| anyhow::anyhow!("no server"))?;
    let mdl = app
        .model_path
        .clone()
        .ok_or_else(|| anyhow::anyhow!("no model"))?;
    let ngl = match app.backend {
        crate::models::Backend::Cuda | crate::models::Backend::Hip | crate::models::Backend::Metal | crate::models::Backend::Vulkan | crate::models::Backend::Auto => "99",
        _ => "0",
    };
    app.loaded_model = Some(mdl.to_string_lossy().to_string());
    app.served_model_id = None;
    app.server_ready = false;
    app.status = "Server starting…".into();

    let mut child = std::process::Command::new(exe)
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
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let (tx, rx) = std::sync::mpsc::channel();
    app.log_rx = Some(rx);
    if let Some(stdout) = child.stdout.take() {
        let txo = tx.clone();
        std::thread::spawn(move || {
            let reader = std::io::BufReader::new(stdout);
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
            let reader = std::io::BufReader::new(stderr);
            for line in reader.lines() {
                if let Ok(l) = line {
                    let _ = txe.send(format!("[ERR] {l}"));
                } else {
                    break;
                }
            }
        });
    }

    let url = app.server_url.clone();
    let tx_ready = tx.clone();
    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::new();
        for _ in 0..60 {
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
                _ => std::thread::sleep(std::time::Duration::from_millis(500)),
            }
        }
    });

    app.server_child = Some(child);
    Ok(())
}
