//! Embedded-only model resolution over public, owner-scoped Monkey read APIs.
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::design::sha256_hex;
use kicad_monkey_core::{
    EmbeddedDataPresence, EmbeddedDecodeLimits, EmbeddedFileOwner, Error, PcbEmbeddedFile,
    PcbModelReference, PcbView,
};

#[derive(Debug)]
pub struct EmbeddedModel {
    pub reference: String,
    pub attachment: PcbModelReference,
    pub sha256: String,
    pub step: Arc<[u8]>,
}

#[derive(Debug)]
pub struct ModelWarning {
    pub reference: String,
    pub model: String,
    pub reason: String,
}

impl std::fmt::Display for ModelWarning {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}: model {}: {}",
            self.reference, self.model, self.reason
        )
    }
}

#[derive(Debug, Default)]
pub struct EmbeddedModels {
    pub instances: Vec<EmbeddedModel>,
    pub warnings: Vec<ModelWarning>,
}

type ResourceKey = (Option<usize>, String);

#[derive(Clone)]
struct ModelData {
    sha256: String,
    step: Arc<[u8]>,
}

/// Enumerate every enabled attachment. This function never opens model paths.
pub fn read_embedded_models(view: &PcbView<'_>) -> Result<EmbeddedModels, Error> {
    let references = view
        .footprints()
        .map(|value| value.map(|footprint| footprint.reference))
        .collect::<Result<Vec<_>, _>>()?;
    let files = view.all_embedded_files().collect::<Result<Vec<_>, _>>()?;
    let mut resources = BTreeMap::new();
    for file in &files {
        let owner = match file.owner {
            EmbeddedFileOwner::Board => None,
            EmbeddedFileOwner::EmbeddedFootprint { footprint_index } => Some(footprint_index),
            EmbeddedFileOwner::StandaloneFootprint => continue,
        };
        resources.entry((owner, file.name.clone())).or_insert(file);
    }
    let mut result = EmbeddedModels::default();
    let mut decoded = BTreeMap::<usize, Result<ModelData, String>>::new();
    let mut contents = BTreeMap::<String, Arc<[u8]>>::new();
    for attachment in view.models() {
        let attachment = attachment?;
        if attachment.hidden == Some(true) {
            continue;
        }
        let reference = references
            .get(attachment.footprint_index)
            .and_then(Clone::clone)
            .unwrap_or_else(|| format!("footprint-{}", attachment.footprint_index));
        let bytes = model_bytes(view, &attachment, &resources, &mut decoded);
        match bytes {
            Ok(ModelData { sha256, step }) => {
                let step = Arc::clone(contents.entry(sha256.clone()).or_insert(step));
                result.instances.push(EmbeddedModel {
                    reference,
                    attachment,
                    sha256,
                    step,
                });
            }
            Err(reason) => result.warnings.push(ModelWarning {
                reference,
                model: attachment.path,
                reason,
            }),
        }
    }
    Ok(result)
}

fn model_bytes(
    view: &PcbView<'_>,
    model: &PcbModelReference,
    resources: &BTreeMap<ResourceKey, &PcbEmbeddedFile>,
    decoded: &mut BTreeMap<usize, Result<ModelData, String>>,
) -> Result<ModelData, String> {
    let name = model.path.strip_prefix("kicad-embed://").ok_or_else(|| {
        "external reference skipped; only embedded models are supported".to_owned()
    })?;
    let extension = name.rsplit_once('.').map_or("", |(_, extension)| extension);
    if !matches!(extension.to_ascii_lowercase().as_str(), "step" | "stp") {
        return Err(format!(
            "unsupported embedded model extension .{extension}; expected .step/.stp"
        ));
    }
    let local = resources.get(&(Some(model.footprint_index), name.to_owned()));
    let resource = local
        .filter(|file| file.data_presence != EmbeddedDataPresence::Absent)
        .or_else(|| resources.get(&(None, name.to_owned())))
        .ok_or_else(|| "embedded payload not found in footprint or board scope".to_owned())?;
    decoded
        .entry(resource.source_range.start)
        .or_insert_with(|| {
            let data = view
                .decode_embedded_file(resource, EmbeddedDecodeLimits::default())
                .map_err(|error| error.to_string())?
                .filter(|bytes| !bytes.is_empty())
                .ok_or_else(|| "embedded payload is absent or empty".to_owned())?;
            Ok(ModelData {
                sha256: sha256_hex(&data),
                step: Arc::from(data),
            })
        })
        .clone()
}
