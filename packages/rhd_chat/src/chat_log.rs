use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use chrono::Local;

pub struct ChatLoggers {
    pub chat_log: ChatLogSink,
    pub raw_log: Option<RawChatLogSink>,
}

pub fn create_chat_loggers(
    logs_root: &Path,
    chat_title: &str,
    raw_enabled: bool,
) -> std::io::Result<ChatLoggers> {
    let dir = create_chat_log_dir(logs_root, chat_title)?;
    let chat_file = open_chat_log_file(&dir)?;
    let chat_log = ChatLogSink::new(chat_file);
    let raw_log = if raw_enabled {
        match open_raw_log_file(&dir) {
            Ok(f) => Some(RawChatLogSink::new(f)),
            Err(_) => None,
        }
    } else {
        None
    };
    Ok(ChatLoggers { chat_log, raw_log })
}

pub struct ChatLogSink {
    file: BufWriter<File>,
}

impl ChatLogSink {
    pub fn new(file: File) -> Self {
        Self {
            file: BufWriter::new(file),
        }
    }

    fn write_block(&mut self, block: &str) {
        let _ = self.file.write_all(block.as_bytes());
        let _ = self.file.flush();
    }

    pub fn log_stream_start(
        &mut self,
        chat_id: i64,
        title: &str,
        model: &str,
        tools: &[String],
        messages: &str,
    ) {
        let mut body = format!("model: {model}\n");
        if !tools.is_empty() {
            body.push_str(&format!("available tools: {}\n", tools.join(", ")));
        }
        body.push_str("\n----- messages sent to API -----\n");
        body.push_str(messages);
        if !messages.ends_with('\n') {
            body.push('\n');
        }
        let block = format!("===== Chat \"{}\" (id={}): stream started =====\n{}\n", title, chat_id, body);
        self.write_block(&block);
    }

    pub fn log_assistant_response(
        &mut self,
        reasoning: Option<&str>,
        content: &str,
        finish_reason: &str,
        tokens: Option<&rhd_api::TokenUsage>,
    ) {
        let mut body = String::new();
        
        if let Some(reasoning_text) = reasoning {
            if !reasoning_text.is_empty() {
                body.push_str("----- reasoning -----\n");
                body.push_str(reasoning_text);
                if !reasoning_text.ends_with('\n') {
                    body.push('\n');
                }
                body.push('\n');
            }
        }
        
        body.push_str("----- message -----\n");
        body.push_str(content);
        if !content.ends_with('\n') {
            body.push('\n');
        }
        
        body.push_str(&format!("\nfinish_reason: {}\n", finish_reason));
        
        if let Some(usage) = tokens {
            body.push_str(&format!(
                "tokens: prompt={}, completion={}, total={}\n",
                usage.prompt_tokens, usage.completion_tokens, usage.total_tokens
            ));
        }
        
        let block = format!("===== Assistant response =====\n{}\n", body);
        self.write_block(&block);
    }

    pub fn log_tool_call(&mut self, name: &str, call_id: &str, arguments: &str) {
        let block = format!(
            "===== Tool call: {} (id={}) =====\n{}\n",
            name, call_id, arguments
        );
        self.write_block(&block);
    }

    pub fn log_tool_result(&mut self, name: &str, call_id: &str, result: &str) {
        let block = format!(
            "===== Tool result: {} (id={}) =====\n{}\n",
            name, call_id, result
        );
        self.write_block(&block);
    }

    pub fn log_stream_finished(&mut self, finish_reason: &str, duration_ms: u64) {
        let block = format!(
            "===== Stream finished =====\nfinish_reason: {}\ntotal duration: {}ms\n",
            finish_reason, duration_ms
        );
        self.write_block(&block);
    }

    pub fn log_stream_error(&mut self, error: &str) {
        let block = format!("===== Stream error =====\nerror: {}\n", error);
        self.write_block(&block);
    }
}

pub fn create_chat_log_dir(logs_root: &Path, chat_title: &str) -> std::io::Result<PathBuf> {
    fs::create_dir_all(logs_root)?;

    let sanitized_title = chat_title
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect::<String>();

    let timestamp = Local::now().format("%Y-%m-%d-%H-%M-%S").to_string();
    let base_name = format!("{}-{}", sanitized_title, timestamp);

    let mut candidate = logs_root.join(&base_name);
    let mut counter = 2u64;
    while candidate.exists() {
        candidate = logs_root.join(format!("{}-{}", base_name, counter));
        counter += 1;
    }

    fs::create_dir(&candidate)?;
    Ok(candidate)
}

pub fn open_chat_log_file(dir: &Path) -> std::io::Result<File> {
    OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(dir.join("log.txt"))
}

pub fn open_raw_log_file(dir: &Path) -> std::io::Result<File> {
    OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(dir.join("raw.txt"))
}

pub struct RawChatLogSink {
    file: BufWriter<File>,
}

impl RawChatLogSink {
    pub fn new(file: File) -> Self {
        Self {
            file: BufWriter::new(file),
        }
    }

    fn write_block(&mut self, block: &str) {
        let _ = self.file.write_all(block.as_bytes());
        let _ = self.file.flush();
    }
}

impl rhd_ai::client::RawLogger for RawChatLogSink {
    fn log_request(&mut self, request_json: &str) {
        let block = format!("===== REQUEST =====\n{}\n", request_json);
        self.write_block(&block);
    }

    fn log_stream_chunk(&mut self, index: usize, chunk_json: &str) {
        let block = format!("===== STREAM CHUNK {} =====\n{}\n", index, chunk_json);
        self.write_block(&block);
    }

    fn log_response(&mut self, response_json: &str) {
        let block = format!("===== RESPONSE =====\n{}\n", response_json);
        self.write_block(&block);
    }

    fn log_error(&mut self, status: Option<u16>, body: &str) {
        let status_str = status.map(|s| s.to_string()).unwrap_or_else(|| "N/A".to_string());
        let block = format!("===== ERROR =====\nstatus: {}\n{}\n", status_str, body);
        self.write_block(&block);
    }

    fn log_tool_result_raw(&mut self, tool_name: &str, call_id: &str, raw_json: &str) {
        let block = format!(
            "===== TOOL RESULT RAW: {} (id={}) =====\n{}\n",
            tool_name, call_id, raw_json
        );
        self.write_block(&block);
    }
}
