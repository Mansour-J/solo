//! The local server end to end, with the bundled PostgreSQL: set up from an empty folder, records kept
//! across restarts, and a database left running by a killed app taken over cleanly.

use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};
use solo_local::{LocalServer, Options};

fn options(dir: &Path) -> Options {
    Options {
        data_dir: dir.to_path_buf(),
        cors_allowed_origins: vec!["tauri://localhost".into()],
        require_key: true,
    }
}

async fn call(
    client: &reqwest::Client,
    server: &LocalServer,
    method: reqwest::Method,
    path: &str,
    body: Option<Value>,
) -> (u16, Value) {
    let mut request = client
        .request(method, format!("{}{path}", server.url()))
        .header("x-client-kind", "desktop");
    if let Some(key) = server.key() {
        request = request.header("x-api-key", key);
    }
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request.send().await.expect("request");
    let status = response.status().as_u16();
    (status, response.json().await.unwrap_or(Value::Null))
}

async fn business_names(client: &reqwest::Client, server: &LocalServer) -> Vec<String> {
    let (status, list) = call(
        client,
        server,
        reqwest::Method::GET,
        "/api/v1/businesses",
        None,
    )
    .await;
    assert_eq!(status, 200, "{list}");
    list.as_array()
        .expect("list")
        .iter()
        .map(|b| b["name"].as_str().unwrap_or_default().to_owned())
        .collect()
}

#[tokio::test]
async fn records_survive_a_restart_and_a_killed_app() {
    let dir = tempfile::tempdir().expect("temp dir");
    let client = reqwest::Client::new();

    // First start: an empty folder becomes a working server, ready without signing in.
    let server = LocalServer::start(options(dir.path()))
        .await
        .expect("first start");
    assert!(
        server.url().starts_with("http://127.0.0.1:"),
        "{}",
        server.url()
    );
    let (_, meta) = call(&client, &server, reqwest::Method::GET, "/api/v1/meta", None).await;
    assert_eq!(meta["email_enabled"], false);
    let (status, created) = call(
        &client,
        &server,
        reqwest::Method::POST,
        "/api/v1/businesses",
        Some(json!({ "name": "Kererū Studio", "complete_onboarding": true })),
    )
    .await;
    assert_eq!(status, 201, "{created}");
    // Without the key, nobody else gets in.
    let stranger = client
        .get(format!("{}/api/v1/businesses", server.url()))
        .send()
        .await
        .expect("request");
    assert_eq!(stranger.status().as_u16(), 403);
    let (first_url, first_key) = (server.url().to_owned(), server.key().map(str::to_owned));
    server.stop().await.expect("stop");

    // Second start: same folder, same books, a fresh port and key.
    let server = LocalServer::start(options(dir.path()))
        .await
        .expect("second start");
    assert_eq!(business_names(&client, &server).await, ["Kererū Studio"]);
    assert_ne!(server.url(), first_url, "a new random port each start");
    assert_ne!(
        server.key().map(str::to_owned),
        first_key,
        "a new key each start"
    );
    server.stop().await.expect("stop");

    // The app is killed while its database runs: the next start stops that database and carries on.
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_solo-local"))
        .arg("--data-dir")
        .arg(dir.path())
        .arg("--url-file")
        .arg(dir.path().join("url.txt"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn solo-local");
    let url_file = dir.path().join("url.txt");
    for _ in 0..120 {
        if url_file.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    assert!(url_file.exists(), "solo-local started");
    child.kill().expect("kill");
    let _ = child.wait();
    assert!(
        dir.path().join("database/postmaster.pid").exists(),
        "the database outlived the app"
    );

    let server = LocalServer::start(options(dir.path()))
        .await
        .expect("start after a crash");
    assert_eq!(business_names(&client, &server).await, ["Kererū Studio"]);
    server.stop().await.expect("stop");
}
