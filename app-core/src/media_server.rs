use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU16, Ordering};
use std::thread;

use tiny_http::{Header, Response, Server, StatusCode};
use tracing::warn;

static PORT: AtomicU16 = AtomicU16::new(0);

const REMOTE_PREFIX: &str = "/remote/";

fn mime_for_ext(ext: &str) -> &'static str {
    match ext {
        "ogg" | "oga" => "audio/ogg",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "m4a" | "aac" => "audio/mp4",
        "mp4" => "video/mp4",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        _ => "application/octet-stream",
    }
}

fn parse_range(range_header: &str, file_len: u64) -> Option<(u64, u64)> {
    let spec = range_header.strip_prefix("bytes=")?;
    let mut parts = spec.splitn(2, '-');
    let start_str = parts.next().unwrap_or("");
    let end_str = parts.next().unwrap_or("");

    if start_str.is_empty() {
        let suffix: u64 = end_str.parse().ok()?;
        Some((file_len.saturating_sub(suffix), file_len - 1))
    } else {
        let start: u64 = start_str.parse().ok()?;
        let end = if end_str.is_empty() {
            file_len - 1
        } else {
            end_str.parse::<u64>().ok()?.min(file_len - 1)
        };
        Some((start, end))
    }
}

fn handle_request(request: tiny_http::Request) {
    // The renderer hits `/remote/<urlencoded-url>` to stream remote sources
    // (currently just Jellyfin video). We act as a thin proxy so the
    // bearer token never leaves this process and we can speak HTTP Range
    // uniformly with the local file route below.
    if request.url().starts_with(REMOTE_PREFIX) {
        if let Some(rest) = request.url().get(REMOTE_PREFIX.len()..) {
            let decoded = urlencoding::decode(rest)
                .map(|d| d.into_owned())
                .unwrap_or_else(|_| rest.to_string());

            if decoded.starts_with("http://") || decoded.starts_with("https://") {
                handle_remote(request, &decoded);
                return;
            }
        }

        let _ = request.respond(
            Response::from_string("Bad remote URL").with_status_code(StatusCode(400)),
        );
        
        return;
    }

    let raw_path = urlencoding::decode(request.url())
        .map(|d| d.into_owned())
        .unwrap_or_else(|_| request.url().to_string());

    let cleaned = if cfg!(windows) && raw_path.get(1..3).is_some_and(|s| s.as_bytes()[0].is_ascii_alphabetic() && s.as_bytes()[1] == b':') {
        &raw_path[1..]
    } else {
        &raw_path
    };

    let file_path = PathBuf::from(cleaned);

    if !file_path.is_file() {
        let _ = request.respond(
            Response::from_string("Not found").with_status_code(StatusCode(404)),
        );
        return;
    }

    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let mime = mime_for_ext(ext);
    let content_type = Header::from_bytes("Content-Type", mime).unwrap();
    let cors = Header::from_bytes("Access-Control-Allow-Origin", "*").unwrap();
    let accept_ranges = Header::from_bytes("Accept-Ranges", "bytes").unwrap();

    let file_len = match std::fs::metadata(&file_path) {
        Ok(m) => m.len(),
        Err(_) => {
            let _ = request.respond(
                Response::from_string("Read error").with_status_code(StatusCode(500)),
            );
            return;
        }
    };

    let range_val = request
        .headers()
        .iter()
        .find(|h| h.field.as_str() == "Range" || h.field.as_str() == "range")
        .map(|h| h.value.as_str().to_string());

    if let Some(range_str) = range_val {
        if let Some((start, end)) = parse_range(&range_str, file_len) {
            let mut file = match std::fs::File::open(&file_path) {
                Ok(f) => f,
                Err(_) => {
                    let _ = request.respond(
                        Response::from_string("Read error")
                            .with_status_code(StatusCode(500)),
                    );
                    return;
                }
            };

            let chunk_len = (end - start + 1) as usize;
            let mut buf = vec![0u8; chunk_len];
            let _ = file.seek(SeekFrom::Start(start));
            let _ = file.read_exact(&mut buf);

            let content_range = Header::from_bytes(
                "Content-Range",
                format!("bytes {start}-{end}/{file_len}"),
            )
            .unwrap();

            let resp = Response::from_data(buf)
                .with_status_code(StatusCode(206))
                .with_header(content_type)
                .with_header(cors)
                .with_header(accept_ranges)
                .with_header(content_range);

            let _ = request.respond(resp);
            return;
        }
    }

    match std::fs::read(&file_path) {
        Ok(data) => {
            let resp = Response::from_data(data)
                .with_header(content_type)
                .with_header(cors)
                .with_header(accept_ranges);
            let _ = request.respond(resp);
        }
        Err(_) => {
            let _ = request.respond(
                Response::from_string("Read error").with_status_code(StatusCode(500)),
            );
        }
    }
}

/// Proxy a GET against `url` back to the caller, forwarding `Range` so HTML5
/// `<video>`/`<audio>` players can seek. We surface upstream status/headers
/// unchanged where possible.
fn handle_remote(request: tiny_http::Request, url: &str) {
    let agent = ureq::Agent::new_with_defaults();
    let mut req = agent.get(url);
    if let Some(range) = request
        .headers()
        .iter()
        .find(|h| h.field.as_str() == "Range" || h.field.as_str() == "range")
    {
        req = req.header("Range", range.value.as_str());
    }

    let resp = match req.call() {
        Ok(r) => r,
        Err(e) => {
            warn!("[media_server] Remote proxy GET {url} failed: {e}");
            let _ = request.respond(
                Response::from_string("Upstream request failed")
                    .with_status_code(StatusCode(502)),
            );
            return;
        }
    };

    let status = resp.status().as_u16();
    let mut content_type: Option<String> = resp
        .headers()
        .get("content-type")
        .and_then(|h| h.to_str().ok().map(|s| s.to_string()));
    let content_range: Option<String> = resp
        .headers()
        .get("content-range")
        .and_then(|h| h.to_str().ok().map(|s| s.to_string()));
    let accept_ranges_upstream: Option<String> = resp
        .headers()
        .get("accept-ranges")
        .and_then(|h| h.to_str().ok().map(|s| s.to_string()));

    if content_type.is_none() {
        content_type = Some("application/octet-stream".into());
    }

    let mut body = resp.into_body();
    let mut reader = body.as_reader();
    let mut bytes = Vec::new();
    if let Err(e) = reader.read_to_end(&mut bytes) {
        warn!("[media_server] Remote proxy body read failed: {e}");
        let _ = request.respond(
            Response::from_string("Upstream read failed").with_status_code(StatusCode(502)),
        );
        return;
    }

    let mut response = Response::from_data(bytes).with_status_code(StatusCode(status));
    if let Some(ct) = content_type {
        if let Ok(h) = Header::from_bytes("Content-Type", ct) {
            response = response.with_header(h);
        }
    }
    if let Some(cr) = content_range {
        if let Ok(h) = Header::from_bytes("Content-Range", cr) {
            response = response.with_header(h);
        }
    }
    if let Some(ar) = accept_ranges_upstream {
        if let Ok(h) = Header::from_bytes("Accept-Ranges", ar) {
            response = response.with_header(h);
        }
    } else if let Ok(h) = Header::from_bytes("Accept-Ranges", "bytes") {
        response = response.with_header(h);
    }
    if let Ok(h) = Header::from_bytes("Access-Control-Allow-Origin", "*") {
        response = response.with_header(h);
    }
    let _ = request.respond(response);
}

pub fn start() -> u16 {
    let server = Server::http("127.0.0.1:0").expect("failed to start media server");
    let port = server.server_addr().to_ip().unwrap().port();
    PORT.store(port, Ordering::SeqCst);

    thread::spawn(move || {
        for request in server.incoming_requests() {
            thread::spawn(move || {
                handle_request(request);
            });
        }
    });

    port
}

pub fn port() -> u16 {
    PORT.load(Ordering::SeqCst)
}
