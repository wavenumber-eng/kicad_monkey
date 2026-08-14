use crate::{SchematicSubpartSettings, SourceBundleError, SourceBundleErrorKind, SourceFile};
use serde::Deserialize;

#[derive(Deserialize, Default)]
#[serde(default)]
struct ProjectDocument {
    schematic: ProjectSchematicSettings,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ProjectSchematicSettings {
    subpart_first_id: Option<i64>,
    subpart_id_separator: Option<i64>,
}

pub(crate) fn project_subpart_settings(
    project: Option<&SourceFile>,
) -> Result<SchematicSubpartSettings, SourceBundleError> {
    let Some(project) = project else {
        return Ok(SchematicSubpartSettings::default());
    };
    let document = serde_json::from_slice::<ProjectDocument>(project.bytes()).map_err(|error| {
        SourceBundleError::new(
            SourceBundleErrorKind::Project,
            Some(project.path()),
            format!("project schematic settings are invalid: {error}"),
        )
    })?;
    Ok(SchematicSubpartSettings {
        first_id: setting_codepoint(
            document.schematic.subpart_first_id,
            SchematicSubpartSettings::default().first_id,
            project,
            "subpart_first_id",
        )?,
        separator: setting_codepoint(
            document.schematic.subpart_id_separator,
            SchematicSubpartSettings::default().separator,
            project,
            "subpart_id_separator",
        )?,
    })
}

fn setting_codepoint(
    value: Option<i64>,
    fallback: u32,
    project: &SourceFile,
    field: &str,
) -> Result<u32, SourceBundleError> {
    value.map_or(Ok(fallback), |value| {
        u32::try_from(value).map_err(|_| {
            SourceBundleError::new(
                SourceBundleErrorKind::Project,
                Some(project.path()),
                format!("schematic.{field} is outside the uint32 range"),
            )
        })
    })
}
