use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
    sync::mpsc,
};

pub fn unzip(file: &PathBuf, dst: &PathBuf) -> anyhow::Result<()> {
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

pub fn unzip_with_progress(
    zip_file: &PathBuf,
    dst: &PathBuf,
    tx: &mpsc::Sender<crate::models::DownloadEvent>,
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
        let _ = tx.send(crate::models::DownloadEvent::Progress {
            kind: crate::models::DownloadKind::Runtime,
            current: (i as u64) + 1,
            total: Some(total),
            stage: "unpack",
        });
    }
    Ok(())
}
