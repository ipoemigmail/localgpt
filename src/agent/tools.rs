use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tracing::debug;

use super::providers::ToolSchema;
use crate::config::Config;

#[derive(Debug, Clone)]
pub struct ToolResult {
    pub call_id: String,
    pub output: String,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn schema(&self) -> ToolSchema;
    async fn execute(&self, arguments: &str) -> Result<String>;
}

pub fn create_default_tools(config: &Config) -> Result<Vec<Box<dyn Tool>>> {
    let workspace = config.workspace_path();

    Ok(vec![
        Box::new(BashTool::new()),
        Box::new(ReadFileTool::new()),
        Box::new(WriteFileTool::new()),
        Box::new(EditFileTool::new()),
        Box::new(MemorySearchTool::new(workspace.clone())),
        Box::new(MemoryAppendTool::new(workspace)),
        Box::new(WebFetchTool::new()),
    ])
}

// Bash Tool
pub struct BashTool;

impl BashTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "bash".to_string(),
            description: "Execute a bash command and return the output".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The bash command to execute"
                    },
                    "timeout_ms": {
                        "type": "integer",
                        "description": "Optional timeout in milliseconds (default: 30000)"
                    }
                },
                "required": ["command"]
            }),
        }
    }

    async fn execute(&self, arguments: &str) -> Result<String> {
        let args: Value = serde_json::from_str(arguments)?;
        let command = args["command"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing command"))?;

        debug!("Executing bash command: {}", command);

        let output = Command::new("bash").arg("-c").arg(command).output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let mut result = String::new();

        if !stdout.is_empty() {
            result.push_str(&stdout);
        }

        if !stderr.is_empty() {
            if !result.is_empty() {
                result.push_str("\n\nSTDERR:\n");
            }
            result.push_str(&stderr);
        }

        if result.is_empty() {
            result = format!(
                "Command completed with exit code: {}",
                output.status.code().unwrap_or(-1)
            );
        }

        Ok(result)
    }
}

// Read File Tool
pub struct ReadFileTool;

impl ReadFileTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "read_file".to_string(),
            description: "Read the contents of a file".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The path to the file to read"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Line number to start reading from (0-indexed)"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of lines to read"
                    }
                },
                "required": ["path"]
            }),
        }
    }

    async fn execute(&self, arguments: &str) -> Result<String> {
        let args: Value = serde_json::from_str(arguments)?;
        let path = args["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing path"))?;

        let path = shellexpand::tilde(path).to_string();

        debug!("Reading file: {}", path);

        let content = fs::read_to_string(&path)?;

        // Handle offset and limit
        let offset = args["offset"].as_u64().unwrap_or(0) as usize;
        let limit = args["limit"].as_u64().map(|l| l as usize);

        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        let start = offset.min(total_lines);
        let end = limit
            .map(|l| (start + l).min(total_lines))
            .unwrap_or(total_lines);

        let selected: Vec<String> = lines[start..end]
            .iter()
            .enumerate()
            .map(|(i, line)| format!("{:4}\t{}", start + i + 1, line))
            .collect();

        Ok(selected.join("\n"))
    }
}

// Write File Tool
pub struct WriteFileTool;

impl WriteFileTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "write_file".to_string(),
            description: "Write content to a file (creates or overwrites)".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The path to the file to write"
                    },
                    "content": {
                        "type": "string",
                        "description": "The content to write to the file"
                    }
                },
                "required": ["path", "content"]
            }),
        }
    }

    async fn execute(&self, arguments: &str) -> Result<String> {
        let args: Value = serde_json::from_str(arguments)?;
        let path = args["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing path"))?;
        let content = args["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing content"))?;

        let path = shellexpand::tilde(path).to_string();
        let path = PathBuf::from(&path);

        debug!("Writing file: {}", path.display());

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&path, content)?;

        Ok(format!(
            "Successfully wrote {} bytes to {}",
            content.len(),
            path.display()
        ))
    }
}

// Edit File Tool
pub struct EditFileTool;

impl EditFileTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for EditFileTool {
    fn name(&self) -> &str {
        "edit_file"
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "edit_file".to_string(),
            description: "Edit a file by replacing old_string with new_string".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The path to the file to edit"
                    },
                    "old_string": {
                        "type": "string",
                        "description": "The text to replace"
                    },
                    "new_string": {
                        "type": "string",
                        "description": "The replacement text"
                    },
                    "replace_all": {
                        "type": "boolean",
                        "description": "Replace all occurrences (default: false)"
                    }
                },
                "required": ["path", "old_string", "new_string"]
            }),
        }
    }

    async fn execute(&self, arguments: &str) -> Result<String> {
        let args: Value = serde_json::from_str(arguments)?;
        let path = args["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing path"))?;
        let old_string = args["old_string"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing old_string"))?;
        let new_string = args["new_string"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing new_string"))?;
        let replace_all = args["replace_all"].as_bool().unwrap_or(false);

        let path = shellexpand::tilde(path).to_string();

        debug!("Editing file: {}", path);

        let content = fs::read_to_string(&path)?;

        let (new_content, count) = if replace_all {
            let count = content.matches(old_string).count();
            (content.replace(old_string, new_string), count)
        } else {
            if content.contains(old_string) {
                (content.replacen(old_string, new_string, 1), 1)
            } else {
                return Err(anyhow::anyhow!("old_string not found in file"));
            }
        };

        fs::write(&path, &new_content)?;

        Ok(format!("Replaced {} occurrence(s) in {}", count, path))
    }
}

// Memory Search Tool
pub struct MemorySearchTool {
    workspace: PathBuf,
}

impl MemorySearchTool {
    pub fn new(workspace: PathBuf) -> Self {
        Self { workspace }
    }
}

#[async_trait]
impl Tool for MemorySearchTool {
    fn name(&self) -> &str {
        "memory_search"
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "memory_search".to_string(),
            description: "Search the memory index for relevant information".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of results (default: 5)"
                    }
                },
                "required": ["query"]
            }),
        }
    }

    async fn execute(&self, arguments: &str) -> Result<String> {
        let args: Value = serde_json::from_str(arguments)?;
        let query = args["query"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing query"))?;
        let limit = args["limit"].as_u64().unwrap_or(5) as usize;

        debug!("Memory search: {} (limit: {})", query, limit);

        // Simple grep-based search for now
        // TODO: Use proper memory index
        let mut results = Vec::new();

        let memory_file = self.workspace.join("MEMORY.md");
        if memory_file.exists() {
            if let Ok(content) = fs::read_to_string(&memory_file) {
                for (i, line) in content.lines().enumerate() {
                    if line.to_lowercase().contains(&query.to_lowercase()) {
                        results.push(format!("MEMORY.md:{}: {}", i + 1, line));
                        if results.len() >= limit {
                            break;
                        }
                    }
                }
            }
        }

        // Search daily logs
        let memory_dir = self.workspace.join("memory");
        if memory_dir.exists() {
            if let Ok(entries) = fs::read_dir(&memory_dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    if results.len() >= limit {
                        break;
                    }

                    let path = entry.path();
                    if path.extension().map(|e| e == "md").unwrap_or(false) {
                        if let Ok(content) = fs::read_to_string(&path) {
                            let filename = path.file_name().unwrap().to_string_lossy();
                            for (i, line) in content.lines().enumerate() {
                                if line.to_lowercase().contains(&query.to_lowercase()) {
                                    results.push(format!(
                                        "memory/{}:{}: {}",
                                        filename,
                                        i + 1,
                                        line
                                    ));
                                    if results.len() >= limit {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if results.is_empty() {
            Ok("No results found".to_string())
        } else {
            Ok(results.join("\n"))
        }
    }
}

// Memory Append Tool
pub struct MemoryAppendTool {
    workspace: PathBuf,
}

impl MemoryAppendTool {
    pub fn new(workspace: PathBuf) -> Self {
        Self { workspace }
    }
}

#[async_trait]
impl Tool for MemoryAppendTool {
    fn name(&self) -> &str {
        "memory_append"
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "memory_append".to_string(),
            description: "Append an entry to today's memory log (memory/YYYY-MM-DD.md)".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "The content to append"
                    }
                },
                "required": ["content"]
            }),
        }
    }

    async fn execute(&self, arguments: &str) -> Result<String> {
        let args: Value = serde_json::from_str(arguments)?;
        let content = args["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing content"))?;

        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let memory_dir = self.workspace.join("memory");
        fs::create_dir_all(&memory_dir)?;

        let path = memory_dir.join(format!("{}.md", today));

        debug!("Appending to memory: {}", path.display());

        let timestamp = chrono::Local::now().format("%H:%M").to_string();
        let entry = format!("\n## {}\n\n{}\n", timestamp, content);

        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;

        use std::io::Write;
        file.write_all(entry.as_bytes())?;

        Ok(format!("Appended to {}", path.display()))
    }
}

// Web Fetch Tool
pub struct WebFetchTool {
    client: reqwest::Client,
}

impl WebFetchTool {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "web_fetch"
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "web_fetch".to_string(),
            description: "Fetch content from a URL".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "The URL to fetch"
                    }
                },
                "required": ["url"]
            }),
        }
    }

    async fn execute(&self, arguments: &str) -> Result<String> {
        let args: Value = serde_json::from_str(arguments)?;
        let url = args["url"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing url"))?;

        debug!("Fetching URL: {}", url);

        let response = self
            .client
            .get(url)
            .header("User-Agent", "LocalGPT/0.1")
            .send()
            .await?;

        let status = response.status();
        let body = response.text().await?;

        // Truncate if too long
        let max_len = 10000;
        let truncated = if body.len() > max_len {
            format!(
                "{}...\n\n[Truncated, {} bytes total]",
                &body[..max_len],
                body.len()
            )
        } else {
            body
        };

        Ok(format!("Status: {}\n\n{}", status, truncated))
    }
}
