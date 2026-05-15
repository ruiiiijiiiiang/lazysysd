use std::{
    io::{Error, Result},
    process::Command,
};

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

    pub async fn fetch_logs(&self, unit_name: &str, scope: &str, limit: usize) -> Result<Vec<String>> {
        let mut command = Command::new("journalctl");
        if scope == "session" {
            command.arg("--user");
        }
        command
            .arg("-u")
            .arg(unit_name)
            .arg("-n")
            .arg(limit.to_string())
            .arg("--no-pager");

        let output = command.output()?;

        if !output.status.success() {
            return Err(Error::other(format!(
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
