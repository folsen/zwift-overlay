use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

/// Events from the Zwift log file watcher.
#[derive(Debug, Clone)]
pub enum LogEvent {
    /// A new interval/split started. The number is the split index.
    IntervalStarted(u32),
}

/// Default Zwift log path (macOS and Windows).
pub fn default_log_path() -> PathBuf {
    if cfg!(target_os = "windows") {
        let profile = std::env::var("USERPROFILE").unwrap_or_else(|_| ".".into());
        PathBuf::from(profile).join("Documents\\Zwift\\Logs\\Log.txt")
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        PathBuf::from(home).join("Documents/Zwift/Logs/Log.txt")
    }
}

/// Poll the Zwift log file for interval (split) events.
/// Runs on a blocking thread.
pub fn watch_zwift_log(tx: mpsc::Sender<LogEvent>, log_path: PathBuf) {
    // Wait for the file to exist
    while !log_path.exists() {
        std::thread::sleep(Duration::from_secs(2));
    }

    // Start at end of file so we only see new lines
    let mut pos = std::fs::metadata(&log_path).map(|m| m.len()).unwrap_or(0);

    loop {
        std::thread::sleep(Duration::from_secs(1));

        // Check if file was truncated/rotated (new session)
        let current_len = std::fs::metadata(&log_path).map(|m| m.len()).unwrap_or(0);
        if current_len < pos {
            pos = 0; // File was truncated, read from start
        }
        if current_len == pos {
            continue; // No new data
        }

        if let Ok(mut file) = File::open(&log_path) {
            if file.seek(SeekFrom::Start(pos)).is_ok() {
                let reader = BufReader::new(&mut file);
                for line in reader.lines() {
                    if let Ok(line) = line {
                        if let Some(split_num) = parse_split_line(&line) {
                            let _ = tx.send(LogEvent::IntervalStarted(split_num));
                        }
                    }
                }
                pos = current_len;
            }
        }
    }
}

/// Parse a line like: `[9:05:05] INFO LEVEL: [Splits] Cycling Split 1 started`
fn parse_split_line(line: &str) -> Option<u32> {
    let marker = "[Splits] Cycling Split ";
    let idx = line.find(marker)?;
    let after = &line[idx + marker.len()..];
    let num_str: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    num_str.parse().ok()
}
