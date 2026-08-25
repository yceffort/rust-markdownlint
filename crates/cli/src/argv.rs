/// cli2 `markdownlint-cli2.mjs` 의 argv 필터를 그대로 따른 단일 패스 파서 결과.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Argv {
    pub globs: Vec<String>,
    /// `Some(None)` 은 `--config` 가 값 없이 끝난 경우.
    pub config_path: Option<Option<String>>,
    pub config_pointer: Option<String>,
    pub fix: bool,
    pub format: bool,
    pub use_stdin: bool,
    pub help: bool,
    pub no_globs: bool,
}

pub fn parse_argv(args: &[String]) -> Argv {
    let mut argv = Argv::default();
    let mut saw_dash_dash = false;
    let mut pointer_pending = false;
    for arg in args {
        if saw_dash_dash {
            argv.globs.push(arg.clone());
        } else if argv.config_path == Some(None) {
            argv.config_path = Some(Some(arg.clone()));
        } else if pointer_pending {
            argv.config_pointer = Some(arg.clone());
            pointer_pending = false;
        } else if arg == "-" {
            argv.use_stdin = true;
        } else if arg == "--" {
            saw_dash_dash = true;
        } else if arg == "--config" {
            argv.config_path = Some(None);
        } else if arg == "--configPointer" {
            pointer_pending = true;
        } else if arg == "--fix" {
            argv.fix = true;
        } else if arg == "--format" {
            argv.format = true;
            argv.use_stdin = true;
        } else if arg == "--help" {
            argv.help = true;
        } else if arg == "--no-globs" {
            argv.no_globs = true;
        } else {
            argv.globs.push(arg.clone());
        }
    }
    argv
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s<const N: usize>(args: [&str; N]) -> Vec<String> {
        args.iter().map(|a| a.to_string()).collect()
    }

    #[test]
    fn argv_single_pass() {
        let a = parse_argv(&s([
            "--fix", "--config", "x.jsonc", "--", "--weird", "docs",
        ]));
        assert!(a.fix);
        assert_eq!(a.config_path, Some(Some("x.jsonc".into())));
        assert_eq!(a.globs, ["--weird", "docs"]);
    }

    #[test]
    fn unknown_flag_is_glob() {
        assert_eq!(parse_argv(&s(["--xyz"])).globs, ["--xyz"]);
    }

    #[test]
    fn config_without_value() {
        assert_eq!(parse_argv(&s(["--config"])).config_path, Some(None));
    }

    #[test]
    fn pending_config_consumes_next_flag() {
        // 원본은 --config 직후 인자를 플래그여도 값으로 소비한다
        let a = parse_argv(&s(["--config", "--fix"]));
        assert_eq!(a.config_path, Some(Some("--fix".into())));
        assert!(!a.fix);
    }

    #[test]
    fn config_pointer_and_flags() {
        let a = parse_argv(&s([
            "--configPointer",
            "/p",
            "--format",
            "-",
            "--no-globs",
            "--help",
        ]));
        assert_eq!(a.config_pointer, Some("/p".into()));
        assert!(a.format);
        assert!(a.use_stdin);
        assert!(a.no_globs);
        assert!(a.help);
        assert!(a.globs.is_empty());
    }
}
