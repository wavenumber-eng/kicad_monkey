use crate::{SourceBundleError, SourceBundleErrorKind};
use std::collections::{BTreeMap, HashMap};

pub(super) struct ExpansionWorkBudget {
    used: usize,
    maximum: usize,
}

impl ExpansionWorkBudget {
    pub(super) const fn new(maximum: usize) -> Self {
        Self { used: 0, maximum }
    }

    fn preflight(&self, bytes: usize) -> Result<(), SourceBundleError> {
        if self
            .used
            .checked_add(bytes)
            .is_none_or(|total| total > self.maximum)
        {
            Err(limit_error("variable expansion work exceeds its limit"))
        } else {
            Ok(())
        }
    }

    fn reserve(&mut self, bytes: usize) -> Result<(), SourceBundleError> {
        self.preflight(bytes)?;
        self.used += bytes;
        Ok(())
    }

    pub(super) fn fold(&mut self, value: &str) -> Result<String, SourceBundleError> {
        let folded_bytes = value.chars().try_fold(0usize, |length, character| {
            character.to_lowercase().try_fold(length, |length, folded| {
                length
                    .checked_add(folded.len_utf8())
                    .ok_or_else(|| limit_error("case-folded variable name size overflows"))
            })
        })?;
        let work_bytes = value
            .len()
            .checked_add(folded_bytes)
            .ok_or_else(|| limit_error("case-folded variable name work overflows"))?;
        self.preflight(work_bytes)?;
        let mut folded = String::with_capacity(folded_bytes);
        folded.extend(value.chars().flat_map(char::to_lowercase));
        self.reserve(work_bytes)?;
        Ok(folded)
    }
}

pub(super) struct VariableResolver<'a, 'b> {
    fields: HashMap<String, &'a str>,
    project: &'a HashMap<String, String>,
    max_output_bytes: usize,
    work: &'b mut ExpansionWorkBudget,
}

impl<'a, 'b> VariableResolver<'a, 'b> {
    pub(super) fn new(
        fields: &'a BTreeMap<String, String>,
        project: &'a HashMap<String, String>,
        max_output_bytes: usize,
        work: &'b mut ExpansionWorkBudget,
    ) -> Result<Self, SourceBundleError> {
        let mut index = HashMap::with_capacity(fields.len());
        for (name, value) in fields {
            let folded = work.fold(name)?;
            index.insert(folded, value.as_str());
        }
        Ok(Self {
            fields: index,
            project,
            max_output_bytes,
            work,
        })
    }

    pub(super) fn expand_blank(
        &mut self,
        value: &str,
        skip: &str,
    ) -> Result<String, SourceBundleError> {
        self.expand(if value == "~" { "" } else { value }, skip)
    }

    pub(super) fn expand(&mut self, value: &str, skip: &str) -> Result<String, SourceBundleError> {
        if value.len() > self.max_output_bytes {
            return Err(limit_error("expanded string exceeds its byte limit"));
        }
        self.work.reserve(value.len())?;
        let mut result = value.to_owned();
        let skip = self.work.fold(skip)?;
        for _ in 0..10 {
            let next = self.expand_once(&result, &skip)?;
            if next == result {
                break;
            }
            result = next;
        }
        Ok(result)
    }

    fn expand_once(&mut self, value: &str, skip: &str) -> Result<String, SourceBundleError> {
        let mut output = BoundedText::new(self.max_output_bytes);
        let mut remaining = value;
        while let Some(start) = remaining.find("${") {
            output.push(&remaining[..start], self.work)?;
            let Some(end) = remaining[start + 2..].find('}') else {
                output.push(&remaining[start..], self.work)?;
                return Ok(output.finish());
            };
            let close = start + 2 + end;
            let name = remaining[start + 2..close].trim();
            let replacement = lookup(&self.fields, self.project, name, skip, self.work)?;
            output.push(replacement.unwrap_or(&remaining[start..=close]), self.work)?;
            remaining = &remaining[close + 1..];
        }
        output.push(remaining, self.work)?;
        Ok(output.finish())
    }
}

fn lookup<'a>(
    fields: &'a HashMap<String, &'a str>,
    project: &'a HashMap<String, String>,
    name: &str,
    skip: &str,
    work: &mut ExpansionWorkBudget,
) -> Result<Option<&'a str>, SourceBundleError> {
    let folded = work.fold(name)?;
    if folded == skip {
        return Ok(None);
    }
    Ok(fields
        .get(&folded)
        .copied()
        .map(blank)
        .or_else(|| project.get(&folded).map(String::as_str)))
}

struct BoundedText {
    text: String,
    maximum: usize,
}

impl BoundedText {
    fn new(maximum: usize) -> Self {
        Self {
            text: String::new(),
            maximum,
        }
    }

    fn push(
        &mut self,
        value: &str,
        work: &mut ExpansionWorkBudget,
    ) -> Result<(), SourceBundleError> {
        if self
            .text
            .len()
            .checked_add(value.len())
            .is_none_or(|length| length > self.maximum)
        {
            return Err(limit_error("expanded string exceeds its byte limit"));
        }
        work.reserve(value.len())?;
        self.text.push_str(value);
        Ok(())
    }

    fn finish(self) -> String {
        self.text
    }
}

fn blank(value: &str) -> &str {
    if value == "~" { "" } else { value }
}

fn limit_error(message: &str) -> SourceBundleError {
    SourceBundleError::new(SourceBundleErrorKind::ResourceLimit, None, message)
}

#[cfg(test)]
mod tests {
    use super::{ExpansionWorkBudget, VariableResolver};
    use crate::SourceBundleErrorKind;
    use std::collections::{BTreeMap, HashMap};

    #[test]
    fn recursive_expansion_accepts_exact_output_and_rejects_one_under() {
        let fields = BTreeMap::from([("GROW".to_owned(), "${GROW}${GROW}".to_owned())]);
        let project = HashMap::new();
        let mut exact_work = ExpansionWorkBudget::new(1_000_000);
        let mut exact =
            VariableResolver::new(&fields, &project, 7_168, &mut exact_work).expect("resolver");
        assert_eq!(
            exact.expand("${GROW}", "Value").expect("exact limit").len(),
            7_168
        );

        let mut limited_work = ExpansionWorkBudget::new(1_000_000);
        let mut limited =
            VariableResolver::new(&fields, &project, 7_167, &mut limited_work).expect("resolver");
        assert_eq!(
            limited
                .expand("${GROW}", "Value")
                .expect_err("one under")
                .kind,
            SourceBundleErrorKind::ResourceLimit
        );
    }

    #[test]
    fn aggregate_work_limit_fails_before_recursive_growth_is_retained() {
        let fields = BTreeMap::from([("GROW".to_owned(), "${GROW}${GROW}".to_owned())]);
        let project = HashMap::new();
        let mut work = ExpansionWorkBudget::new(32);
        let mut resolver = VariableResolver::new(&fields, &project, 1_000_000, &mut work)
            .expect("field index fits");
        assert_eq!(
            resolver
                .expand("${GROW}", "Value")
                .expect_err("work limit")
                .kind,
            SourceBundleErrorKind::ResourceLimit
        );
    }
}
