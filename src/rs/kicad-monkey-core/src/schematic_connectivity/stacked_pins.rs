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
    let total_bytes = range_byte_count(left_prefix.len(), left_value, right_value)
        .ok_or_else(|| limit_error(budget.source_path, "expanded pin bytes overflow"))?;
    ensure_capacity(usage, count, total_bytes, budget)?;
    for number in left_value..=right_value {
        let number_digits = number.to_string();
        let member_bytes = left_prefix
            .len()
            .checked_add(number_digits.len())
            .ok_or_else(|| limit_error(budget.source_path, "expanded pin bytes overflow"))?;
        out.push(format!("{left_prefix}{number_digits}"));
        usage.count += 1;
        usage.bytes += member_bytes;
    }
    Ok(true)
}

fn range_byte_count(prefix_bytes: usize, first: u64, last: u64) -> Option<usize> {
    let count = u128::from(last.checked_sub(first)?.checked_add(1)?);
    let prefix_total = (prefix_bytes as u128).checked_mul(count)?;
    let mut digit_total = 0_u128;
    let first = u128::from(first);
    let last = u128::from(last);
    let mut lower = 0_u128;
    let mut next_power = 10_u128;
    let mut digits = 1_u128;
    while lower <= last {
        let upper = last.min(next_power - 1);
        if upper >= first {
            let band_first = first.max(lower);
            let band_count = upper.checked_sub(band_first)?.checked_add(1)?;
            digit_total = digit_total.checked_add(band_count.checked_mul(digits)?)?;
        }
        lower = next_power;
        next_power = next_power.checked_mul(10)?;
        digits = digits.checked_add(1)?;
    }
    usize::try_from(prefix_total.checked_add(digit_total)?).ok()
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

#[cfg(test)]
mod tests {
    use super::{expand_stacked_pin, range_byte_count};
    use crate::SourceBundleErrorKind;

    #[test]
    fn range_bytes_cover_decimal_boundaries_without_enumeration() {
        assert_eq!(range_byte_count(2, 0, 0), Some(3));
        assert_eq!(range_byte_count(2, 8, 12), Some(18));
        assert_eq!(range_byte_count(0, 98, 102), Some(13));
        assert_eq!(range_byte_count(1, u64::MAX - 1, u64::MAX), Some(42));
    }

    #[test]
    fn range_bytes_are_rejected_before_member_generation() {
        let value = "[LONGPREFIX99990-LONGPREFIX100010]";
        let exact_bytes =
            range_byte_count("LONGPREFIX".len(), 99_990, 100_010).expect("range byte count");
        let expanded = expand_stacked_pin(value, 21, exact_bytes, "design/root.kicad_sch", "count")
            .expect("exact aggregate byte limit");
        assert_eq!(expanded.len(), 21);
        assert_eq!(expanded.iter().map(String::len).sum::<usize>(), exact_bytes);

        let error =
            expand_stacked_pin(value, 21, exact_bytes - 1, "design/root.kicad_sch", "count")
                .expect_err("one-under aggregate byte limit");
        assert_eq!(error.kind, SourceBundleErrorKind::ResourceLimit);
        assert!(error.message.contains("expanded pin bytes"));
    }
}
