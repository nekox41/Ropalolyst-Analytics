use std::{
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

/// Resolve Warframe's per-user log path without assuming a Windows username.
pub fn default_log_path() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|base| base.join("Warframe").join("EE.log"))
}

/// Incremental reader for EE.log. Starts at byte zero so a running mission can
/// be reconstructed when the analyzer starts after the game.
#[derive(Debug)]
pub struct LogTailer {
    path: PathBuf,
    offset: u64,
    pending: Vec<u8>,
}

impl LogTailer {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into(), offset: 0, pending: Vec::new() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }

    /// Read complete new lines. Partial trailing lines are retained until the
    /// next poll, and a truncated/recreated log is automatically rewound.
    pub fn read_new_lines(&mut self) -> io::Result<Vec<String>> {
        let metadata = fs::metadata(&self.path)?;
        if metadata.len() < self.offset {
            self.offset = 0;
            self.pending.clear();
        }

        let mut file = File::open(&self.path)?;
        file.seek(SeekFrom::Start(self.offset))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        self.offset += bytes.len() as u64;
        self.pending.extend_from_slice(&bytes);

        let mut lines = Vec::new();
        let mut consumed = 0;
        for (index, byte) in self.pending.iter().enumerate() {
            if *byte == b'\n' {
                let line = String::from_utf8_lossy(&self.pending[consumed..index]);
                lines.push(line.trim_end_matches('\r').to_owned());
                consumed = index + 1;
            }
        }
        if consumed > 0 {
            self.pending.drain(..consumed);
        }
        Ok(lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write, thread, time::{SystemTime, UNIX_EPOCH}};

    fn temp_log() -> PathBuf {
        let id = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("ropalolyst-{id}.log"))
    }

    #[test]
    fn reads_incrementally_and_buffers_partial_lines() {
        let path = temp_log();
        fs::write(&path, b"1.0 first\n2.0 par").unwrap();
        let mut tailer = LogTailer::new(&path);
        assert_eq!(tailer.read_new_lines().unwrap(), vec!["1.0 first"]);
        fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(b"tial\n").unwrap();
        assert_eq!(tailer.read_new_lines().unwrap(), vec!["2.0 partial"]);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn restarts_from_beginning_when_log_is_truncated() {
        let path = temp_log();
        fs::write(&path, b"1.0 old line\n").unwrap();
        let mut tailer = LogTailer::new(&path);
        assert_eq!(tailer.read_new_lines().unwrap().len(), 1);
        thread::sleep(std::time::Duration::from_millis(2));
        fs::write(&path, b"2.0 new\n").unwrap();
        assert_eq!(tailer.read_new_lines().unwrap(), vec!["2.0 new"]);
        let _ = fs::remove_file(path);
    }
}
