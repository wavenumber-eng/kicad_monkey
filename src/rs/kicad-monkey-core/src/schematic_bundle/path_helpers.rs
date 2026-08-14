pub(super) fn portable_file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

pub(super) fn portable_parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(parent, _name)| parent)
}

pub(super) fn nonempty_or<'a>(preferred: &'a str, fallback: &'a str) -> &'a str {
    if preferred.is_empty() {
        fallback
    } else {
        preferred
    }
}

pub(super) fn join_occurrence_path(parent: &str, segment: &str, trailing_slash: bool) -> String {
    let parent = parent.trim_matches('/');
    let segment = segment.trim_matches('/');
    let joined = match (parent.is_empty(), segment.is_empty()) {
        (true, true) => String::new(),
        (true, false) => segment.to_owned(),
        (false, true) => parent.to_owned(),
        (false, false) => format!("{parent}/{segment}"),
    };
    if trailing_slash {
        if joined.is_empty() {
            "/".to_owned()
        } else {
            format!("/{joined}/")
        }
    } else {
        format!("/{joined}")
    }
}
