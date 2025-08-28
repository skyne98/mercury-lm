use crate::models::DownloadedModel;
use std::fs;

pub fn scan_downloaded_models(app: &mut crate::app::App) {
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
    list.sort_by(|a, b| {
        let ma = fs::metadata(&a.path).ok().and_then(|m| m.modified().ok());
        let mb = fs::metadata(&b.path).ok().and_then(|m| m.modified().ok());
        mb.cmp(&ma).then_with(|| a.file_name.cmp(&b.file_name))
    });
    app.downloaded = list;
}
