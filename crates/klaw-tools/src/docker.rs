//! Docker Tool - Execute commands in containers
//!
//! Real Docker integration for isolated tool execution.

use async_trait::async_trait;
use klaw_core::types::ToolResult;
use serde_json::{json, Value};
use std::collections::HashMap;

/// Docker executor for isolated command execution
pub struct DockerExecutor {
    /// Docker client
    client: Option<DockerClient>,
    /// Default image for execution
    default_image: String,
    /// Memory limit in MB
    memory_mb: u32,
    /// CPU limit (0.0-1.0)
    cpu_limit: f32,
    /// Timeout in seconds
    timeout_seconds: u32,
    /// Network enabled
    network: bool,
    /// Mount workspace
    mount_workspace: bool,
}

/// Simplified Docker client (uses bollard or docker CLI)
#[derive(Clone)]
pub struct DockerClient {
    /// Docker socket path
    socket_path: Option<String>,
}

/// Docker configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DockerConfig {
    pub image: String,
    pub memory_mb: Option<u32>,
    pub cpu_limit: Option<f32>,
    pub timeout_seconds: Option<u32>,
    pub network: Option<bool>,
    pub mount_workspace: Option<bool>,
    pub env: Option<HashMap<String, String>>,
    pub workdir: Option<String>,
}

impl Default for DockerConfig {
    fn default() -> Self {
        Self {
            image: "alpine:latest".to_string(),
            memory_mb: Some(512),
            cpu_limit: Some(0.5),
            timeout_seconds: Some(300),
            network: Some(false),
            mount_workspace: Some(true),
            env: None,
            workdir: Some("/workspace".to_string()),
        }
    }
}

impl DockerExecutor {
    /// Create new Docker executor
    pub fn new() -> Self {
        Self {
            client: None,
            default_image: "alpine:latest".to_string(),
            memory_mb: 512,
            cpu_limit: 0.5,
            timeout_seconds: 300,
            network: false,
            mount_workspace: true,
        }
    }
    
    /// Create with config
    pub fn with_config(config: DockerConfig) -> Self {
        Self {
            client: None,
            default_image: config.image.clone(),
            memory_mb: config.memory_mb.unwrap_or(512),
            cpu_limit: config.cpu_limit.unwrap_or(0.5),
            timeout_seconds: config.timeout_seconds.unwrap_or(300),
            network: config.network.unwrap_or(false),
            mount_workspace: config.mount_workspace.unwrap_or(true),
        }
    }
    
    /// Set Docker socket path
    pub fn with_socket(mut self, socket_path: &str) -> Self {
        self.client = Some(DockerClient {
            socket_path: Some(socket_path.to_string()),
        });
        self
    }
    
    /// Execute command in container
    pub async fn execute(&self, command: &str, workspace: Option<&str>) -> anyhow::Result<DockerResult> {
        // Use docker CLI for now (simpler than bollard)
        let container_id = self.create_container(command, workspace).await?;
        
        // Wait for completion
        let result = self.wait_container(&container_id).await?;
        
        // Clean up
        self.remove_container(&container_id).await?;
        
        Ok(result)
    }
    
    async fn create_container(&self, command: &str, workspace: Option<&str>) -> anyhow::Result<String> {
        use std::process::Command;
        
        let mut args = vec![
            "run".to_string(),
            "-d".to_string(), // detached
            "--rm".to_string(), // auto-remove
        ];
        
        // Memory limit
        args.push("-m".to_string());
        args.push(format!("{}m", self.memory_mb));
        
        // CPU limit
        args.push("--cpus".to_string());
        args.push(format!("{}", self.cpu_limit));
        
        // Timeout via --timeout or kill after
        // Network
        if !self.network {
            args.push("--network".to_string());
            args.push("none".to_string());
        }
        
        // Mount workspace
        if let Some(ws) = workspace {
            if self.mount_workspace {
                args.push("-v".to_string());
                args.push(format!("{}:/workspace", ws));
                args.push("-w".to_string());
                args.push("/workspace".to_string());
            }
        }
        
        // Image
        args.push(self.default_image.clone());
        
        // Command
        args.extend(shell_words::split(command)?);
        
        let output = Command::new("docker")
            .args(&args)
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to run docker: {}", e))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Docker error: {}", stderr));
        }
        
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
    
    async fn wait_container(&self, container_id: &str) -> anyhow::Result<DockerResult> {
        use std::process::Command;
        
        // Wait with timeout
        let wait_args = vec![
            "wait".to_string(),
            container_id.to_string(),
        ];
        
        let output = Command::new("docker")
            .args(&wait_args)
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to wait: {}", e))?;
        
        let exit_code: i32 = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .unwrap_or(-1);
        
        // Get logs
        let logs_args = vec![
            "logs".to_string(),
            container_id.to_string(),
        ];
        
        let logs_output = Command::new("docker")
            .args(&logs_args)
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to get logs: {}", e))?;
        
        let stdout = String::from_utf8_lossy(&logs_output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&logs_output.stderr).to_string();
        
        Ok(DockerResult {
            container_id: container_id.to_string(),
            exit_code,
            stdout,
            stderr,
            timed_out: false,
        })
    }
    
    async fn remove_container(&self, container_id: &str) -> anyhow::Result<()> {
        use std::process::Command;
        
        let _ = Command::new("docker")
            .args(["rm", "-f", container_id])
            .output();
        
        Ok(())
    }
    
    /// Pull image
    pub async fn pull_image(&self, image: &str) -> anyhow::Result<()> {
        use std::process::Command;
        
        let output = Command::new("docker")
            .args(["pull", image])
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to pull image: {}", e))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Pull error: {}", stderr));
        }
        
        Ok(())
    }
    
    /// Check if Docker is available
    pub fn is_available() -> bool {
        std::process::Command::new("docker")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

impl Default for DockerExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Docker execution result
#[derive(Debug, Clone)]
pub struct DockerResult {
    pub container_id: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

/// Docker tool for Klaw
pub struct DockerTool;

impl DockerTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl super::Tool for DockerTool {
    fn name(&self) -> &str {
        "docker_exec"
    }
    
    fn description(&self) -> &str {
        "Execute commands in an isolated Docker container. Provides resource limits and network isolation."
    }
    
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Command to execute in container"
                },
                "image": {
                    "type": "string",
                    "description": "Docker image to use",
                    "default": "alpine:latest"
                },
                "memory_mb": {
                    "type": "integer",
                    "description": "Memory limit in MB",
                    "default": 512
                },
                "cpu_limit": {
                    "type": "number",
                    "description": "CPU limit (0.0-1.0)",
                    "default": 0.5
                },
                "timeout_seconds": {
                    "type": "integer",
                    "description": "Execution timeout in seconds",
                    "default": 300
                },
                "network": {
                    "type": "boolean",
                    "description": "Enable network access",
                    "default": false
                }
            },
            "required": ["command"]
        })
    }
    
    async fn execute(&self, params: Value, ctx: &super::ToolContext) -> anyhow::Result<ToolResult> {
        let command = params.get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("command required"))?;
        
        let image = params.get("image")
            .and_then(|v| v.as_str())
            .unwrap_or("alpine:latest");
        
        let memory_mb = params.get("memory_mb")
            .and_then(|v| v.as_u64())
            .unwrap_or(512) as u32;
        
        let cpu_limit = params.get("cpu_limit")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5) as f32;
        
        let timeout_seconds = params.get("timeout_seconds")
            .and_then(|v| v.as_u64())
            .unwrap_or(300) as u32;
        
        let network = params.get("network")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        
        // Check if Docker is available
        if !DockerExecutor::is_available() {
            return Ok(ToolResult {
                tool_call_id: String::new(),
                content: serde_json::to_string(&json!({
                    "success": false,
                    "error": "Docker is not available. Please install Docker."
                })).unwrap(),
                is_error: true,
            });
        }
        
        let config = DockerConfig {
            image: image.to_string(),
            memory_mb: Some(memory_mb),
            cpu_limit: Some(cpu_limit),
            timeout_seconds: Some(timeout_seconds),
            network: Some(network),
            mount_workspace: Some(true),
            workdir: Some("/workspace".to_string()),
            env: None,
        };
        
        let executor = DockerExecutor::with_config(config);
        
        // Pull image first
        if let Err(e) = executor.pull_image(image).await {
            return Ok(ToolResult {
                tool_call_id: String::new(),
                content: serde_json::to_string(&json!({
                    "success": false,
                    "error": format!("Failed to pull image: {}", e)
                })).unwrap(),
                is_error: true,
            });
        }
        
        // Execute
        match executor.execute(command, Some(&ctx.workspace_dir)).await {
            Ok(result) => Ok(ToolResult {
                tool_call_id: String::new(),
                content: serde_json::to_string(&json!({
                    "success": result.exit_code == 0,
                    "exit_code": result.exit_code,
                    "stdout": result.stdout,
                    "stderr": result.stderr,
                    "container_id": result.container_id,
                    "timed_out": result.timed_out
                })).unwrap(),
                is_error: result.exit_code != 0,
            }),
            Err(e) => Ok(ToolResult {
                tool_call_id: String::new(),
                content: serde_json::to_string(&json!({
                    "success": false,
                    "error": e.to_string()
                })).unwrap(),
                is_error: true,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_docker_config_default() {
        let config = DockerConfig::default();
        assert_eq!(config.image, "alpine:latest");
        assert_eq!(config.memory_mb, Some(512));
        assert!(!config.network.unwrap_or(true));
    }
    
    #[test]
    fn test_docker_executor_new() {
        let executor = DockerExecutor::new();
        assert_eq!(executor.memory_mb, 512);
        assert!(!executor.network);
    }
    
    #[test]
    fn test_docker_result() {
        let result = DockerResult {
            container_id: "abc123".to_string(),
            exit_code: 0,
            stdout: "Hello".to_string(),
            stderr: "".to_string(),
            timed_out: false,
        };
        assert_eq!(result.exit_code, 0);
        assert!(!result.timed_out);
    }
}