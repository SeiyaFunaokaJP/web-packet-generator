//! `wpg config token` and `wpg remote` against a server started in-process on a
//! free port (the CLI runs as a child process, like in a script).

use std::path::PathBuf;
use std::process::{Command, Output};

use serde_json::Value;
use wpg_server::{Config, ServeOptions, Server};

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// A running server; stops when dropped.
struct TestServer {
    url: String,
    data_dir: PathBuf,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
    _dir: tempfile::TempDir,
}

impl TestServer {
    fn start() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path().join("data");
        let mut config = Config {
            data_dir: data_dir.clone(),
            ..Config::default()
        };
        config.templates.library_paths = vec![repo().join("templates")];
        config.server.bind = "127.0.0.1:0".parse().unwrap();
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let server = rt
            .block_on(Server::bind(config, ServeOptions::default()))
            .unwrap();
        let url = format!("http://{}", server.local_addr().unwrap());
        // a password exists (the setup screen is done); its value does not matter here
        server
            .state()
            .auth
            .db()
            .create_user("admin", wpg_server::db::Role::Admin, "$argon2id$unused", 1)
            .unwrap()
            .unwrap();
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        let thread = std::thread::spawn(move || {
            rt.block_on(server.run_until(async move {
                let _ = rx.await;
            }))
            .unwrap();
        });
        Self {
            url,
            data_dir,
            stop: Some(tx),
            thread: Some(thread),
            _dir: dir,
        }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        if let Some(tx) = self.stop.take() {
            let _ = tx.send(());
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn wpg(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_wpg"))
        .current_dir(repo())
        .env("LANG", "en")
        .env_remove("WPG_CONFIG")
        .env_remove("WPG_SERVER")
        .env_remove("WPG_TOKEN")
        .env_remove("WPG_PROTOCOLS_DIR")
        .args(args)
        .output()
        .unwrap()
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).trim().to_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

#[track_caller]
fn ok(o: &Output) -> String {
    assert_eq!(o.status.code(), Some(0), "stderr: {}", stderr(o));
    stdout(o)
}

fn json(o: &Output) -> Value {
    serde_json::from_str(&ok(o)).unwrap_or_else(|e| panic!("{e}: {}", stdout(o)))
}

#[test]
fn tokens_and_remote_jobs() {
    let srv = TestServer::start();
    let data = srv.data_dir.display().to_string();

    // a token from the local database, while the server runs
    let secret = ok(&wpg(&[
        "config",
        "token",
        "create",
        "ci",
        "--data-dir",
        &data,
    ]));
    assert!(secret.starts_with("wpg_") && secret.len() == 68, "{secret}");
    let list = json(&wpg(&[
        "config",
        "token",
        "list",
        "--json",
        "--data-dir",
        &data,
    ]));
    assert_eq!(list[0]["name"], "ci");
    assert_eq!(list[0]["user"], "admin");
    let token_id = list[0]["id"].as_i64().unwrap().to_string();
    let human = ok(&wpg(&["config", "token", "list", "--data-dir", &data]));
    assert!(
        human.contains(&secret[..12]) && !human.contains(&secret),
        "{human}"
    );

    let remote = |args: &[&str]| {
        let mut all = vec!["remote", "--server", &srv.url, "--token", &secret];
        all.extend_from_slice(args);
        wpg(&all)
    };

    // templates
    let t = json(&remote(&["templates", "list", "--json"]));
    assert!(t["templates"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["id"] == "examples-arp-sweep"));
    let file = repo().join("templates/examples/arp-sweep.wpg.yaml");
    let file = file.display().to_string();
    let pushed = ok(&remote(&["templates", "push", &file]));
    assert!(!pushed.is_empty());
    let t = ok(&remote(&["templates", "list"]));
    assert!(t.contains(&pushed), "{t}");

    // a job from a server template, waited for
    let j = json(&remote(&[
        "jobs",
        "start",
        "examples-arp-sweep",
        "--backend",
        "null",
        "--count",
        "20",
        "--wait",
        "--json",
    ]));
    assert_eq!(j["status"]["state"], "finished", "{j}");
    assert_eq!(j["status"]["detail"], "count_reached");

    // a job from a local file, then `jobs wait`
    let id = ok(&remote(&[
        "jobs",
        "start",
        &file,
        "--backend",
        "null",
        "--count",
        "5",
        "--name",
        "from-file",
    ]));
    let out = ok(&remote(&["jobs", "wait", &id, "--timeout", "30"]));
    assert!(out.contains(&id) && out.contains("finished"), "{out}");
    let jobs = json(&remote(&["jobs", "list", "--json"]));
    assert!(jobs
        .as_array()
        .unwrap()
        .iter()
        .any(|j| j["id"] == id.as_str()));

    // a long job, stopped
    let id = ok(&remote(&[
        "jobs",
        "start",
        "examples-arp-sweep",
        "--backend",
        "null",
        "--rate",
        "10pps",
        "--count",
        "100000",
    ]));
    ok(&remote(&["jobs", "stop", &id]));
    let j = json(&remote(&["jobs", "wait", &id, "--json"]));
    assert_eq!(j["status"]["detail"], "stopped", "{j}");

    // server-side errors are reported in the CLI language, exit status 2
    let o = remote(&["jobs", "wait", "no-such-job"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(stderr(&o).contains("404"), "{}", stderr(&o));
    let o = remote(&["jobs", "start", "no-such-template", "--backend", "null"]);
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));

    // revoked: the next request is refused
    ok(&wpg(&[
        "config",
        "token",
        "revoke",
        &token_id,
        "--data-dir",
        &data,
    ]));
    let o = remote(&["jobs", "list"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(stderr(&o).contains("401"), "{}", stderr(&o));
    let o = wpg(&["config", "token", "revoke", &token_id, "--data-dir", &data]);
    assert_eq!(o.status.code(), Some(1));
}

#[test]
fn remote_errors() {
    let srv = TestServer::start();
    // no token
    let o = wpg(&["remote", "--server", &srv.url, "jobs", "list"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(stderr(&o).contains("401"), "{}", stderr(&o));
    // token from the environment, wrong
    let o = Command::new(env!("CARGO_BIN_EXE_wpg"))
        .env("LANG", "en")
        .env("WPG_SERVER", &srv.url)
        .env("WPG_TOKEN", format!("wpg_{}", "0".repeat(64)))
        .args(["remote", "templates", "list"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2));
    assert!(stderr(&o).contains("401"), "{}", stderr(&o));
    // nothing listening
    let o = wpg(&["remote", "--server", "http://127.0.0.1:9", "jobs", "list"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(stderr(&o).contains("127.0.0.1:9"), "{}", stderr(&o));
    // not a URL
    let o = wpg(&["remote", "--server", "ftp://x", "jobs", "list"]);
    assert_eq!(o.status.code(), Some(2));
    // token tools need an existing database
    let dir = tempfile::tempdir().unwrap();
    let o = wpg(&[
        "config",
        "token",
        "list",
        "--data-dir",
        &dir.path().display().to_string(),
    ]);
    assert_eq!(o.status.code(), Some(1));
    // unknown user
    let o = wpg(&[
        "config",
        "token",
        "create",
        "x",
        "--user",
        "nobody",
        "--data-dir",
        &srv.data_dir.display().to_string(),
    ]);
    assert_eq!(o.status.code(), Some(1));
    assert!(stderr(&o).contains("nobody"), "{}", stderr(&o));
}
