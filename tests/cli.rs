use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

fn temp_log(name: &str) -> String {
    let path = std::env::temp_dir().join(format!("memdb-cli-{}-{}.log", std::process::id(), name));
    let _ = std::fs::remove_file(&path);
    path.to_str().unwrap().to_string()
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_memdb")).args(args).output().unwrap()
}

fn run_with_stdin(args: &[&str], input: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_memdb"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    child.wait_with_output().unwrap()
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn new_put_get_round_trip() {
    let log = temp_log("round_trip");
    assert!(run(&[&log, "new"]).status.success());
    assert!(run(&[&log, "put", "1", "hello"]).status.success());

    let out = run(&[&log, "get", "1"]);
    assert!(out.status.success());
    assert_eq!(stdout(&out), "hello\n");
    std::fs::remove_file(&log).unwrap();
}

#[test]
fn put_reads_content_from_stdin_when_no_argument_is_given() {
    let log = temp_log("stdin");
    run(&[&log, "new"]);
    assert!(run_with_stdin(&[&log, "put", "2"], "from stdin").status.success());

    assert_eq!(stdout(&run(&[&log, "get", "2"])), "from stdin\n");
    std::fs::remove_file(&log).unwrap();
}

#[test]
fn put_on_an_existing_id_overwrites_it() {
    let log = temp_log("overwrite");
    run(&[&log, "new"]);
    run(&[&log, "put", "1", "old"]);
    run(&[&log, "put", "1", "new"]);

    assert_eq!(stdout(&run(&[&log, "get", "1"])), "new\n");
    std::fs::remove_file(&log).unwrap();
}

#[test]
fn get_after_delete_fails() {
    let log = temp_log("delete");
    run(&[&log, "new"]);
    run(&[&log, "put", "1", "hello"]);
    assert!(run(&[&log, "delete", "1"]).status.success());

    let out = run(&[&log, "get", "1"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("no document with id 1"));
    std::fs::remove_file(&log).unwrap();
}

#[test]
fn get_of_an_unknown_id_fails() {
    let log = temp_log("unknown_id");
    run(&[&log, "new"]);

    let out = run(&[&log, "get", "7"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("no document with id 7"));
    std::fs::remove_file(&log).unwrap();
}

#[test]
fn commands_on_a_missing_log_fail_and_do_not_create_it() {
    let log = temp_log("missing");
    for args in [
        vec![log.as_str(), "get", "1"],
        vec![log.as_str(), "put", "1", "hello"],
        vec![log.as_str(), "delete", "1"],
    ] {
        let out = run(&args);
        assert_eq!(out.status.code(), Some(1), "{args:?}");
        assert!(stderr(&out).contains("no log file"), "{args:?}");
        assert!(!Path::new(&log).exists(), "{args:?} created the file");
    }
}

#[test]
fn new_fails_if_the_log_already_exists() {
    let log = temp_log("new_twice");
    assert!(run(&[&log, "new"]).status.success());
    assert_eq!(run(&[&log, "new"]).status.code(), Some(1));
    std::fs::remove_file(&log).unwrap();
}

#[test]
fn a_file_that_is_not_a_log_is_rejected() {
    let log = temp_log("junk");
    std::fs::write(&log, "definitely not a memdb log").unwrap();

    let out = run(&[&log, "get", "1"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("not a memdb log"));
    std::fs::remove_file(&log).unwrap();
}

#[test]
fn a_non_numeric_id_is_a_usage_error() {
    let log = temp_log("bad_id");
    run(&[&log, "new"]);

    let out = run(&[&log, "get", "abc"]);
    assert_eq!(out.status.code(), Some(2));
    std::fs::remove_file(&log).unwrap();
}
