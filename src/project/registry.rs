use crate::config::Config;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEntry {
    pub path: String,
    pub name: String,
    pub files_indexed: usize,
    pub last_indexed: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ProjectRegistry {
    entries: Vec<ProjectEntry>,
}

impl ProjectRegistry {
    pub fn load() -> Result<Self> {
        let path = Config::registry_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        let registry: ProjectRegistry = serde_json::from_str(&content)?;
        Ok(registry)
    }

    pub fn save(&self) -> Result<()> {
        Config::ensure_home()?;
        let path = Config::registry_path()?;
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    pub fn add_project(&mut self, path: &Path, files_indexed: usize) {
        let path_str = path.display().to_string();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path_str.clone());
        
        let now = chrono_lite_timestamp();

        // Update existing or add new
        if let Some(entry) = self.entries.iter_mut().find(|e| e.path == path_str) {
            entry.files_indexed = files_indexed;
            entry.last_indexed = now;
        } else {
            self.entries.push(ProjectEntry {
                path: path_str,
                name,
                files_indexed,
                last_indexed: now,
            });
        }
    }

    pub fn remove_project(&mut self, path: &Path) {
        let path_str = path.display().to_string();
        self.entries.retain(|e| e.path != path_str);
    }

    pub fn projects(&self) -> &[ProjectEntry] {
        &self.entries
    }
}

fn chrono_lite_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    
    // Simple ISO-ish format without chrono dependency
    let days_since_epoch = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;
    
    // Approximate date calculation (good enough for display)
    let mut year = 1970;
    let mut remaining_days = days_since_epoch;
    
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }
    
    let days_in_months: [u64; 12] = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    
    let mut month = 1;
    for days in days_in_months {
        if remaining_days < days {
            break;
        }
        remaining_days -= days;
        month += 1;
    }
    let day = remaining_days + 1;
    
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", year, month, day, hours, minutes, seconds)
}

fn is_leap_year(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}
