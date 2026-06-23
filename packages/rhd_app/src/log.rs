use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use chrono::Local;

#[derive(Debug, Clone)]
pub enum OutputLine {
    Stdout(String),
    Stderr(String),
}

impl OutputLine {
    pub fn content(&self) -> &str {
        match self {
            OutputLine::Stdout(s) => s,
            OutputLine::Stderr(s) => s,
        }
    }
}

pub struct LogSink {
    file: Option<BufWriter<File>>,
}

impl LogSink {
    pub fn new(file: Option<File>) -> Self {
        Self {
            file: file.map(BufWriter::new),
        }
    }

    pub fn log(&mut self, prefix: &str, header: &str, body: &str) {
        let block = if body.is_empty() {
            format!("===== {prefix}: {header} =====\n")
        } else {
            format!("===== {prefix}: {header} =====\n{body}\n")
        };
        print!("{block}");
        let _ = io::stdout().flush();
        if let Some(f) = &mut self.file {
            let _ = f.write_all(block.as_bytes());
            let _ = f.flush();
        }
    }

    pub fn log_step(&mut self, step: &str, header: &str, body: &str) {
        let block = format!("===== {step}: {header} =====\n{body}\n");
        print!("{block}");
        let _ = io::stdout().flush();
        if let Some(f) = &mut self.file {
            let _ = f.write_all(block.as_bytes());
            let _ = f.flush();
        }
    }

    pub fn log_command_output(&mut self, step: &str, lines: &[OutputLine]) {
        let mut body = String::new();
        for line in lines {
            match line {
                OutputLine::Stdout(s) => body.push_str(&format!("[STDOUT] {s}")),
                OutputLine::Stderr(s) => body.push_str(&format!("[STDERR] {s}")),
            }
        }
        self.log_step(step, "command output", &body);
    }

    pub fn log_ai_request(
        &mut self,
        step: &str,
        model: &str,
        system_prompt: &str,
        message: &str,
    ) {
        let mut body = format!("model: {model}\n");
        body.push_str("----- system prompt -----\n");
        body.push_str(system_prompt);
        if !system_prompt.ends_with('\n') {
            body.push('\n');
        }
        body.push_str("----- message -----\n");
        body.push_str(message);
        if !message.ends_with('\n') {
            body.push('\n');
        }
        self.log_step(step, "AI request", &body);
    }
}

pub fn create_log_dir(logs_root: &Path, scenario_name: &str) -> io::Result<PathBuf> {
    fs::create_dir_all(logs_root)?;

    let timestamp = Local::now().format("%Y-%m-%d-%H-%M-%S").to_string();
    let base_name = format!("{scenario_name}-{timestamp}");

    let mut candidate = logs_root.join(&base_name);
    let mut counter = 2u64;
    while candidate.exists() {
        candidate = logs_root.join(format!("{base_name}-{counter}"));
        counter += 1;
    }

    fs::create_dir(&candidate)?;
    Ok(candidate)
}

pub fn open_log_file(dir: &Path) -> io::Result<File> {
    OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(dir.join("log.txt"))
}
