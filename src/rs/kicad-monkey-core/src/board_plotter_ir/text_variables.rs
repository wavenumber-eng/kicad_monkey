//! Case-expanded board text variables with bounded substitution.

use super::text_limit_error;
use crate::sexpr::Error;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Case-expanded `${NAME}` variables mirroring Python
/// `kicad_text_variables.normalize_text_variables`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BoardTextVariables {
    by_name: BTreeMap<String, Arc<str>>,
}

impl BoardTextVariables {
    /// Python `_add_variable` per entry: exact, lowercase, and uppercase keys
    /// are written in that order; later entries overwrite earlier ones.
    pub fn from_entries<N, V>(entries: impl IntoIterator<Item = (N, V)>) -> Self
    where
        N: AsRef<str>,
        V: AsRef<str>,
    {
        let mut variables = Self::default();
        for (name, value) in entries {
            variables.insert(name.as_ref(), value.as_ref());
        }
        variables
    }

    pub(super) fn insert(&mut self, name: &str, value: &str) {
        if name.is_empty() {
            return;
        }
        let value: Arc<str> = Arc::from(value);
        self.by_name.insert(name.to_owned(), Arc::clone(&value));
        self.by_name.insert(name.to_lowercase(), Arc::clone(&value));
        self.by_name.insert(name.to_uppercase(), value);
    }

    /// Resolve variables while checking the byte ceiling before every append.
    /// Unterminated placeholder tails are copied once, never repeatedly rescanned.
    pub fn substitute_bounded(&self, text: &str, max_bytes: usize) -> Result<String, Error> {
        self.substitute_bounded_with_local(text, &[], max_bytes)
    }

    /// Resolve with a constant-size carrier-local overlay. This avoids
    /// cloning the complete board/project map for every table cell.
    pub(super) fn substitute_bounded_with_local(
        &self,
        text: &str,
        local: &[(&str, &str)],
        max_bytes: usize,
    ) -> Result<String, Error> {
        if !text.contains("${") {
            if text.len() > max_bytes {
                return Err(text_limit_error());
            }
            return Ok(text.to_owned());
        }
        let mut result = String::with_capacity(text.len().min(max_bytes));
        let push = |result: &mut String, value: &str| -> Result<(), Error> {
            result
                .len()
                .checked_add(value.len())
                .filter(|length| *length <= max_bytes)
                .ok_or_else(text_limit_error)?;
            result.push_str(value);
            Ok(())
        };
        let mut rest = text;
        while let Some(start) = rest.find("${") {
            let after = &rest[start + 2..];
            match after.find('}') {
                Some(end) if end > 0 => {
                    push(&mut result, &rest[..start])?;
                    let name = &after[..end];
                    match local
                        .iter()
                        .rev()
                        .find_map(|(key, value)| (*key == name).then_some(*value))
                    {
                        Some(value) => push(&mut result, value)?,
                        None if self.by_name.contains_key(name) => {
                            push(&mut result, &self.by_name[name])?
                        }
                        None => push(&mut result, &rest[start..start + end + 3])?,
                    }
                    rest = &after[end + 1..];
                }
                Some(_) => {
                    push(&mut result, &rest[..start + 3])?;
                    rest = &rest[start + 3..];
                }
                None => {
                    push(&mut result, rest)?;
                    rest = "";
                    break;
                }
            }
        }
        push(&mut result, rest)?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sexpr::ErrorKind;

    #[test]
    fn substitution_is_linear_on_unterminated_placeholders_and_bounded_before_append() {
        let variables = BoardTextVariables::from_entries([("A", "expanded")]);
        let malformed = "${".repeat(100_000);
        assert_eq!(
            variables
                .substitute_bounded(&malformed, malformed.len())
                .expect("unterminated text remains literal"),
            malformed
        );
        let amplified = "${A}${A}${A}";
        assert_eq!(
            variables.substitute_bounded(amplified, 24).unwrap(),
            "expandedexpandedexpanded"
        );
        assert_eq!(
            variables
                .substitute_bounded(amplified, 23)
                .unwrap_err()
                .kind,
            ErrorKind::ResourceLimit
        );
    }
}
