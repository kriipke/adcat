// Copyright 2020 Sebastian Wiesner <sebastian@swsnr.de>

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Test the command line interface of adcat

#![deny(warnings, clippy::all)]

mod cli {
    use std::fs;
    use std::ffi::OsStr;
    use std::io::{Read, Write};
    use std::path::PathBuf;
    use std::process::{Command, Output, Stdio};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn cargo_adcat() -> Command {
        Command::new(env!("CARGO_BIN_EXE_adcat"))
    }

    fn run_cargo_adcat<I, S>(args: I) -> Output
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        cargo_adcat().args(args).output().unwrap()
    }

    fn temp_fixture_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("adcat-{name}-{unique}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn show_help() {
        let output = run_cargo_adcat(["--help"]);
        let stdout = std::str::from_utf8(&output.stdout).unwrap();
        assert!(
            output.status.success(),
            "non-zero exit code: {:?}",
            output.status,
        );
        assert!(output.stderr.is_empty());
        assert!(stdout.contains("See 'man 1 adcat' for more information."));
    }

    #[test]
    fn long_version_includes_license() {
        let output = run_cargo_adcat(["--version"]);
        let stdout = std::str::from_utf8(&output.stdout).unwrap();
        assert!(
            output.status.success(),
            "non-zero exit code: {:?}",
            output.status,
        );
        assert!(output.stderr.is_empty());
        assert!(
            stdout.contains("This program is subject to the terms of the Mozilla Public License,")
        );
    }

    #[test]
    fn file_list_fail_late() {
        let output = run_cargo_adcat(["does-not-exist", "sample/common-mark.md"]);
        let stderr = std::str::from_utf8(&output.stderr).unwrap();
        let stdout = std::str::from_utf8(&output.stdout).unwrap();
        assert!(!output.status.success());
        // We failed to read the first file but still printed the second.
        assert!(
            stderr.contains("Error: does-not-exist:") && stderr.contains("(os error 2)"),
            "Stderr: {stderr}",
        );
        assert!(stdout.contains("CommonMark sample document"));
    }

    #[test]
    fn file_list_fail_fast() {
        let output = run_cargo_adcat(["--fail", "does-not-exist", "sample/common-mark.md"]);
        let stderr = std::str::from_utf8(&output.stderr).unwrap();
        assert!(!output.status.success());
        // We failed to read the first file and exited early, so nothing was printed at all
        assert!(
            stderr.contains("Error: does-not-exist:") && stderr.contains("(os error 2)"),
            "Stderr: {stderr}",
        );
        assert!(output.stdout.is_empty());
    }

    #[test]
    fn ignore_broken_pipe() {
        let mut child = cargo_adcat()
            .stdin(Stdio::piped())
            // .arg("sample/common-mark.md")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();

        let mut stdin = child.stdin.take().unwrap();
        let mut stderr = Vec::new();
        drop(child.stdout.take());

        writeln!(stdin, "Hello world").unwrap();
        drop(stdin);
        child
            .stderr
            .as_mut()
            .unwrap()
            .read_to_end(&mut stderr)
            .unwrap();
        let exit_code = child.wait().unwrap();

        similar_asserts::assert_eq!(String::from_utf8_lossy(&stderr), "");
        assert_eq!(exit_code.code().unwrap(), 0);
    }

    #[test]
    fn asciidoc_includes_are_preprocessed_from_files() {
        let fixture_dir = temp_fixture_dir("include");
        let main = fixture_dir.join("main.adoc");
        let included = fixture_dir.join("included.adoc");
        fs::write(&included, "Included paragraph from another file.\n").unwrap();
        fs::write(
            &main,
            "= Include Demo\n\nBefore include.\n\ninclude::included.adoc[]\n\nAfter include.\n",
        )
        .unwrap();

        let args = vec![OsStr::new("--ansi"), main.as_os_str()];
        let output = run_cargo_adcat(args);
        let stdout = String::from_utf8(output.stdout).unwrap();

        assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
        assert!(stdout.contains("Before include."));
        assert!(stdout.contains("Included paragraph from another file."));
        assert!(stdout.contains("After include."));
    }

    #[test]
    fn asciidoc_conditionals_are_preprocessed() {
        let fixture_dir = temp_fixture_dir("conditionals");
        let main = fixture_dir.join("main.adoc");
        fs::write(
            &main,
            "= Conditional Demo\n:feature:\n\nifdef::feature[]\nFeature enabled\nendif::[]\nifndef::adcat-conditional-missing[]\nMissing disabled\nendif::[]\nifeval::[1 + 1 == 2]\nMath works\nendif::[]\n",
        )
        .unwrap();

        let args = vec![OsStr::new("--ansi"), main.as_os_str()];
        let output = run_cargo_adcat(args);
        let stdout = String::from_utf8(output.stdout).unwrap();

        assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
        assert!(stdout.contains("Feature enabled"));
        assert!(stdout.contains("Missing disabled"));
        assert!(stdout.contains("Math works"));
    }
}
