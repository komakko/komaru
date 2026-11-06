use std::io::Read;
use std::sync::LazyLock;
use std::time::Duration;

use ureq::Agent;

use crate::core::limits;
use crate::error::FetchError;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_REDIRECTS: u32 = 5;
const CHUNK_BYTES: usize = 64 * 1024;

static AGENT: LazyLock<Agent> = LazyLock::new(|| {
    Agent::config_builder()
        .timeout_resolve(Some(CONNECT_TIMEOUT))
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_recv_response(Some(RESPONSE_TIMEOUT))
        .max_redirects(MAX_REDIRECTS)
        .user_agent(concat!(
            env!("CARGO_PKG_NAME"),
            "/",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .into()
});

pub fn fetch(
    url: &str,
    is_cancelled: impl Fn() -> bool,
    on_progress: impl FnMut(u64, Option<u64>),
) -> Result<Vec<u8>, FetchError> {
    fetch_up_to(url, limits::encoded(), is_cancelled, on_progress)
}

pub fn fetch_up_to(
    url: &str,
    limit: u64,
    is_cancelled: impl Fn() -> bool,
    mut on_progress: impl FnMut(u64, Option<u64>),
) -> Result<Vec<u8>, FetchError> {
    let mut response = AGENT.get(url).call()?;
    let total = response.body().content_length();
    if total.is_some_and(|length| length > limit) {
        return Err(FetchError::TooLarge { limit });
    }
    let mut reader = response
        .body_mut()
        .with_config()
        .limit(limit.saturating_add(1))
        .reader();
    let mut bytes = Vec::new();
    let mut chunk = vec![0u8; CHUNK_BYTES];
    loop {
        if is_cancelled() {
            return Err(FetchError::Cancelled);
        }
        let read = reader.read(&mut chunk)?;
        if read == 0 {
            return Ok(bytes);
        }
        if (bytes.len() + read) as u64 > limit {
            return Err(FetchError::TooLarge { limit });
        }
        bytes.extend_from_slice(&chunk[..read]);
        on_progress(bytes.len() as u64, total);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::{TcpListener, TcpStream};
    use std::thread;

    fn serve_once(respond: impl FnOnce(&mut TcpStream) + Send + 'static) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 2048];
            let _ = stream.read(&mut request);
            respond(&mut stream);
        });
        format!("http://{address}/image.png")
    }

    fn ok_response(body: &[u8]) -> Vec<u8> {
        let mut response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .into_bytes();
        response.extend_from_slice(body);
        response
    }

    fn never() -> bool {
        false
    }

    fn ignore_progress(_received: u64, _total: Option<u64>) {}

    #[test]
    fn downloads_body() {
        let url = serve_once(|stream| {
            stream.write_all(&ok_response(&[9, 8, 7])).unwrap();
        });
        assert_eq!(fetch(&url, never, ignore_progress).unwrap(), vec![9, 8, 7]);
    }

    #[test]
    fn reports_download_progress() {
        let url = serve_once(|stream| {
            stream.write_all(&ok_response(&[0; 100])).unwrap();
        });
        let mut reports = Vec::new();
        fetch(&url, never, |received, total| {
            reports.push((received, total))
        })
        .unwrap();
        assert_eq!(reports.last(), Some(&(100, Some(100))));
    }

    #[test]
    fn reports_http_status_errors() {
        let url = serve_once(|stream| {
            stream
                .write_all(
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .unwrap();
        });
        assert!(matches!(
            fetch(&url, never, ignore_progress),
            Err(FetchError::Transport(_))
        ));
    }

    #[test]
    fn rejects_oversized_declared_length() {
        let url = serve_once(|stream| {
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                limits::DEFAULT_ENCODED_BYTES + 1
            );
            stream.write_all(header.as_bytes()).unwrap();
        });
        assert!(matches!(
            fetch(&url, never, ignore_progress),
            Err(FetchError::TooLarge { .. })
        ));
    }

    #[test]
    fn stops_when_cancelled() {
        let url = serve_once(|stream| {
            stream.write_all(&ok_response(&[1, 2, 3])).unwrap();
        });
        assert!(matches!(
            fetch(&url, || true, ignore_progress),
            Err(FetchError::Cancelled)
        ));
    }

    #[test]
    fn gives_up_on_redirect_loops() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                let mut request = [0u8; 2048];
                let _ = stream.read(&mut request);
                let response = format!(
                    "HTTP/1.1 302 Found\r\nLocation: http://{address}/again\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        assert!(matches!(
            fetch(&format!("http://{address}/start"), never, ignore_progress),
            Err(FetchError::Transport(_))
        ));
    }

    #[test]
    fn rejects_malformed_addresses() {
        assert!(matches!(
            fetch("http://", never, ignore_progress),
            Err(FetchError::Transport(_))
        ));
    }
}
