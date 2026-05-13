use std::{io, process::Command};

use tailspin::Highlighter;

pub struct JournalManager {
    highlighter: Highlighter,
}

impl JournalManager {
    pub fn new() -> Self {
        Self {
            highlighter: Highlighter::default(),
        }
    }

    pub async fn fetch_logs(&self, unit_name: &str, limit: usize) -> io::Result<Vec<String>> {
        let output = Command::new("journalctl")
            .arg("-u")
            .arg(unit_name)
            .arg("-n")
            .arg(limit.to_string())
            .arg("--no-pager")
            .output()?;

        if !output.status.success() {
            return Err(io::Error::other(format!(
                "journalctl failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let content = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<String> = content
            .lines()
            .map(|line| self.highlighter.apply(line).into_owned())
            .collect();

        Ok(lines)
    }
}
