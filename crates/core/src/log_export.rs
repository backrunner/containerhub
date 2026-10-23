use crate::{ErrorKind, LogSnapshot, ManagementError};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogExport {
    pub file_name: String,
    pub content: String,
    pub line_count: usize,
    pub snapshot_lines: usize,
    pub truncated: bool,
}
impl LogSnapshot {
    /// Freeze the bounded, filtered snapshot. This is an export payload, never a host file path.
    pub fn export(&self, query: &str) -> Result<LogExport, ManagementError> {
        if query.len() > 4096 {
            return Err(ManagementError::new(
                ErrorKind::InvalidData,
                "日志筛选条件过长",
            ));
        }
        let mut raw = self.lines.join("\n");
        if !self.lines.is_empty() {
            raw.push('\n');
        }
        let safe = Self::from_bytes(self.key.clone(), self.source, raw.as_bytes());
        let filter = query.trim().to_lowercase();
        let lines: Vec<_> = safe
            .lines
            .iter()
            .filter(|line| line.to_lowercase().contains(&filter))
            .collect();
        let truncated = self.truncated || safe.truncated;
        let metadata = serde_json::json!({"resource":self.key,"source":self.source,"observed_at":self.observed_at,"query":query.trim(),"exported_lines":lines.len(),"snapshot_lines":safe.lines.len(),"locally_truncated":truncated,"scope":"bounded tail; not complete container history"});
        let mut content = format!("# ContainerHub log export\n# {metadata}\n\n");
        for line in &lines {
            content.push_str(line);
            content.push('\n');
        }
        let name: String = self
            .key
            .id
            .chars()
            .take(80)
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        Ok(LogExport {
            file_name: format!(
                "containerhub-{name}-{:?}-{}.log",
                self.source, self.observed_at
            ),
            content,
            line_count: lines.len(),
            snapshot_lines: safe.lines.len(),
            truncated,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn export_freezes_filtered_tail_and_preserves_scope_and_identity() {
        let mut snapshot = LogSnapshot::from_bytes(
            crate::ResourceKey {
                environment: "test".into(),
                provider: crate::ProviderKind::Apple,
                namespace: None,
                id: "../app".into(),
                incarnation: "v1".into(),
            },
            crate::LogSource::Boot,
            b"\x1b[31mERROR failed\x1b[0m\nINFO ready\n",
        );
        snapshot.observed_at = 123;
        snapshot.truncated = true;
        let export = snapshot.export(" error ").unwrap();
        assert_eq!(export.line_count, 1);
        assert_eq!(export.snapshot_lines, 2);
        assert!(export.truncated);
        assert!(export.content.ends_with("ERROR failed\n"));
        assert!(export.content.contains("\"observed_at\":123"));
        assert!(!export.content.contains('\x1b'));
        assert!(!export.file_name.contains('/'));
        snapshot.lines.clear();
        assert!(export.content.contains("failed"));
        assert!(snapshot.export(&"x".repeat(4097)).is_err());
    }
}
