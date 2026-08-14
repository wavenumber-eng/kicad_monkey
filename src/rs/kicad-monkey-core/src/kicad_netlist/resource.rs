use crate::{ProjectError, SourceBundleError, SourceBundleErrorKind};

pub(super) struct StringBudget {
    used: usize,
    maximum: usize,
}

impl StringBudget {
    pub(super) const fn new(maximum: usize) -> Self {
        Self { used: 0, maximum }
    }

    pub(super) fn reserve(&mut self, bytes: usize) -> Result<(), SourceBundleError> {
        self.used = self
            .used
            .checked_add(bytes)
            .ok_or_else(|| limit_error("KiCad netlist retained string bytes overflow"))?;
        if self.used > self.maximum {
            return Err(limit_error(
                "KiCad netlist retained string bytes exceed their limit",
            ));
        }
        Ok(())
    }

    pub(super) fn reserve_many(
        &mut self,
        values: impl IntoIterator<Item = usize>,
    ) -> Result<(), SourceBundleError> {
        for value in values {
            self.reserve(value)?;
        }
        Ok(())
    }
}

pub(super) fn check_count(
    count: usize,
    maximum: usize,
    message: &str,
) -> Result<(), SourceBundleError> {
    if count > maximum {
        Err(limit_error(message))
    } else {
        Ok(())
    }
}

pub(super) fn ensure_capacity(
    current: usize,
    maximum: usize,
    message: &str,
) -> Result<(), SourceBundleError> {
    if current >= maximum {
        Err(limit_error(message))
    } else {
        Ok(())
    }
}

pub(super) fn project_error(error: ProjectError) -> SourceBundleError {
    SourceBundleError::new(SourceBundleErrorKind::Project, None, error.to_string())
}

pub(super) fn limit_error(message: &str) -> SourceBundleError {
    SourceBundleError::new(SourceBundleErrorKind::ResourceLimit, None, message)
}

pub(super) fn schematic_error(message: &str) -> SourceBundleError {
    SourceBundleError::new(SourceBundleErrorKind::Schematic, None, message)
}
