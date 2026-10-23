use crate::{ResourceKey, now};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

pub const MAX_LINES: usize = 500;
pub const MAX_BYTES: usize = 256 * 1024;
const MAX_LINE_CHARS: usize = 4096;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogSource {
    #[default]
    Standard,
    Boot,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogSnapshot {
    pub key: ResourceKey,
    pub source: LogSource,
    pub lines: Vec<String>,
    pub observed_at: u64,
    /// The CLI may already have returned a tail. This flag only describes local truncation.
    pub truncated: bool,
}

impl LogSnapshot {
    /// Display text only: remove terminal control sequences, retain bounded recent lines,
    /// and tolerate non-UTF8 application output. This data is never written to our journal.
    pub fn from_bytes(key: ResourceKey, source: LogSource, bytes: &[u8]) -> Self {
        #[derive(Clone, Copy)]
        enum AnsiState {
            Text,
            Escape,
            Csi,
            String,
            StringEscape,
        }
        let mut state = AnsiState::Text;
        let mut lines = VecDeque::new();
        let mut line = String::new();
        let mut chars = 0;
        let mut size = 0;
        let mut truncated = false;
        let mut push = |line: String| {
            size += line.len();
            lines.push_back(line);
            while lines.len() > MAX_LINES || size > MAX_BYTES {
                size -= lines.pop_front().unwrap().len();
                truncated = true;
            }
        };
        let mut line_truncated = false;
        for c in String::from_utf8_lossy(bytes).chars() {
            match state {
                AnsiState::Text => match c {
                    '\u{1b}' => state = AnsiState::Escape,
                    '\u{9b}' => state = AnsiState::Csi,
                    '\u{9d}' | '\u{90}' | '\u{98}' | '\u{9e}' | '\u{9f}' => {
                        state = AnsiState::String
                    }
                    '\n' => {
                        push(std::mem::take(&mut line));
                        chars = 0;
                    }
                    '\t' => {
                        if chars < MAX_LINE_CHARS {
                            line.push(' ');
                            chars += 1;
                        }
                    }
                    c if c.is_control() => {}
                    c => {
                        if chars < MAX_LINE_CHARS {
                            line.push(c);
                            chars += 1;
                        } else if chars == MAX_LINE_CHARS {
                            line.push('…');
                            chars += 1;
                            line_truncated = true;
                        }
                    }
                },
                AnsiState::Escape => {
                    state = match c {
                        '[' => AnsiState::Csi,
                        ']' | 'P' | 'X' | '^' | '_' => AnsiState::String,
                        _ => AnsiState::Text,
                    }
                }
                AnsiState::Csi => {
                    if ('\u{40}'..='\u{7e}').contains(&c) {
                        state = AnsiState::Text;
                    }
                }
                AnsiState::String => match c {
                    '\u{7}' | '\u{9c}' => state = AnsiState::Text,
                    '\u{1b}' => state = AnsiState::StringEscape,
                    _ => {}
                },
                AnsiState::StringEscape => {
                    state = if c == '\\' {
                        AnsiState::Text
                    } else {
                        AnsiState::String
                    }
                }
            }
        }
        if !line.is_empty() {
            push(line);
        }
        Self {
            key,
            source,
            lines: lines.into(),
            observed_at: now(),
            truncated: truncated || line_truncated,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn logs(bytes: &[u8]) -> LogSnapshot {
        LogSnapshot::from_bytes(
            ResourceKey {
                provider: crate::ProviderKind::Apple,
                namespace: None,
                environment: "test".into(),
                id: "app".into(),
                incarnation: "1".into(),
            },
            LogSource::Standard,
            bytes,
        )
    }
    #[test]
    fn terminal_sequences_and_invalid_utf8_do_not_reach_the_view() {
        let result = logs(b"\x1b[31mready\x1b[0m\n\x1b]52;c;secret\x07safe\n\xff\x00ok\n");
        assert_eq!(result.lines, ["ready", "safe", "�ok"]);
        assert!(!result.truncated);
    }
    #[test]
    fn keeps_recent_lines_and_bounds_multibyte_and_very_long_lines() {
        let input = (0..600).map(|i| format!("line {i}\n")).collect::<String>();
        let result = logs(input.as_bytes());
        assert_eq!(result.lines.len(), 500);
        assert_eq!(result.lines.first().unwrap(), "line 100");
        assert_eq!(result.lines.last().unwrap(), "line 599");
        assert!(result.truncated);
        let result = logs(("界".repeat(5000) + "\n").repeat(500).as_bytes());
        assert!(result.truncated);
        assert!(result.lines.iter().map(String::len).sum::<usize>() <= MAX_BYTES);
        assert!(
            result
                .lines
                .iter()
                .all(|s| s.chars().count() <= MAX_LINE_CHARS + 1)
        );
    }
}
