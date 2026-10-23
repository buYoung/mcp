use codemap_search::related_terms::fetch::{
    fetch_dataset, DatasetSpec, FetchStatus, MAX_DOWNLOAD_BYTES,
};
use sha2::{Digest, Sha256};
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};
use std::thread;
use std::time::{Duration, Instant};

const DATA: &[u8] = b"fixture sense dataset bytes";
const ID: &str = "fetch-fixture";

fn digest(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn archive(entries: &[(&str, tar::EntryType, &[u8])]) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    for (name, kind, data) in entries {
        let mut header = tar::Header::new_ustar();
        // Raw names intentionally permit malicious fixtures rejected by Builder::append_data.
        header.as_mut_bytes()[..name.len()].copy_from_slice(name.as_bytes());
        header.set_entry_type(*kind);
        header.set_size(data.len() as u64);
        header.set_mode(0o600);
        if kind.is_symlink() || kind.is_hard_link() {
            header.set_link_name("outside.sqlite3").unwrap();
        }
        header.set_cksum();
        builder.append(&header, *data).unwrap();
    }
    zstd::stream::encode_all(builder.into_inner().unwrap().as_slice(), 1).unwrap()
}

fn valid_archive() -> Vec<u8> {
    archive(&[("dataset.sqlite3", tar::EntryType::Regular, DATA)])
}

struct FixtureControl {
    should_stop: Arc<AtomicBool>,
    deadline: Instant,
}

impl FixtureControl {
    fn poll_io<T>(&self, mut operation: impl FnMut() -> io::Result<T>) -> io::Result<T> {
        loop {
            if self.should_stop.load(Ordering::Relaxed) {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "fixture cancelled",
                ));
            }
            if Instant::now() >= self.deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "fixture deadline exceeded",
                ));
            }
            match operation() {
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) =>
                {
                    thread::sleep(Duration::from_millis(2));
                }
                result => return result,
            }
        }
    }

    fn write_all(&self, stream: &mut TcpStream, mut bytes: &[u8]) -> io::Result<()> {
        while !bytes.is_empty() {
            let written = self.poll_io(|| stream.write(bytes))?;
            if written == 0 {
                return Err(io::ErrorKind::WriteZero.into());
            }
            bytes = &bytes[written..];
        }
        Ok(())
    }
}

struct FixtureServer {
    should_stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<io::Result<()>>>,
}

impl FixtureServer {
    fn finish(mut self, client_result: &impl std::fmt::Debug) {
        self.should_stop.store(true, Ordering::Relaxed);
        let result = self.handle.take().unwrap().join();
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error))
                if matches!(
                    error.kind(),
                    io::ErrorKind::Interrupted
                        | io::ErrorKind::BrokenPipe
                        | io::ErrorKind::ConnectionReset
                ) => {}
            other => panic!("fixture failed: {other:?}; client result: {client_result:?}"),
        }
    }
}

impl Drop for FixtureServer {
    fn drop(&mut self) {
        self.should_stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn request(stream: &mut TcpStream, control: &FixtureControl) -> io::Result<()> {
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        let mut byte = [0];
        if control.poll_io(|| stream.read(&mut byte))? == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        bytes.push(byte[0]);
        assert!(bytes.len() < 8192);
    }
    Ok(())
}

fn reply(
    stream: &mut TcpStream,
    body: &[u8],
    declared: usize,
    control: &FixtureControl,
) -> io::Result<()> {
    control.write_all(
        stream,
        format!("HTTP/1.1 200 OK\r\nContent-Length: {declared}\r\nConnection: close\r\n\r\n")
            .as_bytes(),
    )?;
    // Header-bound rejection may close the client socket before it reads the body.
    let _ = control.write_all(stream, body);
    Ok(())
}

fn serve_with(
    handler: impl FnOnce(&mut TcpStream, &TcpListener, &FixtureControl) -> io::Result<()>
        + Send
        + 'static,
) -> (String, FixtureServer) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}/data.tar.zst", listener.local_addr().unwrap());
    let should_stop = Arc::new(AtomicBool::new(false));
    let control = FixtureControl {
        should_stop: Arc::clone(&should_stop),
        deadline: Instant::now() + Duration::from_secs(30),
    };
    let handle = thread::spawn(move || {
        let (mut stream, _) = control.poll_io(|| listener.accept())?;
        stream.set_nonblocking(true)?;
        request(&mut stream, &control)?;
        handler(&mut stream, &listener, &control)
    });
    (
        url,
        FixtureServer {
            should_stop,
            handle: Some(handle),
        },
    )
}

fn serve(body: Vec<u8>, declared: usize) -> (String, FixtureServer) {
    serve_with(move |stream, _, control| reply(stream, &body, declared, control))
}

fn final_path(home: &Path) -> PathBuf {
    home.join("related-terms").join(ID).join("dataset.sqlite3")
}

fn fetch(home: &Path, url: &str, hash: &str, size: u64) -> FetchStatus {
    fetch_dataset(
        home,
        None,
        &DatasetSpec {
            id: ID,
            url,
            size,
            sha256: hash,
        },
    )
}

fn failed(status: FetchStatus, home: &Path, expected: &str) {
    let FetchStatus::Failed { reason } = status else {
        panic!("expected failure: {status:?}")
    };
    assert!(reason.contains(expected), "{expected:?} not in {reason:?}");
    assert!(!final_path(home).exists());
    let directory = final_path(home).parent().unwrap().to_path_buf();
    assert!(!directory.join("download.tar.zst.part").exists());
    assert!(!directory.join("dataset.sqlite3.part").exists());
}

#[test]
fn test_related_terms_fetch_fixture_stops_without_a_connection() {
    let home = tempfile::tempdir().unwrap();
    let blocked = home.path().join("not-a-directory");
    std::fs::write(&blocked, b"file").unwrap();
    let (url, server) = serve(valid_archive(), 0);
    let status = fetch(&blocked, &url, &digest(DATA), DATA.len() as u64);
    let started = Instant::now();
    server.finish(&status);
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "fixture must cancel accept, not wait for its deadline"
    );
    assert!(matches!(status, FetchStatus::Failed { .. }), "{status:?}");
}

#[test]
fn test_related_terms_fetch_cut_connection() {
    let home = tempfile::tempdir().unwrap();
    let body = valid_archive();
    let (url, server) = serve(body[..body.len() / 2].to_vec(), body.len());
    let status = fetch(home.path(), &url, &digest(DATA), DATA.len() as u64);
    server.finish(&status);
    failed(status, home.path(), "body");
}

#[test]
fn test_related_terms_fetch_wrong_hash() {
    let home = tempfile::tempdir().unwrap();
    let body = valid_archive();
    let (url, server) = serve(body.clone(), body.len());
    let status = fetch(home.path(), &url, &digest(b"wrong"), DATA.len() as u64);
    server.finish(&status);
    failed(status, home.path(), "SHA-256 mismatch");
}

#[test]
fn test_related_terms_fetch_rejects_extra_traversal_absolute_links_and_metadata() {
    for (name, kind) in [
        ("extra", tar::EntryType::Regular),
        ("../dataset.sqlite3", tar::EntryType::Regular),
        ("/dataset.sqlite3", tar::EntryType::Regular),
        ("./dataset.sqlite3", tar::EntryType::Regular),
        ("dataset.sqlite3", tar::EntryType::Symlink),
        ("dataset.sqlite3", tar::EntryType::Link),
        ("dataset.sqlite3", tar::EntryType::Directory),
        ("dataset.sqlite3", tar::EntryType::XHeader),
        ("dataset.sqlite3", tar::EntryType::Regular),
    ] {
        let mut cases = vec![vec![
            ("dataset.sqlite3", tar::EntryType::Regular, DATA),
            (name, kind, DATA),
        ]];
        if name != "dataset.sqlite3" || kind != tar::EntryType::Regular {
            cases.push(vec![(name, kind, DATA)]);
        }
        for entries in cases {
            let home = tempfile::tempdir().unwrap();
            let body = archive(&entries);
            let (url, server) = serve(body.clone(), body.len());
            let status = fetch(home.path(), &url, &digest(DATA), DATA.len() as u64);
            server.finish(&status);
            failed(status, home.path(), "only one regular file");
        }
    }
}

#[test]
fn test_related_terms_fetch_expansion_past_size_bound() {
    let home = tempfile::tempdir().unwrap();
    let body = valid_archive();
    let (url, server) = serve(body.clone(), body.len());
    let status = fetch(home.path(), &url, &digest(DATA), DATA.len() as u64 - 1);
    server.finish(&status);
    failed(status, home.path(), "exceeds size bound");
}

#[test]
fn test_related_terms_fetch_download_byte_bound() {
    let home = tempfile::tempdir().unwrap();
    let (url, server) = serve(Vec::new(), MAX_DOWNLOAD_BYTES as usize + 1);
    let status = fetch(home.path(), &url, &digest(DATA), DATA.len() as u64);
    server.finish(&status);
    failed(status, home.path(), "download exceeds byte limit");

    // No Content-Length: the streaming counter must enforce the same limit.
    let (url, server) = serve_with(|stream, _, control| {
        control.write_all(stream, b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n")?;
        let chunk = [0u8; 64 * 1024];
        for _ in 0..=MAX_DOWNLOAD_BYTES / chunk.len() as u64 {
            if control.write_all(stream, &chunk).is_err() {
                break;
            }
        }
        Ok(())
    });
    let status = fetch(home.path(), &url, &digest(DATA), DATA.len() as u64);
    server.finish(&status);
    failed(status, home.path(), "download exceeds byte limit");
}

#[test]
fn test_related_terms_fetch_retry_cleans_stale_files() {
    let home = tempfile::tempdir().unwrap();
    let (url, server) = serve(b"not zstd".to_vec(), 8);
    let status = fetch(home.path(), &url, &digest(DATA), DATA.len() as u64);
    server.finish(&status);
    failed(status, home.path(), "Unknown frame descriptor");
    let directory = final_path(home.path()).parent().unwrap().to_path_buf();
    for name in ["download.tar.zst.part", "dataset.sqlite3.part"] {
        std::fs::write(directory.join(name), b"interrupted prior process").unwrap();
    }
    let body = valid_archive();
    let (url, server) = serve(body.clone(), body.len());
    let status = fetch(home.path(), &url, &digest(DATA), DATA.len() as u64);
    server.finish(&status);
    assert_eq!(
        status,
        FetchStatus::Ready {
            path: final_path(home.path()),
            user_owned: false
        }
    );
    assert_eq!(
        digest(&std::fs::read(final_path(home.path())).unwrap()),
        digest(DATA)
    );
    assert!(!directory.join("download.tar.zst.part").exists());
    assert!(!directory.join("dataset.sqlite3.part").exists());
}

#[test]
fn test_related_terms_fetch_local_and_cached_sources_do_not_download_or_rehash() {
    let home = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let path = user.path().join("dataset.sqlite3");
    std::fs::write(&path, DATA).unwrap();
    let spec = DatasetSpec {
        id: ID,
        url: "invalid URL",
        size: 0,
        sha256: "wrong",
    };
    assert_eq!(
        fetch_dataset(home.path(), Some(&path), &spec),
        FetchStatus::Ready {
            path: path.clone(),
            user_owned: true
        }
    );
    assert_eq!(std::fs::read(&path).unwrap(), DATA);
    assert_eq!(std::fs::read_dir(user.path()).unwrap().count(), 1);
    assert!(!home.path().join("related-terms").exists());
    std::fs::create_dir_all(final_path(home.path()).parent().unwrap()).unwrap();
    std::fs::write(
        final_path(home.path()),
        b"prepared file: indexes changed the hash and size",
    )
    .unwrap();
    assert_eq!(
        fetch_dataset(home.path(), None, &spec),
        FetchStatus::Ready {
            path: final_path(home.path()),
            user_owned: false
        }
    );
    assert!(!final_path(home.path())
        .parent()
        .unwrap()
        .join("download.lock")
        .exists());
}

#[test]
fn test_related_terms_fetch_truncated_zstd_and_trailing_archive() {
    let home = tempfile::tempdir().unwrap();
    let mut body = valid_archive();
    body.truncate(body.len() - 2);
    let (url, server) = serve(body.clone(), body.len());
    let status = fetch(home.path(), &url, &digest(DATA), DATA.len() as u64);
    server.finish(&status);
    failed(status, home.path(), "incomplete frame");

    // An extra entry after the first tar's EOF padding must not be accepted.
    let mut tar = zstd::stream::decode_all(valid_archive().as_slice()).unwrap();
    tar.extend(
        zstd::stream::decode_all(archive(&[("extra", tar::EntryType::Regular, b"")]).as_slice())
            .unwrap(),
    );
    let body = zstd::stream::encode_all(tar.as_slice(), 1).unwrap();
    let (url, server) = serve(body.clone(), body.len());
    let status = fetch(home.path(), &url, &digest(DATA), DATA.len() as u64);
    server.finish(&status);
    failed(status, home.path(), "only one regular file");
}

fn process(home: &Path, url: &str, expected: &str) -> std::process::Child {
    Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "e2e::related_terms_fetch::test_related_terms_fetch_process_worker",
            "--nocapture",
        ])
        .env("RELATED_TERMS_FETCH_TEST_HOME", home)
        .env("RELATED_TERMS_FETCH_TEST_URL", url)
        .env("RELATED_TERMS_FETCH_TEST_STATUS", expected)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}

#[test]
fn test_related_terms_fetch_process_worker() {
    let Some(home) = std::env::var_os("RELATED_TERMS_FETCH_TEST_HOME") else {
        return;
    };
    let url = std::env::var("RELATED_TERMS_FETCH_TEST_URL").unwrap();
    let status = fetch(Path::new(&home), &url, &digest(DATA), DATA.len() as u64);
    match std::env::var("RELATED_TERMS_FETCH_TEST_STATUS")
        .unwrap()
        .as_str()
    {
        "ready" => assert!(
            matches!(
                status,
                FetchStatus::Ready {
                    user_owned: false,
                    ..
                }
            ),
            "{status:?}"
        ),
        "in_progress" => assert_eq!(status, FetchStatus::InProgress),
        other => panic!("unexpected status {other}"),
    }
}

#[test]
fn test_related_terms_fetch_concurrent_processes_download_once() {
    let home = tempfile::tempdir().unwrap();
    let (requested_tx, requested_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (url, server) = serve_with(move |stream, listener, control| {
        requested_tx.send(()).unwrap();
        release_rx.recv_timeout(Duration::from_secs(20)).unwrap();
        let body = valid_archive();
        reply(stream, &body, body.len(), control)?;
        assert!(
            matches!(listener.accept(), Err(error) if error.kind() == io::ErrorKind::WouldBlock),
            "only the lock owner may download"
        );
        Ok(())
    });
    let first = process(home.path(), &url, "ready");
    requested_rx.recv_timeout(Duration::from_secs(20)).unwrap();
    let second = process(home.path(), &url, "in_progress")
        .wait_with_output()
        .unwrap();
    assert!(second.status.success(), "{second:?}");
    assert!(!final_path(home.path()).exists());
    release_tx.send(()).unwrap();
    let first = first.wait_with_output().unwrap();
    server.finish(&first);
    assert!(first.status.success(), "{first:?}");
    assert_eq!(
        digest(&std::fs::read(final_path(home.path())).unwrap()),
        digest(DATA)
    );
    assert!(matches!(
        fetch(home.path(), &url, &digest(DATA), DATA.len() as u64),
        FetchStatus::Ready { .. }
    ));
}
