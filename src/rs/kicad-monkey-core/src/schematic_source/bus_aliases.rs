use super::{carrier_form_spans, direct_scalars, limit_error, source_error};
use crate::schematic_bundle::SchematicBundleLimits;
use crate::sexpr_projection::FormSpan;
use crate::source_bundle::SourceBundleError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicBusAlias {
    pub name: String,
    pub members: Vec<String>,
}

pub(crate) fn parse_bus_aliases(
    source: &str,
    source_path: &str,
    spans: &[FormSpan],
    limits: SchematicBundleLimits,
) -> Result<Vec<SchematicBusAlias>, SourceBundleError> {
    let mut aliases = Vec::new();
    let mut member_count = 0_usize;
    for span in spans
        .iter()
        .filter(|span| span.depth == 1 && span.head.as_deref() == Some("bus_alias"))
    {
        if aliases.len() >= limits.max_bus_aliases_per_source {
            return Err(limit_error(
                source_path,
                "schematic bus alias count exceeds its limit",
            ));
        }
        let text = span
            .text(source)
            .map_err(|error| source_error(source_path, error.to_string()))?;
        let selected = carrier_form_spans(text, &["bus_alias", "members"], source_path, limits, 2)?;
        let root = selected
            .iter()
            .find(|selected| selected.depth == 0 && selected.head.as_deref() == Some("bus_alias"))
            .ok_or_else(|| source_error(source_path, "schematic bus alias root is missing"))?;
        let name = direct_scalars(text, root, 1, source_path, limits)?
            .into_iter()
            .next()
            .unwrap_or_default();
        let remaining = limits
            .max_bus_alias_members_per_source
            .saturating_sub(member_count);
        let members = selected
            .iter()
            .find(|selected| selected.depth == 1 && selected.head.as_deref() == Some("members"))
            .map(|members| direct_scalars(text, members, remaining, source_path, limits))
            .transpose()?
            .unwrap_or_default();
        member_count = member_count.checked_add(members.len()).ok_or_else(|| {
            limit_error(source_path, "schematic bus alias member count overflowed")
        })?;
        if member_count > limits.max_bus_alias_members_per_source {
            return Err(limit_error(
                source_path,
                "schematic bus alias member count exceeds its limit",
            ));
        }
        aliases.push(SchematicBusAlias { name, members });
    }
    Ok(aliases)
}
