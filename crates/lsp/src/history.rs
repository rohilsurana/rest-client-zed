use serde::{Deserialize, Serialize};

const MAX_ENTRIES: usize = 50;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub method: String,
    pub url: String,
    pub status: u16,
    pub elapsed_ms: u128,
    pub timestamp: u64,
}

#[derive(Debug, Default)]
pub struct History {
    entries: Vec<HistoryEntry>,
}

impl History {
    pub fn add(&mut self, entry: HistoryEntry) {
        self.entries.push(entry);
        if self.entries.len() > MAX_ENTRIES {
            self.entries.remove(0);
        }
    }

    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    pub fn format(&self) -> String {
        if self.entries.is_empty() {
            return "No request history.".to_string();
        }

        let mut out = String::new();
        for (i, entry) in self.entries.iter().rev().enumerate() {
            out.push_str(&format!(
                "{}. [{}] {} {} — {}ms\n",
                i + 1,
                entry.status,
                entry.method,
                entry.url,
                entry.elapsed_ms,
            ));
        }
        out
    }
}

/// Load history from a session file.
pub fn load(path: &std::path::Path) -> History {
    if let Ok(data) = std::fs::read_to_string(path) {
        if let Ok(entries) = serde_json::from_str::<Vec<HistoryEntry>>(&data) {
            return History { entries };
        }
    }
    History::default()
}

/// Save history to a session file.
pub fn save(path: &std::path::Path, history: &History) {
    if let Ok(json) = serde_json::to_string_pretty(history.entries()) {
        let _ = std::fs::write(path, json);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_format() {
        let mut h = History::default();
        h.add(HistoryEntry {
            method: "GET".to_string(),
            url: "https://example.com".to_string(),
            status: 200,
            elapsed_ms: 150,
            timestamp: 1000,
        });
        h.add(HistoryEntry {
            method: "POST".to_string(),
            url: "https://example.com/api".to_string(),
            status: 201,
            elapsed_ms: 300,
            timestamp: 2000,
        });
        assert_eq!(h.entries().len(), 2);
        let formatted = h.format();
        assert!(formatted.contains("POST"));
        assert!(formatted.contains("GET"));
    }

    #[test]
    fn test_max_entries() {
        let mut h = History::default();
        for i in 0..60 {
            h.add(HistoryEntry {
                method: "GET".to_string(),
                url: format!("https://example.com/{i}"),
                status: 200,
                elapsed_ms: 100,
                timestamp: i as u64,
            });
        }
        assert_eq!(h.entries().len(), MAX_ENTRIES);
        // Oldest entries should be trimmed
        assert!(h.entries()[0].url.contains("/10"));
    }
}
