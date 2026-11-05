#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if !accepts_args(std::env::args_os().skip(1)) {
        eprintln!("Usage: MediaAtlas [file.nfo]");
        std::process::exit(2);
    }
    mediaatlas_desktop::run();
}

fn accepts_args(args: impl IntoIterator<Item = std::ffi::OsString>) -> bool {
    let mut args = args.into_iter();
    let Some(path) = args.next() else {
        return true;
    };
    args.next().is_none()
        && std::path::Path::new(&path)
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case("nfo"))
}

#[cfg(test)]
mod tests {
    use super::accepts_args;

    #[test]
    fn accepts_normal_and_single_nfo_launches_only() {
        assert!(accepts_args(Vec::<std::ffi::OsString>::new()));
        assert!(accepts_args([std::ffi::OsString::from("selected.NFO")]));
        for args in [
            vec!["--unknown"],
            vec!["selected.xml"],
            vec!["one.nfo", "two.nfo"],
        ] {
            assert!(!accepts_args(args.into_iter().map(std::ffi::OsString::from)));
        }
    }
}
