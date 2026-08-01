use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RecentEntry {
    pub id: String,
    pub path: String,
    pub title: String,
    pub mode: String, // "doc" | "sheet" | "slide"
    pub last_opened_at: u64,
    pub pinned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct RecentStore {
    pub entries: Vec<RecentEntry>,
}

impl RecentStore {
    pub fn add(&mut self, path: String, title: String, mode: String) -> RecentEntry {
        // Check if path already exists
        if let Some(existing) = self.entries.iter_mut().find(|e| e.path == path) {
            existing.last_opened_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            existing.title = title;
            return existing.clone();
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let entry = RecentEntry {
            id: uuid::Uuid::now_v7().to_string(),
            path,
            title,
            mode,
            last_opened_at: now,
            pinned: false,
        };

        self.entries.insert(0, entry.clone());
        if self.entries.len() > 50 {
            // Keep pinned or trim from end
            let mut i = self.entries.len() - 1;
            while self.entries.len() > 50 && i > 0 {
                if !self.entries[i].pinned {
                    self.entries.remove(i);
                }
                i -= 1;
            }
        }
        entry
    }

    pub fn toggle_pin(&mut self, id: &str) -> bool {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.pinned = !entry.pinned;
            return entry.pinned;
        }
        false
    }
}
