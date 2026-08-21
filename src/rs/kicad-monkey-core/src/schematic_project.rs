use crate::{
    ProjectDocument, ProjectLimits, ProjectView, SchematicSubpartSettings, SourceBundleError,
    SourceBundleErrorKind, SourceFile,
};

pub(crate) fn project_subpart_settings(
    project: Option<&SourceFile>,
) -> Result<SchematicSubpartSettings, SourceBundleError> {
    let Some(project) = project else {
        return Ok(SchematicSubpartSettings::default());
    };
    let document = ProjectDocument::from_reader(
        project.bytes(),
        ProjectLimits {
            max_source_bytes: project.bytes().len(),
            max_output_bytes: project.bytes().len(),
            ..ProjectLimits::default()
        },
    )
    .map_err(|error| {
        SourceBundleError::new(
            SourceBundleErrorKind::Project,
            Some(project.path()),
            format!("project schematic settings are invalid: {error}"),
        )
    })?;
    let view = document.view();
    if let Some(schematic) = view.raw().get("schematic")
        && !schematic.is_object()
    {
        return Err(SourceBundleError::new(
            SourceBundleErrorKind::Project,
            Some(project.path()),
            "project schematic settings container must be a JSON object",
        ));
    }
    Ok(SchematicSubpartSettings {
        first_id: setting_codepoint(
            setting(&view, "schematic.subpart_first_id", project)?,
            SchematicSubpartSettings::default().first_id,
            project,
            "subpart_first_id",
        )?,
        separator: setting_codepoint(
            setting(&view, "schematic.subpart_id_separator", project)?,
            SchematicSubpartSettings::default().separator,
            project,
            "subpart_id_separator",
        )?,
    })
}

fn setting(
    view: &ProjectView<'_>,
    path: &str,
    project: &SourceFile,
) -> Result<Option<i64>, SourceBundleError> {
    let Some(value) = view.get_path(path) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    value.as_i64().map(Some).ok_or_else(|| {
        SourceBundleError::new(
            SourceBundleErrorKind::Project,
            Some(project.path()),
            format!("{path} must be an integer"),
        )
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
