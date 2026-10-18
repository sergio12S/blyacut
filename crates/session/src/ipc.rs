//! macOS/Linux local bridge. Private directory + socket + per-session bearer secret.
use crate::gateway::Gateway;
use blyacut_core::{id, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Read, Write},
    os::unix::{
        fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
pub const REQUEST_LIMIT: u64 = 256 * 1024;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    socket_path: PathBuf,
    token: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    token: String,
    name: String,
    arguments: Value,
}
#[derive(Clone, Serialize)]
pub struct ConnectionInfo {
    pub config_path: PathBuf,
    pub allowed_root: PathBuf,
}
pub struct Server {
    pub info: ConnectionInfo,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    directory: PathBuf,
}
impl Server {
    pub fn start(gateway: Gateway) -> Result<Self> {
        let directory = Path::new("/tmp").join(format!("blyacut-{}", id()));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .map_err(|e| e.to_string())?;
        let result = (|| {
            let socket_path = directory.join("editor.sock");
            let config_path = directory.join("session.json");
            let listener = UnixListener::bind(&socket_path).map_err(|e| e.to_string())?;
            std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o600))
                .map_err(|e| e.to_string())?;
            listener.set_nonblocking(true).map_err(|e| e.to_string())?;
            let token = id();
            let config = Config {
                socket_path,
                token: token.clone(),
            };
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&config_path)
                .map_err(|e| e.to_string())?;
            file.write_all(&serde_json::to_vec(&config).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            let info = ConnectionInfo {
                config_path,
                allowed_root: gateway.info().allowed_root,
            };
            let stop = Arc::new(AtomicBool::new(false));
            let flag = stop.clone();
            let thread = std::thread::spawn(move || {
                while !flag.load(Ordering::Acquire) {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            // The accept loop polls; each connected request uses bounded
                            // blocking I/O, independent of platform flag inheritance.
                            if stream.set_nonblocking(false).is_err() {
                                continue;
                            }
                            let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
                            let _ = stream.set_write_timeout(Some(Duration::from_secs(3)));
                            let result = (|| -> Result<Value> {
                                let value = read_json(&mut BufReader::new(&stream), REQUEST_LIMIT)?
                                    .ok_or("Empty request")?;
                                let request: Request =
                                    serde_json::from_value(value).map_err(|e| e.to_string())?;
                                if request.token != token || flag.load(Ordering::Acquire) {
                                    return Err("Unauthorized or closed session".into());
                                }
                                gateway.call(&request.name, request.arguments)
                            })();
                            let response = match result {
                                Ok(result) => json!({"result":result}),
                                Err(error) => json!({"error":error}),
                            };
                            if let Ok(bytes) = serde_json::to_vec(&response) {
                                let _ = stream.write_all(&bytes);
                                let _ = stream.write_all(b"\n");
                            }
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(20))
                        }
                        Err(_) => break,
                    }
                }
            });
            Ok((info, stop, thread))
        })();
        match result {
            Ok((info, stop, thread)) => Ok(Self {
                info,
                stop,
                thread: Some(thread),
                directory,
            }),
            Err(e) => {
                let _ = std::fs::remove_dir_all(directory);
                Err(e)
            }
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
/// Bounded newline-delimited JSON, shared with the stdio adapter.
pub fn read_json(reader: &mut impl BufRead, limit: u64) -> Result<Option<Value>> {
    let mut line = Vec::new();
    let count = reader
        .take(limit + 1)
        .read_until(b'\n', &mut line)
        .map_err(|e| e.to_string())?;
    if count == 0 {
        return Ok(None);
    }
    if count as u64 > limit || line.last() != Some(&b'\n') {
        return Err("Message too large or missing newline".into());
    }
    serde_json::from_slice(&line)
        .map(Some)
        .map_err(|e| e.to_string())
}
pub fn call(config_path: &Path, name: &str, arguments: Value) -> Result<Value> {
    let meta = std::fs::symlink_metadata(config_path)
        .map_err(|e| format!("Agent session unavailable: {e}"))?;
    if !meta.is_file() || meta.permissions().mode() & 0o077 != 0 || meta.len() > 4096 {
        return Err("Session config must be a private regular file (0600)".into());
    }
    let config: Config =
        serde_json::from_reader(std::fs::File::open(config_path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let mut stream = UnixStream::connect(&config.socket_path)
        .map_err(|e| format!("Editor session unavailable: {e}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    let request = Request {
        token: config.token,
        name: name.into(),
        arguments,
    };
    let bytes = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
    if bytes.len() as u64 + 1 > REQUEST_LIMIT {
        return Err("Request too large".into());
    }
    stream
        .write_all(&bytes)
        .and_then(|_| stream.write_all(b"\n"))
        .map_err(|e| e.to_string())?;
    let response =
        read_json(&mut BufReader::new(stream), 16 * 1024 * 1024)?.ok_or("Empty editor response")?;
    if let Some(e) = response.get("error") {
        Err(e.as_str().unwrap_or("Unknown editor error").into())
    } else {
        response
            .get("result")
            .cloned()
            .ok_or("Invalid editor response".into())
    }
}
