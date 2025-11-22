//! PlantUML subprocess executor
//!
//! Handles calling PlantUML JAR as subprocess for diagram generation.

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

/// Output format for PlantUML diagrams
#[derive(Debug, Clone, Copy)]
pub enum DiagramFormat {
    Svg,
    Png,
    Txt, // Syntax check
}

impl DiagramFormat {
    pub fn as_flag(&self) -> &'static str {
        match self {
            Self::Svg => "-tsvg",
            Self::Png => "-tpng",
            Self::Txt => "-txt",
        }
    }

    pub fn content_type(&self) -> &'static str {
        match self {
            Self::Svg => "image/svg+xml",
            Self::Png => "image/png",
            Self::Txt => "text/plain",
        }
    }
}

/// PlantUML executor configuration
pub struct PlantUMLExecutor {
    jar_path: PathBuf,
}

impl PlantUMLExecutor {
    pub fn new() -> Result<Self> {
        let jar_path = std::env::var("PLANTUML_JAR")
            .unwrap_or_else(|_| "/opt/plantuml/plantuml.jar".to_string())
            .into();

        Ok(Self { jar_path })
    }

    /// Generate diagram from PlantUML source code
    ///
    /// # Arguments
    /// * `source` - PlantUML diagram source code
    /// * `format` - Output format (SVG, PNG, TXT)
    ///
    /// # Returns
    /// Generated diagram bytes
    pub async fn generate(&self, source: &str, format: DiagramFormat) -> Result<Vec<u8>> {
        tracing::debug!(
            "Generating {} diagram ({} bytes source)",
            format.as_flag(),
            source.len()
        );

        // Create temporary directory for PlantUML output (unused but kept for future file-based mode)
        let _temp_dir = tempfile::tempdir().context("Failed to create temp directory")?;

        // Write source to stdin, read from stdout (pipe mode)
        let mut child = Command::new("java")
            .args(&[
                "-jar",
                self.jar_path.to_str().unwrap(),
                format.as_flag(),
                "-pipe", // Read from stdin, write to stdout
                "-charset",
                "UTF-8",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to spawn PlantUML process")?;

        // Write source to stdin
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(source.as_bytes())
                .await
                .context("Failed to write to PlantUML stdin")?;
            stdin.flush().await?;
            drop(stdin); // Close stdin to signal EOF
        }

        // Wait for process to complete
        let output = child
            .wait_with_output()
            .await
            .context("Failed to wait for PlantUML process")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("PlantUML process failed: {}", stderr);
        }

        // Check if output is empty (syntax error)
        if output.stdout.is_empty() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("PlantUML generated empty output. Stderr: {}", stderr);
        }

        tracing::debug!("Generated {} bytes output", output.stdout.len());

        Ok(output.stdout)
    }

    /// Validate PlantUML syntax
    pub async fn validate(&self, source: &str) -> Result<String> {
        let output = self.generate(source, DiagramFormat::Txt).await?;
        String::from_utf8(output).context("Failed to decode PlantUML text output")
    }
}

impl Default for PlantUMLExecutor {
    fn default() -> Self {
        Self::new().expect("Failed to create PlantUMLExecutor")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Only run when PlantUML JAR is available
    async fn test_generate_svg() {
        let executor = PlantUMLExecutor::new().unwrap();
        let source = "@startuml\nAlice -> Bob: Hello\n@enduml";

        let result = executor.generate(source, DiagramFormat::Svg).await;
        assert!(result.is_ok());

        let svg = String::from_utf8(result.unwrap()).unwrap();
        assert!(svg.contains("<svg"));
    }

    #[tokio::test]
    #[ignore]
    async fn test_validate_syntax() {
        let executor = PlantUMLExecutor::new().unwrap();
        let source = "@startuml\nAlice -> Bob: Hello\n@enduml";

        let result = executor.validate(source).await;
        assert!(result.is_ok());
    }
}
