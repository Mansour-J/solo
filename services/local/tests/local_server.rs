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

/// A 1×1 PNG, as a receipt.
const RECEIPT: [u8; 70] = [
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 218, 99, 100, 96, 248, 95, 15, 0, 2,
    135, 1, 128, 235, 71, 186, 146, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

fn authed(
    client: &reqwest::Client,
    server: &LocalServer,
    method: reqwest::Method,
    path: &str,
) -> reqwest::RequestBuilder {
    let mut request = client
        .request(method, format!("{}{path}", server.url()))
        .header("x-client-kind", "desktop");
    if let Some(key) = server.key() {
        request = request.header("x-api-key", key);
    }
    request
}

async fn restore(client: &reqwest::Client, server: &LocalServer, bytes: Vec<u8>) -> (u16, Value) {
    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(bytes).file_name("books.solobackup"),
    );
    let response = authed(
        client,
        server,
        reqwest::Method::POST,
        "/api/v1/users/me/backups/restore",
    )
    .multipart(form)
    .send()
    .await
    .expect("restore");
    let status = response.status().as_u16();
    (status, response.json().await.unwrap_or(Value::Null))
}

#[tokio::test]
async fn a_backup_restores_the_books_and_their_receipts() {
    let dir = tempfile::tempdir().expect("temp dir");
    let client = reqwest::Client::new();
    let server = LocalServer::start(options(dir.path()))
        .await
        .expect("start");
    let get = reqwest::Method::GET;
    let post = reqwest::Method::POST;

    let (_, status) = call(
        &client,
        &server,
        get.clone(),
        "/api/v1/users/me/backups",
        None,
    )
    .await;
    assert_eq!(status["available"], true);
    assert_eq!(status["last"], Value::Null);

    // A business with an expense and its receipt.
    let (_, business) = call(
        &client,
        &server,
        post.clone(),
        "/api/v1/businesses",
        Some(json!({ "name": "Tūī Design", "complete_onboarding": true })),
    )
    .await;
    let base = format!(
        "/api/v1/businesses/{}",
        business["id"].as_str().expect("id")
    );
    let (_, categories) = call(
        &client,
        &server,
        get.clone(),
        &format!("{base}/expense-categories"),
        None,
    )
    .await;
    let (status_code, expense) = call(
        &client,
        &server,
        post.clone(),
        &format!("{base}/expenses"),
        Some(json!({ "expense_date": business["today"], "supplier": "Paper Test Supplies", "category_id": categories[0]["id"],
                     "gross_amount": 2_000, "gst_treatment": "no_gst", "payment_method": "business_account", "classification": "business" })),
    )
    .await;
    assert_eq!(status_code, 201, "{expense}");
    let expense_path = format!("{base}/expenses/{}", expense["id"].as_str().expect("id"));
    let attached = authed(
        &client,
        &server,
        post.clone(),
        &format!("{expense_path}/attachments"),
    )
    .multipart(reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(RECEIPT.to_vec()).file_name("receipt.png"),
    ))
    .send()
    .await
    .expect("attach");
    assert_eq!(attached.status().as_u16(), 201);
    let attached: Value = attached.json().await.expect("json");
    let receipt = format!(
        "{expense_path}/attachments/{}",
        attached["attachments"][0]["id"]
            .as_str()
            .expect("attachment")
    );

    // Back up, then change the books.
    let backup = authed(&client, &server, post.clone(), "/api/v1/users/me/backups")
        .send()
        .await
        .expect("backup");
    assert_eq!(backup.status().as_u16(), 200);
    assert!(
        backup.headers()["content-disposition"]
            .to_str()
            .expect("header")
            .contains(".solobackup")
    );
    let backup = backup.bytes().await.expect("bytes").to_vec();
    let (_, status) = call(
        &client,
        &server,
        get.clone(),
        "/api/v1/users/me/backups",
        None,
    )
    .await;
    assert_eq!(status["last"]["kind"], "backup");
    assert_eq!(status["last"]["size_bytes"], backup.len());
    call(
        &client,
        &server,
        post.clone(),
        "/api/v1/businesses",
        Some(json!({ "name": "Later Ltd", "complete_onboarding": true })),
    )
    .await;

    // Something that isn't a backup is refused, and nothing changes.
    let (code, refused) = restore(&client, &server, b"not a backup".to_vec()).await;
    assert_eq!(code, 422, "{refused}");
    assert!(
        refused.to_string().contains("isn’t a Solo backup"),
        "{refused}"
    );
    let mut names = business_names(&client, &server).await;
    names.sort();
    assert_eq!(names, ["Later Ltd", "Tūī Design"]);

    // Restore: the books are as they were at the backup, after a restart.
    let (code, restored) = restore(&client, &server, backup).await;
    assert_eq!(code, 200, "{restored}");
    assert_eq!(restored["restart_required"], true);
    server.stop().await.expect("stop");
    let kept: Vec<_> = std::fs::read_dir(dir.path().join("backups"))
        .expect("backups folder")
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("before-restore-")
        })
        .collect();
    assert_eq!(kept.len(), 1, "a copy of the books before the restore");

    let server = LocalServer::start(options(dir.path()))
        .await
        .expect("start after restoring");
    assert_eq!(business_names(&client, &server).await, ["Tūī Design"]);
    let file = authed(&client, &server, get.clone(), &receipt)
        .send()
        .await
        .expect("receipt");
    assert_eq!(file.status().as_u16(), 200);
    assert_eq!(file.bytes().await.expect("bytes").as_ref(), RECEIPT);
    let (_, status) = call(&client, &server, get, "/api/v1/users/me/backups", None).await;
    assert_eq!(status["last"]["kind"], "restored");
    server.stop().await.expect("stop");
}
