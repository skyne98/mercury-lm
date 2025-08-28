use crate::models::{HFFile, HFModel, HFModelInfo};

pub fn hf_search_models(q: &str) -> anyhow::Result<Vec<String>> {
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

pub fn hf_fetch_files(model: &str) -> anyhow::Result<Vec<HFFile>> {
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
