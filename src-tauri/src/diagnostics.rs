use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use chrono::Utc;

use crate::error::AppResult;

pub struct Diagnostics {
    root: PathBuf,
}

impl Diagnostics {
    pub fn open(app_data: &Path) -> AppResult<Self> {
        let root = app_data.join("diagnostics");
        fs::create_dir_all(&root)?;
        let cutoff = SystemTime::now()
            .checked_sub(Duration::from_secs(7 * 24 * 60 * 60))
            .unwrap_or(SystemTime::UNIX_EPOCH);
        for entry in fs::read_dir(&root)?.flatten() {
            if entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .is_ok_and(|modified| modified < cutoff)
            {
                let _ = fs::remove_file(entry.path());
            }
        }
        let diagnostics = Self { root };
        diagnostics.event("application_started")?;
        Ok(diagnostics)
    }

    pub fn event(&self, name: &str) -> AppResult<()> {
        let safe_name: String = name
            .chars()
            .filter(|character| character.is_ascii_lowercase() || *character == '_')
            .take(64)
            .collect();
        let path = self
            .root
            .join(format!("{}.log", Utc::now().format("%Y-%m-%d")));
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        writeln!(file, "{} {safe_name}", Utc::now().to_rfc3339())?;
        Ok(())
    }

    pub fn export(&self, destination: &Path) -> AppResult<()> {
        let mut entries: Vec<_> = fs::read_dir(&self.root)?.flatten().collect();
        entries.sort_by_key(|entry| entry.file_name());
        let mut output = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(destination)?;
        for entry in entries {
            if entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "log")
            {
                output.write_all(&fs::read(entry.path())?)?;
            }
        }
        output.sync_all()?;
        Ok(())
    }
}
