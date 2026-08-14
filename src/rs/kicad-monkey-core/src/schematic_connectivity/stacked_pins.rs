use crate::{SourceBundleError, SourceBundleErrorKind};

#[derive(Clone, Copy)]
struct Budget<'a> {
    max_count: usize,
    max_bytes: usize,
    source_path: &'a str,
    count_message: &'a str,
}

#[derive(Default)]
struct Usage {
    count: usize,
    bytes: usize,
}

pub(super) fn expand_stacked_pin(
    value: &str,
    max_count: usize,
    max_bytes: usize,
    source_path: &str,
    count_message: &str,
) -> Result<Vec<String>, SourceBundleError> {
    let budget = Budget {
        max_count,
        max_bytes,
        source_path,
        count_message,
    };
    if !(value.starts_with('[') && value.ends_with(']')) {
        ensure_capacity(&Usage::default(), 1, value.len(), budget)?;
        return Ok(vec![value.to_owned()]);
    }
    let mut out = Vec::new();
    let mut usage = Usage::default();
    for raw in value[1..value.len() - 1].split(',') {
        let part = raw.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((left, right)) = part.split_once('-') {
            if !push_range(&mut out, &mut usage, left.trim(), right.trim(), budget)? {
                ensure_capacity(&Usage::default(), 1, value.len(), budget)?;
                return Ok(vec![value.to_owned()]);
            }
        } else {
            ensure_capacity(&usage, 1, part.len(), budget)?;
            out.push(part.to_owned());
            usage.count += 1;
            usage.bytes += part.len();
        }
    }
    Ok(out)
}

fn push_range(
    out: &mut Vec<String>,
    usage: &mut Usage,
    left: &str,
    right: &str,
    budget: Budget<'_>,
) -> Result<bool, SourceBundleError> {
    let Some((left_prefix, left_value)) = alpha_numeric_pin(left) else {
        return Ok(false);
    };
    let Some((right_prefix, right_value)) = alpha_numeric_pin(right) else {
        return Ok(false);
    };
    if left_prefix != right_prefix || left_value > right_value {
        return Ok(false);
    }
    let range = right_value
        .checked_sub(left_value)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| limit_error(budget.source_path, "stacked pin range overflows"))?;
    let count = usize::try_from(range).map_err(|_| {
        limit_error(
            budget.source_path,
            "stacked pin range exceeds platform size",
        )
    })?;
    ensure_capacity(usage, count, 0, budget)?;
    for number in left_value..=right_value {
        let number_digits = number.to_string();
        let member_bytes = left_prefix
            .len()
            .checked_add(number_digits.len())
            .ok_or_else(|| limit_error(budget.source_path, "expanded pin bytes overflow"))?;
        ensure_capacity(usage, 1, member_bytes, budget)?;
        out.push(format!("{left_prefix}{number_digits}"));
        usage.count += 1;
        usage.bytes += member_bytes;
    }
    Ok(true)
}

fn ensure_capacity(
    usage: &Usage,
    added_count: usize,
    added_bytes: usize,
    budget: Budget<'_>,
) -> Result<(), SourceBundleError> {
    if usage
        .count
        .checked_add(added_count)
        .is_none_or(|total| total > budget.max_count)
    {
        return Err(limit_error(budget.source_path, budget.count_message));
    }
    if usage
        .bytes
        .checked_add(added_bytes)
        .is_none_or(|total| total > budget.max_bytes)
    {
        return Err(limit_error(
            budget.source_path,
            "expanded pin bytes exceed their limit",
        ));
    }
    Ok(())
}

fn alpha_numeric_pin(value: &str) -> Option<(&str, u64)> {
    let suffix = value
        .char_indices()
        .rev()
        .take_while(|(_, character)| character.is_ascii_digit())
        .last()
        .map_or(value.len(), |(index, _)| index);
    (suffix < value.len())
        .then(|| {
            value[suffix..]
                .parse()
                .ok()
                .map(|number| (&value[..suffix], number))
        })
        .flatten()
}

fn limit_error(source_path: &str, message: &str) -> SourceBundleError {
    SourceBundleError::new(
        SourceBundleErrorKind::ResourceLimit,
        Some(source_path),
        message,
    )
}
