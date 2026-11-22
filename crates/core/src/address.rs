//! Printer address helpers shared by the commands and the device hub.

/// Split the target of an address (after `scheme:`) from its `?key=value` options.
///
/// The options follow the last `?` that has no path separator after it. A Windows
/// verbatim path such as `\\?\C:\out` therefore stays whole, with or without options.
pub fn split_query(rest: &str) -> (&str, Option<&str>) {
    match rest.rsplit_once('?') {
        Some((target, query)) if !query.contains(['/', '\\']) => (target, Some(query)),
        _ => (rest, None),
    }
}

#[cfg(test)]
mod tests {
    use super::split_query;

    #[test]
    fn splits_options() {
        assert_eq!(
            split_query("/tmp/out?model=x"),
            ("/tmp/out", Some("model=x"))
        );
        assert_eq!(split_query("/tmp/out"), ("/tmp/out", None));
        assert_eq!(split_query("?model=x"), ("", Some("model=x")));
    }

    #[test]
    fn keeps_windows_verbatim_paths() {
        assert_eq!(split_query(r"\\?\C:\out"), (r"\\?\C:\out", None));
        assert_eq!(
            split_query(r"\\?\C:\out?model=x&delay_ms=5"),
            (r"\\?\C:\out", Some("model=x&delay_ms=5"))
        );
    }
}
