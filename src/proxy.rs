use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(30);
const MAX_HEADER: usize = 16 * 1024;
const MAX_BODY: u64 = 64 * 1024 * 1024;
const MAX_CLIENTS: usize = 64;
const COPY_CHUNK: usize = 64 * 1024;

/// Live state of package responses being written to consoles.
///
/// `bytes` is a monotonic total so a reader can derive throughput from two
/// samples without trusting wall-clock time. Per-file `served` is the highest
/// byte offset reached, so retried or resumed ranges never inflate progress.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TransferState {
    pub active: Vec<ActiveTransfer>,
    pub files: HashMap<String, FileTransfer>,
    pub bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveTransfer {
    pub id: u64,
    pub peer: SocketAddr,
    pub url: String,
    pub file: String,
    pub sent: u64,
    pub total: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FileTransfer {
    pub size: u64,
    pub served: u64,
    pub complete: bool,
    pub interrupted: bool,
}

#[derive(Clone)]
pub struct Transfers {
    state: Arc<Mutex<TransferState>>,
    next_id: Arc<AtomicU64>,
}

impl Default for Transfers {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(TransferState::default())),
            next_id: Arc::new(AtomicU64::new(1)),
        }
    }
}

impl Transfers {
    pub fn snapshot(&self) -> TransferState {
        self.state
            .lock()
            .map(|state| state.clone())
            .unwrap_or_default()
    }

    fn begin(&self, peer: SocketAddr, url: &str, file: &str, size: u64, total: u64) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut state) = self.state.lock() {
            let entry = state.files.entry(file.to_owned()).or_default();
            entry.size = size;
            // A fresh request retries an interrupted part.
            entry.interrupted = false;
            state.active.push(ActiveTransfer {
                id,
                peer,
                url: url.to_owned(),
                file: file.to_owned(),
                sent: 0,
                total,
            });
        }
        id
    }

    fn progress(&self, id: u64, sent: u64, served: u64, file: &str) {
        if let Ok(mut state) = self.state.lock() {
            let mut delta = 0;
            if let Some(active) = state.active.iter_mut().find(|active| active.id == id) {
                delta = sent.saturating_sub(active.sent);
                active.sent = sent;
            }
            state.bytes = state.bytes.saturating_add(delta);
            if let Some(entry) = state.files.get_mut(file) {
                // shortcut: highest offset reached; retried and resumed ranges
                // never inflate progress, but a tail-only range still counts.
                entry.served = entry.served.max(served);
            }
        }
    }

    fn end(&self, id: u64, file: &str, interrupted: bool) {
        if let Ok(mut state) = self.state.lock() {
            state.active.retain(|active| active.id != id);
            if let Some(entry) = state.files.get_mut(file) {
                if interrupted {
                    entry.interrupted = entry.served < entry.size;
                } else if entry.size > 0 && entry.served >= entry.size {
                    entry.complete = true;
                    entry.interrupted = false;
                }
            }
        }
    }
}

/// Marks a request interrupted unless the caller reached its stated length.
struct TransferGuard<'a> {
    transfers: &'a Transfers,
    id: u64,
    file: &'a str,
    finished: bool,
}

impl Drop for TransferGuard<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.transfers.end(self.id, self.file, true);
        }
    }
}

pub struct Proxy {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    pub address: SocketAddr,
    sockets: Arc<Mutex<HashMap<usize, Vec<TcpStream>>>>,
    pub events: Receiver<String>,
    pub transfers: Transfers,
    package_dir: Arc<Mutex<Option<PathBuf>>>,
}

impl Proxy {
    pub fn start(address: SocketAddr, package_dir: Option<PathBuf>) -> io::Result<Self> {
        if let Some(dir) = &package_dir {
            if !dir.is_dir() {
                return Err(invalid("Choose an existing package folder"));
            }
        }
        let std::net::IpAddr::V4(bind_ip) = address.ip() else {
            return Err(invalid("IPv4 bind required"));
        };
        if address.port() == 0 || bind_ip.is_multicast() || bind_ip.is_broadcast() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Enter an IPv4 address and a nonzero port",
            ));
        }
        let subnets = local_subnets()?;
        if subnets.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "No local IPv4 subnet detected; refusing to start",
            ));
        }
        let listener = TcpListener::bind(address)?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let stop = Arc::new(AtomicBool::new(false));
        let sockets = Arc::new(Mutex::new(HashMap::new()));
        let package_dir = Arc::new(Mutex::new(package_dir));
        let transfers = Transfers::default();
        let (event_sender, events) = mpsc::sync_channel(256);
        let thread_stop = stop.clone();
        let thread_sockets = sockets.clone();
        let thread_package_dir = package_dir.clone();
        let thread_transfers = transfers.clone();
        let thread = thread::spawn(move || {
            serve(
                listener,
                subnets,
                thread_stop,
                thread_sockets,
                event_sender,
                thread_package_dir,
                thread_transfers,
            )
        });
        Ok(Self {
            stop,
            thread: Some(thread),
            address,
            sockets,
            events,
            transfers,
            package_dir,
        })
    }

    pub fn set_package_dir(&self, dir: PathBuf) -> io::Result<()> {
        if !dir.is_dir() {
            return Err(invalid("Choose an existing package folder"));
        }
        *self
            .package_dir
            .lock()
            .map_err(|_| invalid("Proxy stopping"))? = Some(dir);
        Ok(())
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        if let Ok(mut sockets) = self.sockets.lock() {
            for streams in sockets.values() {
                for socket in streams {
                    let _ = socket.shutdown(Shutdown::Both);
                }
            }
            sockets.clear();
        }
    }
}

impl Drop for Proxy {
    fn drop(&mut self) {
        self.stop();
    }
}

fn local_subnets() -> io::Result<Vec<(u32, u32)>> {
    let interfaces = if_addrs::get_if_addrs()?;
    Ok(interfaces
        .into_iter()
        .filter_map(|interface| {
            let if_addrs::IfAddr::V4(v4) = interface.addr else {
                return None;
            };
            if !v4.ip.is_private() {
                return None;
            }
            let mask = u32::from(v4.netmask);
            // Restrict discovery to contiguous /16-or-narrower subnets.
            (mask >= u32::from(Ipv4Addr::new(255, 255, 0, 0))
                && (!mask) & ((!mask).wrapping_add(1)) == 0)
                .then_some((u32::from(v4.ip) & mask, mask))
        })
        .collect())
}

fn serve(
    listener: TcpListener,
    subnets: Vec<(u32, u32)>,
    stop: Arc<AtomicBool>,
    sockets: Arc<Mutex<HashMap<usize, Vec<TcpStream>>>>,
    events: SyncSender<String>,
    package_dir: Arc<Mutex<Option<PathBuf>>>,
    transfers: Transfers,
) {
    let active = Arc::new(AtomicUsize::new(0));
    let next_id = AtomicUsize::new(0);
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((mut client, peer)) => {
                if !allowed_peer(peer, &subnets) || active.load(Ordering::Relaxed) >= MAX_CLIENTS {
                    let _ = client.shutdown(Shutdown::Both);
                    continue;
                }
                let Ok(registered) = client.try_clone() else {
                    continue;
                };
                let id = next_id.fetch_add(1, Ordering::Relaxed);
                if let Ok(mut clients) = sockets.lock() {
                    clients.insert(id, vec![registered]);
                } else {
                    continue;
                }
                active.fetch_add(1, Ordering::Relaxed);
                let _ = events.try_send(format!("Device connected: {peer}"));
                let active = active.clone();
                let sockets = sockets.clone();
                let events = events.clone();
                let package_dir = package_dir.clone();
                let transfers = transfers.clone();
                thread::spawn(move || {
                    let _ = client.set_read_timeout(Some(TIMEOUT));
                    let _ = client.set_write_timeout(Some(TIMEOUT));
                    let selected_dir = package_dir.lock().ok().and_then(|dir| dir.clone());
                    if let Err(error) = handle(
                        &mut client,
                        &sockets,
                        id,
                        peer,
                        &events,
                        selected_dir.as_deref(),
                        &transfers,
                    ) {
                        let status = if error.kind() == io::ErrorKind::PermissionDenied {
                            403
                        } else {
                            400
                        };
                        let _ = write!(client, "HTTP/1.1 {status} Error\r\nConnection: close\r\nContent-Length: 0\r\n\r\n");
                    }
                    if let Ok(mut clients) = sockets.lock() {
                        clients.remove(&id);
                    }
                    active.fetch_sub(1, Ordering::Relaxed);
                });
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50))
            }
            Err(_) => break,
        }
    }
}

fn allowed_peer(peer: SocketAddr, subnets: &[(u32, u32)]) -> bool {
    match peer.ip() {
        std::net::IpAddr::V4(ip) => subnets
            .iter()
            .any(|(network, mask)| u32::from(ip) & mask == *network),
        _ => false,
    }
}

fn read_header(client: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut data = Vec::new();
    let mut byte = [0u8; 1];
    while data.len() < MAX_HEADER {
        client.read_exact(&mut byte)?;
        data.push(byte[0]);
        if data.ends_with(b"\r\n\r\n") {
            return Ok(data);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "Request headers too large",
    ))
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
fn forbidden(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, message)
}

fn log_request(events: &SyncSender<String>, peer: SocketAddr, method: &str, target: &str) {
    let target = if method == "CONNECT" {
        if target.contains('@') || target.contains('%') {
            "[invalid authority]".to_string()
        } else {
            target.to_string()
        }
    } else if let Ok(mut url) = url::Url::parse(target) {
        if !url.username().is_empty() || url.password().is_some() {
            "[URL with credentials]".to_string()
        } else {
            url.set_query(None);
            url.set_fragment(None);
            url.into()
        }
    } else {
        "[invalid URL]".to_string()
    };
    let _ = events.try_send(format!("{peer} {method} {target}"));
}

fn destination(host: &str, port: u16) -> io::Result<TcpStream> {
    if host.is_empty() || host.contains('@') || host.contains('%') {
        return Err(invalid("Invalid host"));
    }
    let addresses = (host, port).to_socket_addrs()?;
    for address in addresses {
        let std::net::IpAddr::V4(ip) = address.ip() else {
            continue;
        };
        if !public_ip(ip) {
            continue;
        }
        if let Ok(stream) = TcpStream::connect_timeout(&address, Duration::from_secs(5)) {
            stream.set_read_timeout(Some(TIMEOUT))?;
            stream.set_write_timeout(Some(TIMEOUT))?;
            return Ok(stream);
        }
    }
    Err(forbidden("No reachable public IPv4 destination"))
}

fn public_ip(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(a == 0
        || a == 10
        || a == 127
        || a >= 224
        || a == 169 && b == 254
        || a == 172 && (16..=31).contains(&b)
        || a == 192 && b == 168
        || a == 100 && (64..=127).contains(&b)
        || a == 192 && b == 0 && c == 0
        || a == 192 && b == 0 && c == 2
        || a == 198 && (b == 18 || b == 19 || b == 51 && c == 100)
        || a == 203 && b == 0 && c == 113)
}

fn handle(
    client: &mut TcpStream,
    sockets: &Mutex<HashMap<usize, Vec<TcpStream>>>,
    id: usize,
    peer: SocketAddr,
    events: &SyncSender<String>,
    package_dir: Option<&Path>,
    transfers: &Transfers,
) -> io::Result<()> {
    let header = read_header(client)?;
    let text = std::str::from_utf8(&header).map_err(|_| invalid("Invalid header encoding"))?;
    let mut lines = text.split("\r\n");
    let request = lines
        .next()
        .ok_or_else(|| invalid("Missing request line"))?;
    if !request.is_ascii() || request.contains('\t') || request.contains("  ") {
        return Err(invalid("Malformed request line"));
    }
    let parts: Vec<_> = request.split(' ').collect();
    if parts.len() != 3 || parts[2] != "HTTP/1.1" {
        return Err(invalid("Unsupported request"));
    }
    let mut host_header = None;
    let mut length = None;
    let mut range = None;
    for line in lines.take_while(|line| !line.is_empty()) {
        let (key, value) = line
            .split_once(':')
            .ok_or_else(|| invalid("Malformed header"))?;
        if key.is_empty()
            || !key
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            || value.contains('\r')
            || value.contains('\n')
        {
            return Err(invalid("Malformed header"));
        }
        if key.eq_ignore_ascii_case("host") && host_header.replace(value.trim()).is_some() {
            return Err(invalid("Duplicate Host header"));
        }
        if key.eq_ignore_ascii_case("transfer-encoding")
            || key.eq_ignore_ascii_case("proxy-authorization")
            || key.eq_ignore_ascii_case("upgrade")
        {
            return Err(invalid("Unsupported header"));
        }
        if key.eq_ignore_ascii_case("range") && range.replace(value.trim()).is_some() {
            return Err(invalid("Duplicate Range"));
        }
        if key.eq_ignore_ascii_case("content-length") {
            if length.is_some() {
                return Err(invalid("Duplicate Content-Length"));
            }
            length = Some(
                value
                    .trim()
                    .parse::<u64>()
                    .map_err(|_| invalid("Invalid Content-Length"))?,
            );
        }
    }
    let host_header = host_header.ok_or_else(|| invalid("Host header required"))?;
    if host_header.is_empty() || length.unwrap_or(0) > MAX_BODY {
        return Err(invalid("Host header or body size invalid"));
    }
    if parts[0] == "CONNECT" {
        if length.unwrap_or(0) != 0 {
            return Err(invalid("CONNECT body not supported"));
        }
        let (host, port) = parts[1]
            .rsplit_once(':')
            .ok_or_else(|| invalid("Invalid CONNECT target"))?;
        if host.is_empty() || host_header != parts[1] {
            return Err(invalid("Host must match CONNECT authority"));
        }
        if port != "443" {
            return Err(forbidden("Only HTTPS port 443 is allowed"));
        }
        log_request(events, peer, "CONNECT", parts[1]);
        let upstream = destination(host, 443)?;
        let mut active = sockets.lock().map_err(|_| invalid("Proxy stopping"))?;
        let streams = active
            .get_mut(&id)
            .ok_or_else(|| invalid("Proxy stopping"))?;
        streams.push(upstream.try_clone()?);
        drop(active);
        client.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")?;
        tunnel(client, upstream)
    } else {
        log_request(events, peer, parts[0], parts[1]);
        if let Some(dir) = package_dir {
            if serve_package(
                client,
                Request {
                    method: parts[0],
                    target: parts[1],
                    host: host_header,
                    range,
                },
                dir,
                events,
                peer,
                transfers,
            )? {
                return Ok(());
            }
        }
        forward_http(
            client,
            parts[0],
            parts[1],
            host_header,
            &header,
            length.unwrap_or(0),
        )
    }
}

/// Request parts a locally served package response needs.
struct Request<'a> {
    method: &'a str,
    target: &'a str,
    host: &'a str,
    range: Option<&'a str>,
}

fn serve_package(
    client: &mut TcpStream,
    request: Request<'_>,
    dir: &Path,
    events: &SyncSender<String>,
    peer: SocketAddr,
    transfers: &Transfers,
) -> io::Result<bool> {
    let Request {
        method,
        target,
        host,
        range,
    } = request;
    if !matches!(method, "GET" | "HEAD") {
        return Ok(false);
    }
    let Ok(url) = url::Url::parse(target) else {
        return Ok(false);
    };
    if url.scheme() != "http"
        || url.host_str() != Some("gst.prod.dl.playstation.net")
        || !host.eq_ignore_ascii_case("gst.prod.dl.playstation.net")
        || url.port_or_known_default() != Some(80)
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Ok(false);
    }
    let Some(name) = url.path_segments().and_then(|mut parts| parts.next_back()) else {
        return Ok(false);
    };
    if !name.ends_with(".pkg")
        || name.is_empty()
        || name == ".pkg"
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        return Ok(false);
    }
    let path = dir.join(name);
    let Ok(metadata) = fs::symlink_metadata(&path) else {
        return Ok(false);
    };
    if !metadata.file_type().is_file() {
        return Ok(false);
    }
    let mut file = File::open(&path)?;
    let size = file.metadata()?.len();
    let (start, end, partial) = if let Some(value) = range {
        let Some(spec) = value.strip_prefix("bytes=") else {
            return Err(invalid("Unsupported Range"));
        };
        let Some((first, last)) = spec.split_once('-') else {
            return Err(invalid("Invalid Range"));
        };
        if first.is_empty() {
            return Err(invalid("Suffix Range unsupported"));
        }
        let start = first.parse::<u64>().map_err(|_| invalid("Invalid Range"))?;
        let end = if last.is_empty() {
            size.saturating_sub(1)
        } else {
            last.parse::<u64>().map_err(|_| invalid("Invalid Range"))?
        };
        if start >= size || end < start || end >= size {
            return Err(invalid("Range outside file"));
        }
        (start, end, true)
    } else {
        (0, size.saturating_sub(1), false)
    };
    let length = if partial { end - start + 1 } else { size };
    let status = if partial {
        "206 Partial Content"
    } else {
        "200 OK"
    };
    write!(client, "HTTP/1.1 {status}\r\nContent-Type: application/octet-stream\r\nAccept-Ranges: bytes\r\nContent-Length: {length}\r\n")?;
    if partial {
        write!(client, "Content-Range: bytes {start}-{end}/{size}\r\n")?;
    }
    client.write_all(b"Connection: close\r\n\r\n")?;
    if method == "GET" {
        // Track only streaming responses; a HEAD probe carries no bytes.
        let id = transfers.begin(peer, target, name, size, length);
        let mut guard = TransferGuard {
            transfers,
            id,
            file: name,
            finished: false,
        };
        file.seek(SeekFrom::Start(start))?;
        let mut buffer = vec![0u8; COPY_CHUNK.min(length.max(1) as usize)];
        let mut sent = 0u64;
        while sent < length {
            let want = (length - sent).min(buffer.len() as u64) as usize;
            let read = file.read(&mut buffer[..want])?;
            if read == 0 {
                break;
            }
            client.write_all(&buffer[..read])?;
            sent += read as u64;
            transfers.progress(id, sent, start + sent, name);
        }
        if sent < length {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Package file ended early",
            ));
        }
        transfers.end(id, name, false);
        guard.finished = true;
    }
    let _ = events.try_send(format!("Served local PKG: {name} ({length} bytes)"));
    Ok(true)
}

fn forward_http(
    client: &mut TcpStream,
    method: &str,
    target: &str,
    host_header: &str,
    header: &[u8],
    body_len: u64,
) -> io::Result<()> {
    if !matches!(
        method,
        "GET" | "HEAD" | "POST" | "PUT" | "PATCH" | "DELETE" | "OPTIONS"
    ) {
        return Err(invalid("Unsupported method"));
    }
    let url = url::Url::parse(target).map_err(|_| invalid("Absolute HTTP URL required"))?;
    if url.scheme() != "http"
        || url.username() != ""
        || url.password().is_some()
        || url.port_or_known_default() != Some(80)
    {
        return Err(forbidden("Only public HTTP port 80 is allowed"));
    }
    let host = url.host_str().ok_or_else(|| invalid("Missing URL host"))?;
    if !host_header.eq_ignore_ascii_case(host)
        && !host_header.eq_ignore_ascii_case(&format!("{host}:80"))
    {
        return Err(invalid("Host must match HTTP target"));
    }
    if !host.is_ascii() || host.contains(':') {
        return Err(invalid("Only IPv4 or DNS hosts are supported"));
    }
    let mut upstream = destination(host, 80)?;
    let mut path = url.path().to_string();
    if let Some(query) = url.query() {
        path.push('?');
        path.push_str(query);
    }
    write!(upstream, "{method} {path} HTTP/1.1\r\n")?;
    let text = std::str::from_utf8(header).map_err(|_| invalid("Invalid header encoding"))?;
    for line in text
        .split("\r\n")
        .skip(1)
        .take_while(|line| !line.is_empty())
    {
        let key = line
            .split_once(':')
            .ok_or_else(|| invalid("Malformed header"))?
            .0;
        if key.eq_ignore_ascii_case("connection")
            || key.eq_ignore_ascii_case("proxy-connection")
            || key.eq_ignore_ascii_case("keep-alive")
            || key.eq_ignore_ascii_case("host")
        {
            continue;
        }
        write!(upstream, "{line}\r\n")?;
    }
    write!(upstream, "Host: {host}\r\nConnection: close\r\n\r\n")?;
    let copied = io::copy(&mut client.take(body_len), &mut upstream)?;
    if copied != body_len {
        return Err(invalid("Incomplete request body"));
    }
    io::copy(&mut upstream, client)?;
    Ok(())
}

fn tunnel(client: &mut TcpStream, mut upstream: TcpStream) -> io::Result<()> {
    let mut client_read = client.try_clone()?;
    let mut upstream_write = upstream.try_clone()?;
    let upload = thread::spawn(move || {
        let result = io::copy(&mut client_read, &mut upstream_write);
        let _ = upstream_write.shutdown(Shutdown::Write);
        result
    });
    let result = io::copy(&mut upstream, client);
    let _ = client.shutdown(Shutdown::Write);
    let _ = upload.join();
    result.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peers_must_match_local_subnet() {
        let subnets = [(
            u32::from(Ipv4Addr::new(192, 168, 1, 0)),
            u32::from(Ipv4Addr::new(255, 255, 255, 0)),
        )];
        assert!(allowed_peer("192.168.1.42:1234".parse().unwrap(), &subnets));
        assert!(!allowed_peer(
            "192.168.2.42:1234".parse().unwrap(),
            &subnets
        ));
        assert!(!allowed_peer("127.0.0.1:1234".parse().unwrap(), &subnets));
        assert!(!allowed_peer("192.168.1.42:1234".parse().unwrap(), &[]));
    }

    #[test]
    fn normal_subnet_mask_is_contiguous() {
        let mask = u32::from(Ipv4Addr::new(255, 255, 255, 0));
        assert_eq!((!mask) & ((!mask).wrapping_add(1)), 0);
        let invalid = u32::from(Ipv4Addr::new(255, 255, 0, 255));
        assert_ne!((!invalid) & ((!invalid).wrapping_add(1)), 0);
    }

    #[test]
    fn malformed_host_and_framing_are_rejected() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        for header in [
            "CONNECT example.org:443 HTTP/1.1\r\nHost: other.org:443\r\n\r\n",
            "GET http://example.org/ HTTP/1.1\r\nHost: other.org\r\n\r\n",
            "POST http://example.org/ HTTP/1.1\r\nHost: example.org\r\nTransfer-Encoding: chunked\r\n\r\n",
            "GET http://example.org/ HTTP/1.1\r\nHost: example.org\r\nContent-Length: 0\r\nContent-Length: 0\r\n\r\n",
        ] {
            let mut peer = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
            peer.write_all(header.as_bytes()).unwrap();
            let (mut accepted, _) = listener.accept().unwrap();
            let (events, _) = mpsc::sync_channel(1);
            let peer = accepted.peer_addr().unwrap();
            assert!(handle(
                &mut accepted,
                &Mutex::new(HashMap::new()),
                0,
                peer,
                &events,
                None,
                &Transfers::default(),
            )
            .is_err());
        }
    }

    #[test]
    fn connect_destination_is_logged() {
        let peer = "192.168.1.42:1234".parse().unwrap();
        let (events, received) = mpsc::sync_channel(2);
        log_request(&events, peer, "CONNECT", "example.org:443");
        assert_eq!(
            received.try_recv().unwrap(),
            format!("{peer} CONNECT example.org:443")
        );
        log_request(&events, peer, "GET", "http://example.org/game.pkg?id=1");
        assert_eq!(
            received.try_recv().unwrap(),
            format!("{peer} GET http://example.org/game.pkg")
        );
    }

    #[test]
    fn request_log_hides_url_credentials_and_fragments() {
        let peer = "192.168.1.42:1234".parse().unwrap();
        let (events, received) = mpsc::sync_channel(3);
        log_request(
            &events,
            peer,
            "GET",
            "http://user:secret@example.org/private",
        );
        log_request(&events, peer, "GET", "http://example.org/game.pkg#secret");
        log_request(&events, peer, "CONNECT", "user@host:443");
        assert_eq!(
            received.try_recv().unwrap(),
            format!("{peer} GET [URL with credentials]")
        );
        assert_eq!(
            received.try_recv().unwrap(),
            format!("{peer} GET http://example.org/game.pkg")
        );
        assert_eq!(
            received.try_recv().unwrap(),
            format!("{peer} CONNECT [invalid authority]")
        );
    }

    #[test]
    fn stop_closes_registered_connections() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut peer = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (accepted, _) = listener.accept().unwrap();
        let sockets = Arc::new(Mutex::new(HashMap::from([(0, vec![accepted])])));
        let mut proxy = Proxy {
            stop: Arc::new(AtomicBool::new(false)),
            thread: None,
            address: listener.local_addr().unwrap(),
            sockets,
            events: mpsc::sync_channel(1).1,
            transfers: Transfers::default(),
            package_dir: Arc::new(Mutex::new(None)),
        };
        proxy.stop();
        peer.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
        assert_eq!(peer.read(&mut [0u8; 1]).unwrap(), 0);
    }

    #[test]
    fn transfers_track_progress_without_inflating_retries() {
        let transfers = Transfers::default();
        let peer = "192.168.1.50:1234".parse().unwrap();
        let url = "http://gst.prod.dl.playstation.net/game_0.pkg";
        let id = transfers.begin(peer, url, "game_0.pkg", 100, 100);
        transfers.progress(id, 40, 40, "game_0.pkg");
        transfers.progress(id, 80, 80, "game_0.pkg");
        assert_eq!(transfers.snapshot().bytes, 80);
        transfers.end(id, "game_0.pkg", true);
        let snapshot = transfers.snapshot();
        assert!(snapshot.active.is_empty());
        assert!(snapshot.files["game_0.pkg"].interrupted);
        assert!(!snapshot.files["game_0.pkg"].complete);

        // A retry restarts byte accounting but keeps the highest offset reached.
        let id = transfers.begin(peer, url, "game_0.pkg", 100, 100);
        assert!(!transfers.snapshot().files["game_0.pkg"].interrupted);
        transfers.progress(id, 100, 100, "game_0.pkg");
        transfers.end(id, "game_0.pkg", false);
        let snapshot = transfers.snapshot();
        assert_eq!(snapshot.files["game_0.pkg"].served, 100);
        assert!(snapshot.files["game_0.pkg"].complete);
        assert!(!snapshot.files["game_0.pkg"].interrupted);
        assert_eq!(snapshot.bytes, 180);
    }

    #[test]
    fn destinations_exclude_local_and_reserved_addresses() {
        for address in [
            "127.0.0.1",
            "10.0.0.1",
            "172.16.0.1",
            "192.168.1.2",
            "169.254.1.1",
            "100.64.0.1",
            "0.0.0.0",
            "224.0.0.1",
        ] {
            assert!(!public_ip(address.parse().unwrap()), "{address}");
        }
        assert!(public_ip("8.8.8.8".parse().unwrap()));
    }
}
