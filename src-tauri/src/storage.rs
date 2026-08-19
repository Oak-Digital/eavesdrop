use std::{path::Path, sync::Mutex};

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};

use crate::{
    error::{AppError, AppResult},
    models::{
        AppSettings, CaptureMode, Highlight, Recording, SettingsPatch, Transcript,
        TranscriptSegment,
    },
};

pub struct Repository {
    connection: Mutex<Connection>,
}

pub struct RecordingSecrets {
    pub asset_path: Option<String>,
    pub journal_path: Option<String>,
    pub wrapped_key: Vec<u8>,
}

impl Repository {
    pub fn open(path: &Path) -> AppResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(path)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        let repository = Self {
            connection: Mutex::new(connection),
        };
        repository.migrate()?;
        Ok(repository)
    }

    fn migrate(&self) -> AppResult<()> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| AppError::Storage("database lock poisoned".into()))?;
        connection.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS recordings (
              id TEXT PRIMARY KEY,
              title TEXT NOT NULL,
              mode TEXT NOT NULL CHECK(mode IN ('in_person', 'online')),
              started_at TEXT NOT NULL,
              ended_at TEXT,
              duration_ms INTEGER NOT NULL DEFAULT 0,
              playable_duration_ms INTEGER NOT NULL DEFAULT 0,
              status TEXT NOT NULL,
              size_bytes INTEGER NOT NULL DEFAULT 0,
              codec TEXT NOT NULL DEFAULT 'AAC-LC',
              detected_app TEXT,
              deleted_at TEXT,
              asset_path TEXT,
              journal_path TEXT,
              wrapped_key BLOB NOT NULL,
              created_at TEXT NOT NULL,
              updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS highlights (
              id TEXT PRIMARY KEY,
              recording_id TEXT NOT NULL REFERENCES recordings(id) ON DELETE CASCADE,
              offset_ms INTEGER NOT NULL,
              created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS transcripts (
              recording_id TEXT PRIMARY KEY REFERENCES recordings(id) ON DELETE CASCADE,
              text TEXT NOT NULL,
              language TEXT,
              created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS transcript_segments (
              recording_id TEXT NOT NULL REFERENCES recordings(id) ON DELETE CASCADE,
              segment_index INTEGER NOT NULL,
              start_ms INTEGER NOT NULL,
              end_ms INTEGER NOT NULL,
              text TEXT NOT NULL,
              PRIMARY KEY(recording_id, segment_index)
            );
            CREATE TABLE IF NOT EXISTS detected_meetings (
              id TEXT PRIMARY KEY,
              app TEXT NOT NULL,
              display_name TEXT NOT NULL,
              detected_at TEXT NOT NULL,
              ended_at TEXT,
              dismissed_at TEXT
            );
            CREATE TABLE IF NOT EXISTS delivery_attempts (
              id TEXT PRIMARY KEY,
              recording_id TEXT NOT NULL REFERENCES recordings(id) ON DELETE CASCADE,
              destination_id TEXT NOT NULL,
              project_id TEXT,
              status TEXT NOT NULL,
              remote_asset_id TEXT,
              error_code TEXT,
              created_at TEXT NOT NULL,
              updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS settings (
              singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
              onboarding_completed INTEGER NOT NULL,
              meeting_detection_enabled INTEGER NOT NULL,
              launch_at_login INTEGER NOT NULL,
              microphone_id TEXT
            );
            INSERT OR IGNORE INTO settings(singleton, onboarding_completed, meeting_detection_enabled, launch_at_login)
              VALUES(1, 0, 1, 0);
            CREATE INDEX IF NOT EXISTS idx_recordings_started_at ON recordings(started_at DESC);
            CREATE INDEX IF NOT EXISTS idx_recordings_deleted_at ON recordings(deleted_at);
            "
        )?;
        ensure_column(&connection, "settings", "whisper_model_path", "TEXT")?;
        Ok(())
    }

    pub fn settings(&self) -> AppResult<AppSettings> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| AppError::Storage("database lock poisoned".into()))?;
        connection.query_row(
            "SELECT onboarding_completed, meeting_detection_enabled, launch_at_login, microphone_id, whisper_model_path FROM settings WHERE singleton = 1",
            [],
            |row| Ok(AppSettings {
                onboarding_completed: row.get::<_, i64>(0)? != 0,
                meeting_detection_enabled: row.get::<_, i64>(1)? != 0,
                launch_at_login: row.get::<_, i64>(2)? != 0,
                microphone_id: row.get(3)?,
                whisper_model_path: row.get(4)?,
            }),
        ).map_err(Into::into)
    }

    pub fn update_settings(&self, patch: &SettingsPatch) -> AppResult<AppSettings> {
        let current = self.settings()?;
        let next = AppSettings {
            onboarding_completed: patch
                .onboarding_completed
                .unwrap_or(current.onboarding_completed),
            meeting_detection_enabled: patch
                .meeting_detection_enabled
                .unwrap_or(current.meeting_detection_enabled),
            launch_at_login: patch.launch_at_login.unwrap_or(current.launch_at_login),
            microphone_id: patch.microphone_id.clone().unwrap_or(current.microphone_id),
            whisper_model_path: patch
                .whisper_model_path
                .clone()
                .unwrap_or(current.whisper_model_path),
        };
        let connection = self
            .connection
            .lock()
            .map_err(|_| AppError::Storage("database lock poisoned".into()))?;
        connection.execute(
            "UPDATE settings SET onboarding_completed = ?1, meeting_detection_enabled = ?2, launch_at_login = ?3, microphone_id = ?4, whisper_model_path = ?5 WHERE singleton = 1",
            params![next.onboarding_completed, next.meeting_detection_enabled, next.launch_at_login, next.microphone_id, next.whisper_model_path],
        )?;
        Ok(next)
    }

    pub fn insert_recording(
        &self,
        recording: &Recording,
        journal_path: &str,
        wrapped_key: &[u8],
    ) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        let connection = self
            .connection
            .lock()
            .map_err(|_| AppError::Storage("database lock poisoned".into()))?;
        connection.execute(
            "INSERT INTO recordings(id, title, mode, started_at, status, codec, detected_app, journal_path, wrapped_key, created_at, updated_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
            params![
                recording.id,
                recording.title,
                mode_string(recording.mode),
                recording.started_at,
                recording.status,
                recording.codec,
                recording.detected_app,
                journal_path,
                wrapped_key,
                now,
            ],
        )?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn finalize_recording(
        &self,
        id: &str,
        ended_at: &str,
        duration_ms: i64,
        playable_ms: i64,
        size_bytes: i64,
        asset_path: &str,
        recovered: bool,
    ) -> AppResult<()> {
        let status = if recovered { "recovered" } else { "ready" };
        let connection = self
            .connection
            .lock()
            .map_err(|_| AppError::Storage("database lock poisoned".into()))?;
        connection.execute(
            "UPDATE recordings SET ended_at=?2, duration_ms=?3, playable_duration_ms=?4, size_bytes=?5, asset_path=?6, journal_path=NULL, status=?7, updated_at=?2 WHERE id=?1",
            params![id, ended_at, duration_ms, playable_ms, size_bytes, asset_path, status],
        )?;
        Ok(())
    }

    pub fn add_highlight(&self, recording_id: &str, offset_ms: i64) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        let connection = self
            .connection
            .lock()
            .map_err(|_| AppError::Storage("database lock poisoned".into()))?;
        connection.execute(
            "INSERT INTO highlights(id, recording_id, offset_ms, created_at) VALUES(?1, ?2, ?3, ?4)",
            params![uuid::Uuid::new_v4().to_string(), recording_id, offset_ms, now],
        )?;
        Ok(())
    }

    pub fn list_recordings(&self, deleted_only: bool) -> AppResult<Vec<Recording>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| AppError::Storage("database lock poisoned".into()))?;
        let predicate = if deleted_only {
            "deleted_at IS NOT NULL"
        } else {
            "deleted_at IS NULL"
        };
        let mut statement = connection.prepare(&format!(
            "SELECT id,title,mode,started_at,ended_at,duration_ms,playable_duration_ms,status,size_bytes,codec,detected_app,deleted_at FROM recordings WHERE {predicate} ORDER BY started_at DESC"
        ))?;
        let recordings = statement
            .query_map([], recording_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        recordings
            .into_iter()
            .map(|mut recording| {
                recording.highlights = highlights_for(&connection, &recording.id)?;
                recording.transcript = transcript_for(&connection, &recording.id)?;
                Ok(recording)
            })
            .collect()
    }

    pub fn recording(&self, id: &str) -> AppResult<Recording> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| AppError::Storage("database lock poisoned".into()))?;
        let mut recording = connection.query_row(
            "SELECT id,title,mode,started_at,ended_at,duration_ms,playable_duration_ms,status,size_bytes,codec,detected_app,deleted_at FROM recordings WHERE id=?1",
            [id],
            recording_from_row,
        ).optional()?.ok_or(AppError::NotFound)?;
        recording.highlights = highlights_for(&connection, id)?;
        recording.transcript = transcript_for(&connection, id)?;
        Ok(recording)
    }

    pub fn save_transcript(&self, recording_id: &str, transcript: &Transcript) -> AppResult<()> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| AppError::Storage("database lock poisoned".into()))?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO transcripts(recording_id, text, language, created_at) VALUES(?1, ?2, ?3, ?4)
             ON CONFLICT(recording_id) DO UPDATE SET text=excluded.text, language=excluded.language, created_at=excluded.created_at",
            params![recording_id, transcript.text, transcript.language, transcript.created_at],
        )?;
        transaction.execute(
            "DELETE FROM transcript_segments WHERE recording_id=?1",
            [recording_id],
        )?;
        for (index, segment) in transcript.segments.iter().enumerate() {
            transaction.execute(
                "INSERT INTO transcript_segments(recording_id, segment_index, start_ms, end_ms, text) VALUES(?1, ?2, ?3, ?4, ?5)",
                params![recording_id, index as i64, segment.start_ms, segment.end_ms, segment.text],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn secrets(&self, id: &str) -> AppResult<RecordingSecrets> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| AppError::Storage("database lock poisoned".into()))?;
        connection
            .query_row(
                "SELECT asset_path, journal_path, wrapped_key FROM recordings WHERE id=?1",
                [id],
                |row| {
                    Ok(RecordingSecrets {
                        asset_path: row.get(0)?,
                        journal_path: row.get(1)?,
                        wrapped_key: row.get(2)?,
                    })
                },
            )
            .optional()?
            .ok_or(AppError::NotFound)
    }

    pub fn unfinished_recordings(&self) -> AppResult<Vec<String>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| AppError::Storage("database lock poisoned".into()))?;
        let mut statement = connection.prepare(
            "SELECT id FROM recordings WHERE status='recording' AND journal_path IS NOT NULL",
        )?;
        Ok(statement
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?)
    }

    pub fn rename_recording(&self, id: &str, title: &str) -> AppResult<Recording> {
        let title = title.trim();
        if title.is_empty() || title.chars().count() > 160 {
            return Err(AppError::State(
                "recording title must be between 1 and 160 characters".into(),
            ));
        }
        let connection = self
            .connection
            .lock()
            .map_err(|_| AppError::Storage("database lock poisoned".into()))?;
        connection.execute(
            "UPDATE recordings SET title=?2, updated_at=?3 WHERE id=?1",
            params![id, title, Utc::now().to_rfc3339()],
        )?;
        drop(connection);
        self.recording(id)
    }

    pub fn set_deleted(&self, id: &str, deleted: bool) -> AppResult<()> {
        self.set_deleted_many(&[id.to_string()], deleted)
    }

    pub fn set_deleted_many(&self, ids: &[String], deleted: bool) -> AppResult<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| AppError::Storage("database lock poisoned".into()))?;
        let transaction = connection.transaction()?;
        let value = deleted.then(|| Utc::now().to_rfc3339());
        let updated_at = Utc::now().to_rfc3339();
        for id in ids {
            let changed = transaction.execute(
                "UPDATE recordings SET deleted_at=?2, updated_at=?3 WHERE id=?1",
                params![id, value, updated_at],
            )?;
            if changed == 0 {
                return Err(AppError::NotFound);
            }
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn purge_expired(&self) -> AppResult<Vec<String>> {
        let cutoff = (Utc::now() - chrono::Duration::days(7)).to_rfc3339();
        let connection = self
            .connection
            .lock()
            .map_err(|_| AppError::Storage("database lock poisoned".into()))?;
        let mut statement = connection.prepare(
            "SELECT asset_path FROM recordings WHERE deleted_at IS NOT NULL AND deleted_at < ?1",
        )?;
        let paths = statement
            .query_map([&cutoff], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        drop(statement);
        connection.execute(
            "DELETE FROM recordings WHERE deleted_at IS NOT NULL AND deleted_at < ?1",
            [&cutoff],
        )?;
        Ok(paths)
    }
}

fn mode_string(mode: CaptureMode) -> &'static str {
    match mode {
        CaptureMode::InPerson => "in_person",
        CaptureMode::Online => "online",
    }
}

fn recording_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Recording> {
    let mode: String = row.get(2)?;
    Ok(Recording {
        id: row.get(0)?,
        title: row.get(1)?,
        mode: if mode == "online" {
            CaptureMode::Online
        } else {
            CaptureMode::InPerson
        },
        started_at: row.get(3)?,
        ended_at: row.get(4)?,
        duration_ms: row.get(5)?,
        playable_duration_ms: row.get(6)?,
        status: row.get(7)?,
        size_bytes: row.get(8)?,
        codec: row.get(9)?,
        detected_app: row.get(10)?,
        deleted_at: row.get(11)?,
        highlights: Vec::new(),
        transcript: None,
    })
}

fn transcript_for(connection: &Connection, recording_id: &str) -> AppResult<Option<Transcript>> {
    let transcript = connection
        .query_row(
            "SELECT text,language,created_at FROM transcripts WHERE recording_id=?1",
            [recording_id],
            |row| {
                Ok(Transcript {
                    text: row.get(0)?,
                    language: row.get(1)?,
                    created_at: row.get(2)?,
                    segments: Vec::new(),
                })
            },
        )
        .optional()?;
    let Some(mut transcript) = transcript else {
        return Ok(None);
    };
    let mut statement = connection.prepare(
        "SELECT start_ms,end_ms,text FROM transcript_segments WHERE recording_id=?1 ORDER BY segment_index",
    )?;
    transcript.segments = statement
        .query_map([recording_id], |row| {
            Ok(TranscriptSegment {
                start_ms: row.get(0)?,
                end_ms: row.get(1)?,
                text: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(transcript))
}

fn ensure_column(
    connection: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> AppResult<()> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;
    if !columns.iter().any(|existing| existing == column) {
        connection.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
            [],
        )?;
    }
    Ok(())
}

fn highlights_for(connection: &Connection, recording_id: &str) -> AppResult<Vec<Highlight>> {
    let mut statement = connection.prepare(
        "SELECT id,offset_ms,created_at FROM highlights WHERE recording_id=?1 ORDER BY offset_ms",
    )?;
    Ok(statement
        .query_map([recording_id], |row| {
            Ok(Highlight {
                id: row.get(0)?,
                offset_ms: row.get(1)?,
                created_at: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_recording(id: &str) -> Recording {
        Recording {
            id: id.into(),
            title: format!("Recording {id}"),
            mode: CaptureMode::InPerson,
            started_at: Utc::now().to_rfc3339(),
            ended_at: None,
            duration_ms: 0,
            playable_duration_ms: 0,
            status: "recording".into(),
            size_bytes: 0,
            codec: "AAC-LC".into(),
            detected_app: None,
            deleted_at: None,
            highlights: Vec::new(),
            transcript: None,
        }
    }

    #[test]
    fn migrations_and_settings_are_stable() {
        let temp = tempfile::tempdir().unwrap();
        let repository = Repository::open(&temp.path().join("library.sqlite3")).unwrap();
        assert!(repository.settings().unwrap().meeting_detection_enabled);
        let updated = repository
            .update_settings(&SettingsPatch {
                onboarding_completed: Some(true),
                whisper_model_path: Some(Some("model.bin".into())),
                ..Default::default()
            })
            .unwrap();
        assert!(updated.onboarding_completed);
        assert_eq!(updated.whisper_model_path.as_deref(), Some("model.bin"));
        let reopened = Repository::open(&temp.path().join("library.sqlite3")).unwrap();
        assert_eq!(
            reopened.settings().unwrap().whisper_model_path.as_deref(),
            Some("model.bin")
        );
    }

    #[test]
    fn bulk_delete_and_restore_are_atomic() {
        let temp = tempfile::tempdir().unwrap();
        let repository = Repository::open(&temp.path().join("library.sqlite3")).unwrap();
        for id in ["one", "two"] {
            repository
                .insert_recording(&test_recording(id), "journal", &[0; 32])
                .unwrap();
        }

        repository
            .set_deleted_many(&["one".into(), "two".into()], true)
            .unwrap();
        assert!(repository.list_recordings(false).unwrap().is_empty());
        assert_eq!(repository.list_recordings(true).unwrap().len(), 2);

        let error = repository
            .set_deleted_many(&["one".into(), "missing".into()], false)
            .unwrap_err();
        assert!(matches!(error, AppError::NotFound));
        assert_eq!(repository.list_recordings(true).unwrap().len(), 2);

        repository
            .set_deleted_many(&["one".into(), "two".into()], false)
            .unwrap();
        assert_eq!(repository.list_recordings(false).unwrap().len(), 2);
    }

    #[test]
    fn timestamped_transcripts_round_trip() {
        let temp = tempfile::tempdir().unwrap();
        let repository = Repository::open(&temp.path().join("library.sqlite3")).unwrap();
        repository
            .insert_recording(&test_recording("one"), "journal", &[0; 32])
            .unwrap();
        let transcript = Transcript {
            text: "Hello there.".into(),
            language: Some("English".into()),
            created_at: Utc::now().to_rfc3339(),
            segments: vec![TranscriptSegment {
                start_ms: 120,
                end_ms: 840,
                text: "Hello there.".into(),
            }],
        };

        repository.save_transcript("one", &transcript).unwrap();

        assert_eq!(
            repository.recording("one").unwrap().transcript,
            Some(transcript)
        );
    }
}
