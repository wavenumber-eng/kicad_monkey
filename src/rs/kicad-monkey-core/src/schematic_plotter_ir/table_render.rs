//! Bounded schematic table projection using the canonical text-box renderer.

use super::annotation_render::render_text_box;
use super::*;
use crate::plotter_text_cache::PlotterTextCacheSession;

pub(super) fn append_table_records(
    source: &str,
    spans: &GraphicSpans,
    variables: &BTreeMap<String, String>,
    metrics: Option<&PlotterTextCacheSession<'_>>,
    limits: SchematicPlotLimits,
    budget: &mut PlotBudget,
    records: &mut Vec<SchematicPlotRecord>,
) -> Result<(), Error> {
    let mut total_cells = 0usize;
    let mut total_lines = 0usize;
    for span in &spans.tables {
        let form = parse_span(source, span, limits)?;
        let cell_values = child(&form, "cells").and_then(list).unwrap_or(&[]);
        let cells = || {
            cell_values.iter().skip(1).filter(|value| {
                list(value).and_then(|values| values.first()).and_then(text) == Some("table_cell")
            })
        };
        let cell_count = cells().count();
        total_cells = checked_limit(total_cells, cell_count, limits.max_table_cells)?;
        let mut operations = Vec::new();
        let mut points = 0usize;
        for cell in cells() {
            let remaining_lines = limits
                .max_table_cell_lines
                .checked_sub(total_lines)
                .ok_or_else(limit_error)?;
            let remaining_operations = budget
                .remaining_operations()
                .checked_sub(operations.len())
                .ok_or_else(limit_error)?;
            let rendered = render_text_box(
                cell,
                variables,
                metrics,
                limits,
                remaining_lines,
                remaining_operations,
                budget,
            )?;
            total_lines = total_lines
                .checked_add(rendered.line_count)
                .filter(|value| *value <= limits.max_table_cell_lines)
                .ok_or_else(limit_error)?;
            charge_outline_metadata(budget, &rendered.operations)?;
            checked_limit(
                operations.len(),
                rendered.operations.len(),
                budget.remaining_operations(),
            )?;
            checked_limit(points, rendered.points, budget.remaining_points())?;
            points = points
                .checked_add(rendered.points)
                .ok_or_else(limit_error)?;
            operations.extend(rendered.operations);
        }
        let uuid = child_string(&form, "uuid").unwrap_or_default();
        budget.charge_metadata(uuid.len().saturating_mul(2))?;
        budget.charge(1, operations.len(), points)?;
        records.push(SchematicPlotRecord::Table(SchematicTableRecord {
            uuid,
            cell_count,
            operations,
        }));
    }
    Ok(())
}

fn charge_outline_metadata(
    budget: &mut PlotBudget,
    operations: &[SchematicPlotOperation],
) -> Result<(), Error> {
    for operation in operations {
        let SchematicPlotOperation::Plotter(PlotterOperation::Rect(value)) = operation else {
            continue;
        };
        budget.charge_metadata(
            value
                .stroke_color
                .as_deref()
                .map_or(0, str::len)
                .saturating_add(value.fill_color.as_deref().map_or(0, str::len)),
        )?;
    }
    Ok(())
}
