//! Checking for and installing updates (`src/updates/`), end to end through the updater plugin: a local
//! feed serves a `latest.json` and an update package signed with a throwaway key, and a recording
//! `System` stands in for the installer and the restart.
//!
//! An integration test rather than unit tests: on Windows only test targets can be given the manifest a
//! Tauri app needs to start (`build.rs`).
#![cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]

use std::collections::HashMap;
use std::io::{Cursor, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use minisign::KeyPair;
use serde_json::{Value, json};
use tauri::Manager;
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::test::{
    INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets,
};
use tauri::webview::InvokeRequest;

use solo_desktop_lib::updates::{
    self, AvailableUpdate, System, UpdateSupport, Updates, update_check, update_install,
    update_support,
};
use tauri_plugin_updater::{Error as UpdaterError, Update, UpdaterBuilder};

/// The version the test app reports.
const CURRENT: &str = "0.4.0";

// ----- What the user is told -----

const UNAVAILABLE: &str = "Updates aren’t available in this version of the app.";
const CHECK_FIRST: &str = "Check for updates first.";
const CHECK_FAILED: &str =
    "Couldn’t check for updates. Check your internet connection and try again.";
const DOWNLOAD_FAILED: &str =
    "The download didn’t finish. Check your internet connection and try again.";
const NOT_GENUINE: &str =
    "The download didn’t pass the security check, so it wasn’t installed. Try again later.";
const NOT_AUTHORISED: &str = "Installing needs your computer’s password. The update wasn’t installed because the password was cancelled or the installation failed.";
const INSTALL_FAILED: &str =
    "The update couldn’t be installed. Try again later, or install the new version yourself.";

/// Progress is sent at most this often (in bytes).
const PROGRESS_STEP: u64 = 256 * 1024;

// ----- A feed on 127.0.0.1 -----

/// Responses by path: status and body.
type Routes = Arc<Mutex<HashMap<String, (u16, Vec<u8>)>>>;

/// Serves fixed responses by path; everything else is a 404.
struct Feed {
    base: String,
    routes: Routes,
}

impl Feed {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind the feed");
        let base = format!("http://{}", listener.local_addr().expect("address"));
        let routes: Routes = Arc::default();
        let served = routes.clone();
        std::thread::spawn(move || {
            for mut stream in listener.incoming().flatten() {
                let mut request = Vec::new();
                let mut buffer = [0_u8; 1024];
                while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                    match stream.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => request.extend_from_slice(&buffer[..n]),
                    }
                }
                let request = String::from_utf8_lossy(&request);
                let path = request.split_whitespace().nth(1).unwrap_or("/").to_owned();
                let (status, body) = served
                    .lock()
                    .unwrap()
                    .get(&path)
                    .cloned()
                    .unwrap_or((404, b"not found".to_vec()));
                let head = format!(
                    "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(&body);
            }
        });
        Self { base, routes }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    fn serve(&self, path: &str, status: u16, body: impl Into<Vec<u8>>) {
        self.routes
            .lock()
            .unwrap()
            .insert(path.to_owned(), (status, body.into()));
    }

    /// Publishes `latest.json` announcing `version`, with a package at `/solo.exe`.
    fn release(&self, version: &str, notes: Option<&str>, signature: &str) {
        let latest = json!({
            "version": version,
            "notes": notes,
            "pub_date": "2026-11-26T09:30:00Z",
            "url": self.url("/solo.exe"),
            "signature": signature,
        });
        self.serve("/latest.json", 200, latest.to_string());
    }
}

// ----- Signing, as the release workflow does -----

fn public_key(keys: &KeyPair) -> String {
    BASE64.encode(keys.pk.to_box().expect("public key").to_string())
}

/// A signature over `data`; like `tauri signer`, its trusted comment names the version it was signed for.
fn sign(keys: &KeyPair, data: &[u8], version: Option<&str>) -> String {
    let comment = match version {
        Some(version) => {
            format!("timestamp:1795645074\tfile:Solo_{version}_x64-setup.exe\tversion:{version}")
        }
        None => "timestamp:1795645074\tfile:Solo_x64-setup.exe".to_owned(),
    };
    let signature = minisign::sign(
        Some(&keys.pk),
        &keys.sk,
        Cursor::new(data),
        Some(&comment),
        None,
    )
    .expect("sign");
    BASE64.encode(signature.to_string())
}

/// An update package large enough to arrive in several pieces.
fn package() -> Vec<u8> {
    (0..700 * 1024).map(|i| (i % 251) as u8).collect()
}

// ----- The app, with a recording system -----

#[derive(Clone, Default)]
struct Recorder {
    events: Arc<Mutex<Vec<String>>>,
    installed: Arc<Mutex<Vec<Vec<u8>>>>,
    /// Errors the next installs fail with, first to last.
    failures: Arc<Mutex<Vec<UpdaterError>>>,
}

impl Recorder {
    fn events(&self) -> Vec<String> {
        self.events.lock().unwrap().clone()
    }
}

impl System for Recorder {
    fn prepare(&self, builder: UpdaterBuilder) -> UpdaterBuilder {
        self.events.lock().unwrap().push("prepare".into());
        builder
    }

    fn install(&self, update: &Update, bytes: &[u8]) -> Result<(), UpdaterError> {
        self.events
            .lock()
            .unwrap()
            .push(format!("install {}", update.version));
        let mut failures = self.failures.lock().unwrap();
        if failures.is_empty() {
            self.installed.lock().unwrap().push(bytes.to_vec());
            Ok(())
        } else {
            Err(failures.remove(0))
        }
    }

    fn restart(&self) {
        self.events.lock().unwrap().push("restart".into());
    }
}

/// The app with the updater configured as the release builds are, pointing at `endpoints`.
fn app(pubkey: &str, endpoints: &[String], system: Recorder) -> tauri::App<MockRuntime> {
    let mut context = mock_context(noop_assets());
    context.config_mut().plugins.0.insert(
        "updater".into(),
        json!({
            "pubkey": pubkey,
            "endpoints": endpoints,
            "requireSignedVersion": true,
            // The test feed is plain HTTP on 127.0.0.1; release feeds are HTTPS.
            "dangerousInsecureTransportProtocol": true,
        }),
    );
    context.package_info_mut().version = CURRENT.parse().expect("version");
    mock_builder()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(Updates::new(system))
        // As registered in `lib.rs`.
        .invoke_handler(updates::commands())
        .build(context)
        .expect("the test app")
}

/// Calls a command the way the page does (`services/platform.ts`): through IPC, with its argument names.
fn invoke(app: &tauri::App<MockRuntime>, cmd: &str, args: Value) -> Result<Value, Value> {
    let webview = match app.get_webview_window("main") {
        Some(webview) => webview,
        None => tauri::WebviewWindowBuilder::new(app, "main", Default::default())
            .build()
            .expect("the page"),
    };
    let request = InvokeRequest {
        cmd: cmd.into(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        // The page's own address (tauri://localhost, or http://tauri.localhost on Windows): anything else is a
        // remote origin, which may not call the app's commands.
        url: webview.url().expect("the page's address"),
        body: InvokeBody::Json(args),
        headers: Default::default(),
        invoke_key: INVOKE_KEY.into(),
    };
    get_ipc_response(&webview, request).map(|body| match body {
        InvokeResponseBody::Json(json) => serde_json::from_str(&json).expect("a JSON response"),
        InvokeResponseBody::Raw(_) => panic!("commands answer with JSON"),
    })
}

/// A test app whose feed and key are ready; the caller publishes the release.
struct Setup {
    feed: Feed,
    keys: KeyPair,
    system: Recorder,
    app: tauri::App<MockRuntime>,
}

fn setup() -> Setup {
    let feed = Feed::start();
    let keys = KeyPair::generate_unencrypted_keypair().expect("keys");
    let system = Recorder::default();
    let app = app(
        &public_key(&keys),
        &[feed.url("/latest.json")],
        system.clone(),
    );
    Setup {
        feed,
        keys,
        system,
        app,
    }
}

impl Setup {
    fn check(&self) -> Result<Option<AvailableUpdate>, String> {
        tauri::async_runtime::block_on(update_check(
            self.app.handle().clone(),
            self.app.state::<Updates>(),
        ))
    }

    /// Installs, returning the result and the progress the page was sent.
    fn install(&self) -> (Result<(), String>, Vec<Value>) {
        let sent: Arc<Mutex<Vec<Value>>> = Arc::default();
        let received = sent.clone();
        let channel = Channel::new(move |body| {
            let InvokeResponseBody::Json(json) = body else {
                panic!("progress is JSON")
            };
            received
                .lock()
                .unwrap()
                .push(serde_json::from_str(&json).expect("progress"));
            Ok(())
        });
        let result =
            tauri::async_runtime::block_on(update_install(self.app.state::<Updates>(), channel));
        let progress = sent.lock().unwrap().clone();
        (result, progress)
    }

    /// Publishes a genuine 0.5.0 and checks for it.
    fn genuine_update_found(&self) -> Vec<u8> {
        let package = package();
        self.feed.serve("/solo.exe", 200, package.clone());
        self.feed.release(
            "0.5.0",
            Some("Faster reports."),
            &sign(&self.keys, &package, Some("0.5.0")),
        );
        assert!(self.check().expect("check").is_some());
        package
    }
}

// ----- Support -----

#[test]
fn updates_are_offered_only_when_the_build_names_a_feed() {
    let keys = KeyPair::generate_unencrypted_keypair().expect("keys");
    let with_feed = app(
        &public_key(&keys),
        &["https://releases.example.com/latest.json".into()],
        Recorder::default(),
    );
    assert_eq!(
        update_support(with_feed.handle().clone()),
        UpdateSupport {
            enabled: true,
            current_version: CURRENT.into()
        }
    );

    let without = app(&public_key(&keys), &[], Recorder::default());
    assert!(!update_support(without.handle().clone()).enabled);
    let check = tauri::async_runtime::block_on(update_check(
        without.handle().clone(),
        without.state::<Updates>(),
    ));
    assert_eq!(check, Err(UNAVAILABLE.into()));
}

#[test]
fn the_app_ships_with_its_public_key_and_requires_signed_versions() {
    let config: Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).expect("tauri.conf.json");
    let updater = &config["plugins"]["updater"];
    assert_eq!(
        updater["requireSignedVersion"], true,
        "an old signed release must not pass for a new one"
    );
    let pubkey = BASE64
        .decode(updater["pubkey"].as_str().expect("pubkey"))
        .expect("base64");
    let pubkey = String::from_utf8(pubkey).expect("text");
    let pubkey = minisign::PublicKeyBox::from_string(&pubkey).expect("a minisign public key");
    assert!(
        minisign::PublicKey::from_box(pubkey).is_ok(),
        "a minisign public key"
    );
    assert_eq!(
        updater["endpoints"],
        json!([]),
        "release builds set the feed from UPDATER_ENDPOINT"
    );
    assert!(updater.get("dangerousInsecureTransportProtocol").is_none());
}

#[test]
fn the_page_checks_and_installs_through_the_commands() {
    let s = setup();
    assert_eq!(
        invoke(&s.app, "update_support", json!({})),
        Ok(json!({ "enabled": true, "current_version": CURRENT }))
    );
    assert_eq!(
        invoke(
            &s.app,
            "update_install",
            json!({ "onProgress": "__CHANNEL__:1" })
        ),
        Err(json!(CHECK_FIRST))
    );

    let package = package();
    s.feed.serve("/solo.exe", 200, package.clone());
    s.feed
        .release("0.5.0", None, &sign(&s.keys, &package, Some("0.5.0")));
    assert_eq!(
        invoke(&s.app, "update_check", json!({})),
        Ok(json!({
            "version": "0.5.0",
            "current_version": CURRENT,
            "published_at": 1_795_685_400,
            "notes": null,
        }))
    );
    assert_eq!(
        invoke(
            &s.app,
            "update_install",
            json!({ "onProgress": "__CHANNEL__:2" })
        ),
        Ok(Value::Null)
    );
    assert_eq!(s.system.events(), ["prepare", "install 0.5.0", "restart"]);
}

// ----- Checking -----

#[test]
fn a_newer_version_is_offered_with_its_date_and_notes() {
    let s = setup();
    s.feed.release(
        "0.5.0",
        Some("Faster reports.\nA new tax summary."),
        &sign(&s.keys, b"x", Some("0.5.0")),
    );
    assert_eq!(
        s.check(),
        Ok(Some(AvailableUpdate {
            version: "0.5.0".into(),
            current_version: CURRENT.into(),
            published_at: Some(1_795_685_400),
            notes: Some("Faster reports.\nA new tax summary.".into()),
        }))
    );
    assert_eq!(s.system.events(), ["prepare"]);

    s.feed
        .release("0.5.1", Some("  \n"), &sign(&s.keys, b"x", Some("0.5.1")));
    let update = s.check().expect("check").expect("an update");
    assert_eq!(update.notes, None, "blank notes aren’t shown");
}

#[test]
fn the_latest_version_needs_no_update() {
    let s = setup();
    for version in [CURRENT, "0.3.9"] {
        s.feed
            .release(version, None, &sign(&s.keys, b"x", Some(version)));
        assert_eq!(s.check(), Ok(None), "{version} isn’t newer");
    }
    s.feed.serve("/latest.json", 204, "");
    assert_eq!(s.check(), Ok(None), "no content means no update");
    let (result, _) = s.install();
    assert_eq!(result, Err(CHECK_FIRST.into()));
}

#[test]
fn a_check_that_fails_says_to_try_again_without_the_detail() {
    let s = setup();
    s.feed.serve("/latest.json", 500, "boom");
    assert_eq!(s.check(), Err(CHECK_FAILED.into()));
    s.feed.serve("/latest.json", 200, "{ not json");
    assert_eq!(s.check(), Err(CHECK_FAILED.into()));
    s.feed.serve(
        "/latest.json",
        200,
        json!({ "version": "0.5.0" }).to_string(),
    );
    assert_eq!(
        s.check(),
        Err(CHECK_FAILED.into()),
        "no package to download"
    );

    // Nothing listening.
    let closed = TcpListener::bind("127.0.0.1:0").expect("bind");
    let gone = format!(
        "http://{}/latest.json",
        closed.local_addr().expect("address")
    );
    drop(closed);
    let offline = app(&public_key(&s.keys), &[gone], Recorder::default());
    let check = tauri::async_runtime::block_on(update_check(
        offline.handle().clone(),
        offline.state::<Updates>(),
    ));
    assert_eq!(check, Err(CHECK_FAILED.into()));
}

#[test]
fn installing_before_checking_asks_for_a_check() {
    let s = setup();
    let (result, progress) = s.install();
    assert_eq!(result, Err(CHECK_FIRST.into()));
    assert!(progress.is_empty());
    assert!(s.system.events().is_empty());
}

// ----- Installing -----

#[test]
fn a_genuine_update_downloads_with_progress_installs_and_restarts() {
    let s = setup();
    let package = s.genuine_update_found();
    let (result, progress) = s.install();
    assert_eq!(result, Ok(()));
    assert_eq!(s.system.events(), ["prepare", "install 0.5.0", "restart"]);
    assert_eq!(
        *s.system.installed.lock().unwrap(),
        std::slice::from_ref(&package),
        "the bytes that were verified are the ones installed"
    );

    let total = package.len() as u64;
    let (installing, downloading) = progress.split_last().expect("progress");
    assert_eq!(*installing, json!({ "event": "installing" }));
    assert!(!downloading.is_empty());
    let mut last = 0;
    for event in downloading {
        assert_eq!(event["event"], "downloading");
        assert_eq!(event["total"], total);
        let downloaded = event["downloaded"].as_u64().expect("bytes");
        assert!(downloaded > last && downloaded <= total);
        last = downloaded;
    }
    assert!(
        downloading.len() as u64 <= total / PROGRESS_STEP + 1,
        "progress is sent every {PROGRESS_STEP} bytes, not for every piece: {}",
        downloading.len()
    );
}

/// Installs the update found by the check and expects it refused as not genuine.
fn refused(s: &Setup) {
    let (result, progress) = s.install();
    assert_eq!(result, Err(NOT_GENUINE.into()));
    assert!(
        !progress.contains(&json!({ "event": "installing" })),
        "nothing is installed"
    );
    assert_eq!(
        s.system.events(),
        ["prepare"],
        "nothing installed or restarted"
    );
}

#[test]
fn a_package_changed_after_signing_is_refused() {
    let s = setup();
    let mut package = s.genuine_update_found();
    package[1000] ^= 1;
    s.feed.serve("/solo.exe", 200, package);
    refused(&s);
}

#[test]
fn a_package_signed_with_another_key_is_refused() {
    let s = setup();
    let package = package();
    let stranger = KeyPair::generate_unencrypted_keypair().expect("keys");
    s.feed.serve("/solo.exe", 200, package.clone());
    s.feed
        .release("0.5.0", None, &sign(&stranger, &package, Some("0.5.0")));
    assert!(s.check().expect("check").is_some());
    refused(&s);
}

#[test]
fn an_old_release_announced_as_a_new_version_is_refused() {
    let s = setup();
    let old = package();
    s.feed.serve("/solo.exe", 200, old.clone());
    // A genuine 0.3.0 package and signature, announced as 0.5.0.
    s.feed
        .release("0.5.0", None, &sign(&s.keys, &old, Some("0.3.0")));
    assert!(s.check().expect("check").is_some());
    refused(&s);

    // A signature that doesn't say which version it's for.
    s.feed.release("0.5.0", None, &sign(&s.keys, &old, None));
    assert!(s.check().expect("check").is_some());
    let (result, _) = s.install();
    assert_eq!(result, Err(NOT_GENUINE.into()));

    // Not a signature at all.
    s.feed.release("0.5.0", None, "not base64!");
    assert!(s.check().expect("check").is_some());
    let (result, _) = s.install();
    assert_eq!(result, Err(NOT_GENUINE.into()));
}

#[test]
fn a_download_that_fails_says_to_check_the_connection() {
    let s = setup();
    s.genuine_update_found();
    s.feed.serve("/solo.exe", 404, "gone");
    let (result, progress) = s.install();
    assert_eq!(result, Err(DOWNLOAD_FAILED.into()));
    assert!(progress.is_empty());
    assert_eq!(s.system.events(), ["prepare"]);
}

#[test]
fn a_failed_install_keeps_the_update_to_try_again() {
    let s = setup();
    s.genuine_update_found();
    s.system.failures.lock().unwrap().extend([
        UpdaterError::AuthenticationFailed,
        UpdaterError::TempDirNotFound,
    ]);

    let (first, _) = s.install();
    assert_eq!(first, Err(NOT_AUTHORISED.into()));
    let (second, _) = s.install();
    assert_eq!(second, Err(INSTALL_FAILED.into()));
    let (third, _) = s.install();
    assert_eq!(third, Ok(()));
    assert_eq!(
        s.system.events(),
        [
            "prepare",
            "install 0.5.0",
            "install 0.5.0",
            "install 0.5.0",
            "restart"
        ],
        "restarts only once it’s installed"
    );
}
