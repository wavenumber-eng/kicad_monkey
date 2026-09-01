//! Shared metadata and resource accounting for typed Plotter-IR projection.

use crate::PlotterOperation;
use std::fmt;
use std::mem::size_of;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlotProjectionErrorKind {
    ResourceLimit,
    NumericRange,
    InvalidModel,
    ContractValidation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlotProjectionError {
    pub kind: PlotProjectionErrorKind,
    message: String,
}

impl PlotProjectionError {
    pub(crate) fn new(kind: PlotProjectionErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for PlotProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for PlotProjectionError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlotDocumentMetadata {
    pub document_id: String,
    pub source_path: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlotDocumentProjectionLimits {
    pub max_records: usize,
    pub max_operations: usize,
    pub max_points: usize,
    pub max_string_bytes: usize,
    pub max_nested_items: usize,
    pub max_materialized_bytes: usize,
}

impl Default for PlotDocumentProjectionLimits {
    fn default() -> Self {
        Self {
            max_records: 1_000_000,
            max_operations: 4_000_000,
            max_points: 16_000_000,
            max_string_bytes: 256 * 1024 * 1024,
            max_nested_items: 32_000_000,
            max_materialized_bytes: 2 * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ProjectionUsage {
    pub records: usize,
    pub operations: usize,
    pub points: usize,
    pub string_bytes: usize,
    pub nested_items: usize,
}

impl ProjectionUsage {
    pub(crate) fn add_string(&mut self, value: &str) -> Result<(), PlotProjectionError> {
        self.string_bytes = checked_add(self.string_bytes, value.len(), "string bytes")?;
        Ok(())
    }

    pub(crate) fn add_optional_string(
        &mut self,
        value: Option<&str>,
    ) -> Result<(), PlotProjectionError> {
        value.map_or(Ok(()), |value| self.add_string(value))
    }

    pub(crate) fn add_strings<'a>(
        &mut self,
        values: impl IntoIterator<Item = &'a String>,
    ) -> Result<(), PlotProjectionError> {
        for value in values {
            self.add_string(value)?;
            self.nested_items = checked_add(self.nested_items, 1, "nested items")?;
        }
        Ok(())
    }

    pub(crate) fn add_operation(
        &mut self,
        operation: &PlotterOperation,
    ) -> Result<(), PlotProjectionError> {
        self.operations = checked_add(self.operations, 1, "operations")?;
        self.points = checked_add(self.points, operation_points(operation)?, "points")?;
        self.nested_items = checked_add(
            self.nested_items,
            operation_nested_items(operation)?,
            "nested items",
        )?;
        for value in operation_strings(operation) {
            self.add_string(value)?;
        }
        Ok(())
    }

    pub(crate) fn enforce(
        self,
        limits: PlotDocumentProjectionLimits,
    ) -> Result<(), PlotProjectionError> {
        for (actual, maximum, label) in [
            (self.records, limits.max_records, "records"),
            (self.operations, limits.max_operations, "operations"),
            (self.points, limits.max_points, "points"),
            (self.string_bytes, limits.max_string_bytes, "string bytes"),
            (self.nested_items, limits.max_nested_items, "nested items"),
        ] {
            if actual > maximum {
                return Err(resource_error(format!(
                    "plot document projection {label} exceeds its limit"
                )));
            }
        }
        // This estimate intentionally overcharges the simultaneously retained
        // core and generated vectors plus their fixed-size payloads. Dynamic
        // strings and nested vectors are charged independently below.
        let materialized = 64_usize
            .checked_mul(1024)
            .and_then(|value| value.checked_add(self.records.checked_mul(4096)?))
            .and_then(|value| value.checked_add(self.operations.checked_mul(4096)?))
            .and_then(|value| value.checked_add(self.points.checked_mul(64)?))
            .and_then(|value| value.checked_add(self.string_bytes.checked_mul(4)?))
            .and_then(|value| value.checked_add(self.nested_items.checked_mul(512)?))
            .and_then(|value| {
                value.checked_add(
                    self.records
                        .checked_add(self.operations)?
                        .checked_mul(size_of::<usize>())?,
                )
            })
            .ok_or_else(|| resource_error("plot document materialized byte estimate overflowed"))?;
        if materialized > limits.max_materialized_bytes {
            return Err(resource_error(
                "plot document projection materialized bytes exceeds its limit",
            ));
        }
        Ok(())
    }
}

fn operation_points(operation: &PlotterOperation) -> Result<usize, PlotProjectionError> {
    let count = match operation {
        PlotterOperation::ThickSegment(_) | PlotterOperation::Rect(_) => 2,
        PlotterOperation::ArcThreePoint(_) => 3,
        PlotterOperation::Circle(_)
        | PlotterOperation::Text(_)
        | PlotterOperation::FlashPadCircle(_)
        | PlotterOperation::FlashPadOval(_)
        | PlotterOperation::FlashPadRect(_)
        | PlotterOperation::FlashPadRoundRect(_) => 1,
        PlotterOperation::BezierCurve(_) | PlotterOperation::FlashPadTrapez(_) => 4,
        PlotterOperation::PlotPoly(value) => value.points.len(),
        PlotterOperation::FlashPadCustom(value) => checked_sum(
            value.polygons.iter().map(Vec::len),
            "custom pad polygon points",
        )?
        .checked_add(1)
        .ok_or_else(|| resource_error("custom pad point count overflowed"))?,
    };
    Ok(count)
}

fn operation_nested_items(operation: &PlotterOperation) -> Result<usize, PlotProjectionError> {
    let count = match operation {
        PlotterOperation::PlotPoly(value) => value.points.len(),
        PlotterOperation::FlashPadCustom(value) => {
            let polygon_items = checked_sum(
                value.polygons.iter().map(|polygon| {
                    polygon
                        .len()
                        .checked_add(1)
                        .ok_or_else(|| resource_error("custom pad nested item count overflowed"))
                }),
                "custom pad nested items",
            )?;
            polygon_items
                .checked_add(value.polygon_widths_nm.as_ref().map_or(0, Vec::len))
                .and_then(|count| count.checked_add(value.layers.len()))
                .ok_or_else(|| resource_error("custom pad nested item count overflowed"))?
        }
        PlotterOperation::ThickSegment(value) => value.layers.len(),
        PlotterOperation::Circle(value) => value.layers.len(),
        PlotterOperation::FlashPadCircle(value) => value.layers.len(),
        PlotterOperation::FlashPadOval(value) => value.layers.len(),
        PlotterOperation::FlashPadRect(value) => value.layers.len(),
        PlotterOperation::FlashPadRoundRect(value) => value.layers.len(),
        PlotterOperation::FlashPadTrapez(value) => value.layers.len(),
        _ => 0,
    };
    Ok(count)
}

fn operation_strings(operation: &PlotterOperation) -> Vec<&str> {
    let mut values = Vec::new();
    match operation {
        PlotterOperation::ThickSegment(value) => {
            push_optional(&mut values, value.layer.as_deref());
            push_optional(&mut values, value.role.as_deref());
            values.extend(value.layers.iter().map(String::as_str));
        }
        PlotterOperation::ArcThreePoint(value) => {
            push_optional(&mut values, value.layer.as_deref());
            push_optional(&mut values, value.stroke_color.as_deref());
            push_optional(&mut values, value.fill_color.as_deref());
        }
        PlotterOperation::Circle(value) => {
            push_optional(&mut values, value.layer.as_deref());
            push_optional(&mut values, value.role.as_deref());
            push_optional(&mut values, value.stroke_color.as_deref());
            push_optional(&mut values, value.fill_color.as_deref());
            values.extend(value.layers.iter().map(String::as_str));
        }
        PlotterOperation::Rect(value) => {
            push_optional(&mut values, value.layer.as_deref());
            push_optional(&mut values, value.stroke_color.as_deref());
            push_optional(&mut values, value.fill_color.as_deref());
        }
        PlotterOperation::PlotPoly(value) => {
            push_optional(&mut values, value.layer.as_deref());
            push_optional(&mut values, value.stroke_color.as_deref());
            push_optional(&mut values, value.fill_color.as_deref());
        }
        PlotterOperation::BezierCurve(value) => {
            push_optional(&mut values, value.layer.as_deref());
            push_optional(&mut values, value.stroke_color.as_deref());
        }
        PlotterOperation::Text(value) => {
            values.extend([
                value.text.as_str(),
                value.color.as_str(),
                value.font_face.as_str(),
            ]);
            push_optional(&mut values, value.layer.as_deref());
        }
        PlotterOperation::FlashPadCircle(value) => {
            values.extend(value.layers.iter().map(String::as_str));
        }
        PlotterOperation::FlashPadOval(value) => {
            values.extend(value.layers.iter().map(String::as_str));
        }
        PlotterOperation::FlashPadRect(value) => {
            values.extend(value.layers.iter().map(String::as_str));
        }
        PlotterOperation::FlashPadRoundRect(value) => {
            values.extend(value.layers.iter().map(String::as_str));
        }
        PlotterOperation::FlashPadCustom(value) => {
            push_optional(&mut values, value.anchor_shape.as_deref());
            values.extend(value.layers.iter().map(String::as_str));
        }
        PlotterOperation::FlashPadTrapez(value) => {
            values.extend(value.layers.iter().map(String::as_str));
        }
    }
    values
}

fn push_optional<'a>(values: &mut Vec<&'a str>, value: Option<&'a str>) {
    if let Some(value) = value {
        values.push(value);
    }
}

fn checked_add(left: usize, right: usize, label: &str) -> Result<usize, PlotProjectionError> {
    left.checked_add(right)
        .ok_or_else(|| resource_error(format!("plot document projection {label} overflowed")))
}

fn checked_sum<I, T>(values: I, label: &str) -> Result<usize, PlotProjectionError>
where
    I: IntoIterator<Item = T>,
    T: IntoCheckedCount,
{
    values.into_iter().try_fold(0usize, |total, value| {
        let value = value.into_checked_count()?;
        total
            .checked_add(value)
            .ok_or_else(|| resource_error(format!("{label} overflowed")))
    })
}

trait IntoCheckedCount {
    fn into_checked_count(self) -> Result<usize, PlotProjectionError>;
}

impl IntoCheckedCount for usize {
    fn into_checked_count(self) -> Result<usize, PlotProjectionError> {
        Ok(self)
    }
}

impl IntoCheckedCount for Result<usize, PlotProjectionError> {
    fn into_checked_count(self) -> Result<usize, PlotProjectionError> {
        self
    }
}

fn resource_error(message: impl Into<String>) -> PlotProjectionError {
    PlotProjectionError::new(PlotProjectionErrorKind::ResourceLimit, message)
}
