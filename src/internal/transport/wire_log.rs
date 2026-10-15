//! Optional wire log of every line exchanged with the Claude CLI process.
//!
//! Enabled by setting `CLAUDE_SDK_WIRE_LOG=/path/to/file.jsonl`. Each line
//! written is a JSON object: `{"ts_ms":…,"dir":"send"|"recv"|"recv_invalid","data":…}`
//! where `data` is the parsed JSON message (or the raw string if it was not
//! valid JSON). The file is opened in append mode.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Environment variable naming the wire log file.
pub const WIRE_LOG_ENV: &str = "CLAUDE_SDK_WIRE_LOG";

static SINK: OnceLock<Option<Mutex<File>>> = OnceLock::new();

fn sink() -> Option<&'static Mutex<File>> {
    SINK.get_or_init(|| {
        let path = std::env::var_os(WIRE_LOG_ENV).filter(|p| !p.is_empty())?;
        match OpenOptions::new().create(true).append(true).open(&path) {
            Ok(f) => Some(Mutex::new(f)),
            Err(e) => {
                tracing::warn!("cannot open {WIRE_LOG_ENV} {path:?}: {e}");
                None
            }
        }
    })
    .as_ref()
}

/// Record one line travelling in direction `dir` ("send" or "recv").
pub(crate) fn log(dir: &str, line: &str) {
    let Some(sink) = sink() else { return };
    let line = line.trim_end();
    let data = serde_json::from_str::<serde_json::Value>(line)
        .unwrap_or_else(|_| serde_json::Value::String(line.to_string()));
    let ts_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let entry = serde_json::json!({ "ts_ms": ts_ms, "dir": dir, "data": data });
    if let Ok(mut f) = sink.lock() {
        let _ = writeln!(f, "{entry}");
    }
}
