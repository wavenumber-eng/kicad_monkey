//! Generic owned-tree mutation primitives.

use crate::sexpr::{Error, ErrorKind, Sexp};

fn named_list(value: &Sexp, name: &str) -> bool {
    let Sexp::List(values) = value else {
        return false;
    };
    values.first().is_some_and(|head| head.is_atom(name))
}

/// Replace the first direct child list whose head matches `name`.
pub fn replace_element(root: &mut Sexp, name: &str, replacement: Sexp) -> bool {
    let Sexp::List(values) = root else {
        return false;
    };
    let Some(index) = values.iter().position(|value| named_list(value, name)) else {
        return false;
    };
    values[index] = replacement;
    true
}

/// Remove and return the first direct child list whose head matches `name`.
pub fn remove_element(root: &mut Sexp, name: &str) -> Option<Sexp> {
    let Sexp::List(values) = root else {
        return None;
    };
    values
        .iter()
        .position(|value| named_list(value, name))
        .map(|index| values.remove(index))
}

/// Remove every direct child list whose head matches `name`, preserving order.
pub fn remove_all_elements(root: &mut Sexp, name: &str) -> Vec<Sexp> {
    let Sexp::List(values) = root else {
        return Vec::new();
    };
    let mut removed = Vec::new();
    let mut retained = Vec::with_capacity(values.len());
    for value in values.drain(..) {
        if named_list(&value, name) {
            removed.push(value);
        } else {
            retained.push(value);
        }
    }
    *values = retained;
    removed
}

/// Set `(name value)` as a direct child, replacing the first match or appending.
pub fn set_value(root: &mut Sexp, name: &str, value: Sexp) -> Result<(), Error> {
    if !matches!(root, Sexp::List(_)) {
        return Err(Error::build(
            ErrorKind::InvalidBuildValue,
            "set_value root must be a list",
        ));
    }
    let replacement = Sexp::List(vec![Sexp::Atom(name.to_owned()), value]);
    if !replace_element(root, name, replacement.clone()) {
        let Sexp::List(values) = root else {
            return Err(Error::build(
                ErrorKind::InvalidBuildValue,
                "set_value root must be a list",
            ));
        };
        values.push(replacement);
    }
    Ok(())
}

/// Find a nested sequence of direct child list heads.
pub fn find_path<'a>(root: &'a Sexp, names: &[&str]) -> Option<&'a Sexp> {
    let mut current = root;
    for name in names {
        let Sexp::List(values) = current else {
            return None;
        };
        current = values.iter().find(|value| named_list(value, name))?;
    }
    Some(current)
}

/// Depth-first iterator over list nodes, including a list root.
pub struct Walk<'a> {
    stack: Vec<&'a Sexp>,
}

impl<'a> Iterator for Walk<'a> {
    type Item = &'a Sexp;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(value) = self.stack.pop() {
            let Sexp::List(children) = value else {
                continue;
            };
            self.stack.extend(children.iter().rev());
            return Some(value);
        }
        None
    }
}

/// Walk all list nodes depth-first.
pub fn walk(root: &Sexp) -> Walk<'_> {
    Walk { stack: vec![root] }
}

/// Replace every matching descendant without recursing into replacements.
pub fn transform_descendants<F>(root: &mut Sexp, name: &str, transform: &mut F) -> usize
where
    F: FnMut(&Sexp) -> Sexp,
{
    let Sexp::List(values) = root else {
        return 0;
    };
    let mut count = 0;
    for child in values {
        if named_list(child, name) {
            *child = transform(child);
            count += 1;
        } else {
            count += transform_descendants(child, name, transform);
        }
    }
    count
}
