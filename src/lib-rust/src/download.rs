use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{misc, Error};

/// A single HTTP header (name and value pair) to be sent with a web request.
#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct HttpHeader {
    /// The name of the HTTP header (eg. "Authorization").
    pub Name: String,
    /// The value of the HTTP header.
    pub Value: String,
}

/// Options to customize HTTP requests (custom headers, timeout, etc).
#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct HttpOptions {
    /// Additional headers to send with each request.
    pub Headers: Vec<HttpHeader>,
    /// Timeout applied to the entire request (connection + transfer), in milliseconds.
    /// The default of 0 means requests never time out.
    pub TimeoutMilliseconds: u64,
}

/// Downloads a file from a URL and writes it to a file while reporting progress from 0-100.
pub fn download_url_to_file<A, S: AsRef<Path>>(url: &str, file_path: S, options: Option<&HttpOptions>, mut progress: A) -> Result<(), Error>
where
    A: FnMut(i16),
{
    let file_path = file_path.as_ref();
    let agent = get_download_agent(options)?;
    let mut req = agent.get(url);
    if let Some(options) = options {
        for header in &options.Headers {
            req = req.header(&header.Name, &header.Value);
        }
    }
    let (head, body) = req.call()?.into_parts();

    let total_size = head
        .headers
        .get("Content-Length")
        .and_then(|s| s.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());
    let mut file = misc::retry_io(|| File::create(file_path))?;

    const CHUNK_SIZE: usize = 2 * 1024 * 1024; // 2MB
    let mut downloaded: u64 = 0;
    let mut buffer = vec![0; CHUNK_SIZE];
    let mut reader = body.into_reader();

    let mut last_progress = 0;

    loop {
        let size = reader.read(&mut buffer)?; // Explicitly propagate errors
        if size == 0 {
            break; // End of stream
        }
        file.write_all(&buffer[..size])?;
        downloaded += size as u64;

        if let Some(total) = total_size {
            // floor to nearest 5% to reduce message spam
            let new_progress = (downloaded as f64 / total as f64 * 20.0).floor() as i16 * 5;
            if new_progress > last_progress {
                last_progress = new_progress;
                progress(last_progress);
            }
        }
    }

    Ok(())
}

/// Downloads a file from a URL and returns it as a string.
pub fn download_url_as_string(url: &str, options: Option<&HttpOptions>) -> Result<String, Error> {
    let agent = get_download_agent(options)?;
    let mut req = agent.get(url);
    if let Some(options) = options {
        for header in &options.Headers {
            req = req.header(&header.Name, &header.Value);
        }
    }
    let r = req.call()?.body_mut().read_to_string()?;
    Ok(r)
}

/// Downloads a URL to a file like [`download_url_to_file`], but can be stopped: `progress` gets the bytes
/// received so far and the total when the server gives one, at least a few times a second, and the
/// download ends with `Ok(false)` as soon as it returns false. A connection that delivers nothing for
/// a minute is an error instead of a wait without end.
pub fn download_url_to_file_while<A, S: AsRef<Path>>(url: &str, file_path: S, mut progress: A) -> Result<bool, Error>
where
    A: FnMut(u64, Option<u64>) -> bool,
{
    enum Part {
        Total(Option<u64>),
        Data(Vec<u8>),
        Done,
        Failed(Error),
    }
    const STALL: Duration = Duration::from_secs(60);
    const POLL: Duration = Duration::from_millis(200);

    // The request runs on its own thread: a read that never returns must not keep the caller from
    // seeing a cancel. When the caller leaves early the thread ends with its next send.
    let (tx, rx) = std::sync::mpsc::sync_channel::<Part>(4);
    let url = url.to_string();
    std::thread::spawn(move || {
        let result = (|| -> Result<(), Error> {
            let (head, body) = get_download_agent(None)?.get(&url).call()?.into_parts();
            let total = head.headers.get("Content-Length").and_then(|s| s.to_str().ok()).and_then(|s| s.parse::<u64>().ok());
            if tx.send(Part::Total(total)).is_err() {
                return Ok(());
            }
            let mut reader = body.into_reader();
            loop {
                let mut buffer = vec![0; 1024 * 1024];
                let size = reader.read(&mut buffer)?;
                if size == 0 {
                    return Ok(());
                }
                buffer.truncate(size);
                if tx.send(Part::Data(buffer)).is_err() {
                    return Ok(());
                }
            }
        })();
        let _ = tx.send(match result {
            Ok(()) => Part::Done,
            Err(e) => Part::Failed(e),
        });
    });

    let mut file = misc::retry_io(|| File::create(file_path.as_ref()))?;
    let mut downloaded: u64 = 0;
    let mut total = None;
    let mut last_data = std::time::Instant::now();
    loop {
        match rx.recv_timeout(POLL) {
            Ok(Part::Total(value)) => total = value,
            Ok(Part::Data(buffer)) => {
                file.write_all(&buffer)?;
                downloaded += buffer.len() as u64;
                last_data = std::time::Instant::now();
            }
            Ok(Part::Done) => return Ok(true),
            Ok(Part::Failed(e)) => return Err(e),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if last_data.elapsed() > STALL {
                    return Err(Error::Other(format!("The download received nothing for {} seconds.", STALL.as_secs())));
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return Err(Error::Other("The download stopped unexpectedly.".into())),
        }
        if !progress(downloaded, total) {
            return Ok(false);
        }
    }
}

fn get_download_agent(options: Option<&HttpOptions>) -> Result<ureq::Agent, Error> {
    // let tls_builder = native_tls::TlsConnector::builder();
    // let tls_connector = tls_builder.build()?;
    // Ok(ureq::AgentBuilder::new().tls_connector(tls_connector.into()).build())
    let mut config = ureq::Agent::config_builder();
    let timeout_ms = options.map(|o| o.TimeoutMilliseconds).unwrap_or(0);
    if timeout_ms > 0 {
        config = config.timeout_global(Some(Duration::from_millis(timeout_ms)));
    }
    Ok(config.build().into())
}

#[test]
fn test_download_uses_tls_and_encoding_correctly() {
    assert_eq!(
        download_url_as_string("https://dotnetcli.blob.core.windows.net/dotnet/WindowsDesktop/5.0/latest.version", None).unwrap(),
        "5.0.17"
    );
}

#[test]
fn test_download_file_reports_progress() {
    // https://www.ip-toolbox.com/speedtest-files/
    //"https://proof.ovh.net/files/10Mb.dat"
    let test_file = "https://velopacktesting.blob.core.windows.net/ci-test-files/10Mb.dat";
    let mut prog_count = 0;
    let mut last_prog = 0;

    let tmpfile = tempfile::NamedTempFile::new().unwrap();
    let tmppath = tmpfile.path();

    download_url_to_file(test_file, Path::new(tmppath), None, |p| {
        assert!(p >= last_prog);
        prog_count += 1;
        last_prog = p;
    })
    .unwrap();

    assert!(prog_count >= 4);
    assert!(prog_count <= 20);
    assert_eq!(last_prog, 100);

    let meta = tmppath.metadata().unwrap();
    let len = meta.len();

    assert_eq!(len, 10 * 1024 * 1024);
}

#[test]
fn test_interrupted_download() {
    use std::io::Write;
    use std::net::TcpListener;
    use std::thread;

    // Start a simple HTTP server that cuts the connection mid-download
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind test server");
    let addr = listener.local_addr().unwrap();

    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            // Write a partial HTTP response
            let response = "HTTP/1.1 200 OK\r\nContent-Length: 100000\r\n\r\n";
            stream.write_all(response.as_bytes()).expect("Failed to write response");

            // Send part of the data, then close the connection early
            let partial_data = vec![0u8; 1024]; // 1 KB
            stream.write_all(&partial_data).expect("Failed to write partial data");

            // Connection closes here, simulating an interrupted download
            thread::sleep(std::time::Duration::from_millis(100));
        }
    });

    let tmpfile = tempfile::NamedTempFile::new().unwrap();
    let result = download_url_to_file(&format!("http://{}", addr), tmpfile.path(), None, |_| {});

    assert!(result.is_err(), "Download should fail due to connection interruption");
}

#[test]
fn test_successful_download() {
    use std::io::Write;
    use std::net::TcpListener;
    use std::thread;

    // Start a simple HTTP server
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind test server");
    let addr = listener.local_addr().unwrap();

    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            // Write a full HTTP response with full content
            let response = "HTTP/1.1 200 OK\r\nContent-Length: 10240\r\n\r\n";
            stream.write_all(response.as_bytes()).expect("Failed to write response");

            // Send full 10KB of data
            let full_data = vec![0u8; 10240];
            stream.write_all(&full_data).expect("Failed to write full data");

            // give client time to receive and disconnect
            thread::sleep(std::time::Duration::from_millis(100));
        }
    });

    let tmpfile = tempfile::NamedTempFile::new().unwrap();
    let _ = download_url_to_file(&format!("http://{}", addr), tmpfile.path(), None, |_| {}).unwrap();

    // Verify that the downloaded file has the expected size
    let metadata = tmpfile.path().metadata().unwrap();
    assert_eq!(metadata.len(), 10240, "Downloaded file size should match the expected content size");
}

#[test]
fn test_download_while_reports_bytes_and_stops_when_asked() {
    let tmp = tempfile::TempDir::new().unwrap();
    let file = tmp.path().join("file");
    let url = "https://dotnetcli.blob.core.windows.net/dotnet/WindowsDesktop/5.0/latest.version";
    let mut seen = 0;
    assert!(download_url_to_file_while(url, &file, |done, _| {
        seen = done;
        true
    })
    .unwrap());
    assert_eq!(std::fs::read_to_string(&file).unwrap().trim(), "5.0.17");
    assert_eq!(seen, std::fs::metadata(&file).unwrap().len());
    assert!(!download_url_to_file_while(url, &file, |_, _| false).unwrap());
}
