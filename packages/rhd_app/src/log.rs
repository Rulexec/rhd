use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use chrono::Local;
use rhd_api::{LogSection, LogSectionKind, ScenarioMeta};

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
    line_count: u64,
}

impl LogSink {
    pub fn new(file: Option<File>) -> Self {
        Self {
            file: file.map(BufWriter::new),
            line_count: 0,
        }
    }

    pub fn current_line(&self) -> u64 {
        self.line_count + 1
    }

    fn count_lines(s: &str) -> u64 {
        s.chars().filter(|&c| c == '\n').count() as u64
    }

    fn write_block(&mut self, block: &str) {
        self.line_count += Self::count_lines(block);
        print!("{block}");
        let _ = io::stdout().flush();
        if let Some(f) = &mut self.file {
            let _ = f.write_all(block.as_bytes());
            let _ = f.flush();
        }
    }

    pub fn log(&mut self, prefix: &str, header: &str, body: &str) {
        let block = if body.is_empty() {
            format!("===== {prefix}: {header} =====\n")
        } else {
            format!("===== {prefix}: {header} =====\n{body}\n")
        };
        self.write_block(&block);
    }

    pub fn log_aborted(&mut self) {
        let block = "===== ABORTED =====\n".to_string();
        self.write_block(&block);
    }

    pub fn log_step(&mut self, step: &str, header: &str, body: &str) {
        let block = format!("===== {step}: {header} =====\n{body}\n");
        self.write_block(&block);
    }

    pub fn log_step_dashed(&mut self, step: &str, header: &str, body: &str) {
        let block = format!("----- {step}: {header} -----\n{body}\n");
        self.write_block(&block);
    }

    pub fn log_command_output(&mut self, step: &str, lines: &[OutputLine]) {
        let mut body = String::new();
        for line in lines {
            match line {
                OutputLine::Stdout(s) => body.push_str(&format!("[STDOUT] {s}")),
                OutputLine::Stderr(s) => body.push_str(&format!("[STDERR] {s}")),
            }
        }
        self.log_step_dashed(step, "command output", &body);
    }

    pub fn log_ai_request(
        &mut self,
        step: &str,
        model: &str,
        tools: &[String],
        system_prompt: &str,
        message: &str,
    ) {
        let mut body = format!("model: {model}\n");
        if !tools.is_empty() {
            body.push_str(&format!("available tools: {}\n", tools.join(", ")));
        }
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

pub fn write_meta_json(dir: &Path, meta: &ScenarioMeta) -> io::Result<()> {
    let json = serde_json::to_string_pretty(meta)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    fs::write(dir.join("meta.json"), json)
}

pub fn read_finished_scenarios(logs_dir: &Path) -> io::Result<Vec<ScenarioMeta>> {
    let mut scenarios = Vec::new();

    if !logs_dir.exists() {
        return Ok(scenarios);
    }

    for entry in fs::read_dir(logs_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let meta_path = path.join("meta.json");
        if !meta_path.exists() {
            continue;
        }

        match fs::read_to_string(&meta_path) {
            Ok(content) => match serde_json::from_str::<ScenarioMeta>(&content) {
                Ok(meta) => scenarios.push(meta),
                Err(_) => continue,
            },
            Err(_) => continue,
        }
    }

    scenarios.sort_by(|a, b| b.started.cmp(&a.started));
    Ok(scenarios)
}

pub struct SectionTracker {
    start_line: Option<u64>,
    kind: LogSectionKind,
}

impl SectionTracker {
    pub fn start(sink: &LogSink, kind: LogSectionKind) -> Self {
        Self {
            start_line: Some(sink.current_line()),
            kind,
        }
    }

    pub fn end(self, sink: &LogSink) -> LogSection {
        let end_line = sink.current_line().saturating_sub(1);
        LogSection {
            kind: self.kind,
            start_line: self.start_line.unwrap_or(1),
            end_line,
        }
    }
}
