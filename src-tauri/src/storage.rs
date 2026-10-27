use std::{path::Path, sync::Mutex};

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};

use crate::{
    error::{AppError, AppResult},
    models::{
        AppSettings, CaptureMode, Highlight, Recording, SettingsPatch, Summary, ThemePreference,
        Transcript, TranscriptSegment,
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
            CREATE TABLE IF NOT EXISTS summaries (
              recording_id TEXT PRIMARY KEY REFERENCES recordings(id) ON DELETE CASCADE,
              suggested_title TEXT NOT NULL,
              overview TEXT NOT NULL,
              model TEXT NOT NULL,
              created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS summary_points (
              recording_id TEXT NOT NULL REFERENCES recordings(id) ON DELETE CASCADE,
              kind TEXT NOT NULL CHECK(kind IN ('key_point', 'decision', 'action_item')),
              position INTEGER NOT NULL,
              text TEXT NOT NULL,
              PRIMARY KEY(recording_id, kind, position)
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
        ensure_column(&connection, "settings", "summary_model_path", "TEXT")?;
        ensure_column(&connection, "settings", "summary_prompt", "TEXT")?;
        ensure_column(
            &connection,
            "settings",
            "summary_source",
            "TEXT NOT NULL DEFAULT 'local'",
        )?;
        ensure_column(
            &connection,
            "settings",
            "cloud_provider",
            "TEXT NOT NULL DEFAULT 'openai'",
        )?;
        ensure_column(
            &connection,
            "settings",
            "cloud_model",
            "TEXT NOT NULL DEFAULT 'gpt-5.4-mini'",
        )?;
        ensure_column(
            &connection,
            "settings",
            "summary_template",
            "TEXT NOT NULL DEFAULT 'discussion'",
        )?;
        ensure_column(
            &connection,
            "settings",
            "transcription_source",
            "TEXT NOT NULL DEFAULT 'local'",
        )?;
        ensure_column(
            &connection,
            "settings",
            "auto_transcribe",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        ensure_column(
            &connection,
            "settings",
            "auto_summarize",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        ensure_column(&connection, "transcripts", "source", "TEXT")?;
        ensure_column(&connection, "transcripts", "model", "TEXT")?;
        ensure_column(&connection, "summaries", "markdown", "TEXT")?;
        ensure_column(&connection, "summaries", "template_id", "TEXT")?;
        ensure_column(
            &connection,
            "settings",
            "theme",
            "TEXT NOT NULL DEFAULT 'light'",
        )?;
        // The trash view no longer exists. Keep previously recoverable items
        // accessible until the user confirms permanent deletion.
        connection.execute(
            "UPDATE recordings SET deleted_at=NULL WHERE deleted_at IS NOT NULL",
            [],
        )?;
        Ok(())
    }

    pub fn settings(&self) -> AppResult<AppSettings> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| AppError::Storage("database lock poisoned".into()))?;
        connection.query_row(
            "SELECT onboarding_completed, meeting_detection_enabled, launch_at_login, microphone_id, whisper_model_path, summary_model_path, summary_prompt, theme, summary_template, summary_source, cloud_provider, cloud_model, transcription_source, auto_transcribe, auto_summarize FROM settings WHERE singleton = 1",
            [],
            |row| Ok(AppSettings {
                auto_transcribe: row.get::<_, i64>(13)? != 0,
                auto_summarize: row.get::<_, i64>(14)? != 0,
                onboarding_completed: row.get::<_, i64>(0)? != 0,
                meeting_detection_enabled: row.get::<_, i64>(1)? != 0,
                launch_at_login: row.get::<_, i64>(2)? != 0,
                microphone_id: row.get(3)?,
                whisper_model_path: row.get(4)?,
                summary_model_path: row.get(5)?,
                summary_prompt: row.get(6)?,
                summary_template: row.get(8)?,
                summary_source: if row.get::<_, String>(9)? == "cloud" { crate::cloud_summarization::SummarySource::Cloud } else { crate::cloud_summarization::SummarySource::Local },
                cloud_provider: if row.get::<_, String>(10)? == "anthropic" { crate::cloud_summarization::CloudProvider::Anthropic } else { crate::cloud_summarization::CloudProvider::Openai },
                cloud_model: row.get(11)?,
                transcription_source: if row.get::<_, String>(12)? == "cloud" { crate::cloud_transcription::TranscriptionSource::Cloud } else { crate::cloud_transcription::TranscriptionSource::Local },
                theme: ThemePreference::from_stored(&row.get::<_, String>(7)?),
            }),
        ).map_err(Into::into)
    }

    pub fn update_settings(&self, patch: &SettingsPatch) -> AppResult<AppSettings> {
        let current = self.settings()?;
        if let Some(id) = &patch.summary_template {
            crate::summary_templates::get(id).map_err(AppError::State)?;
        }
        if let Some(model) = &patch.cloud_model {
            crate::cloud_summarization::validate_model(model)?;
        }
        let next = AppSettings {
            auto_transcribe: patch.auto_transcribe.unwrap_or(current.auto_transcribe),
            auto_summarize: patch.auto_summarize.unwrap_or(current.auto_summarize),
            transcription_source: patch
                .transcription_source
                .unwrap_or(current.transcription_source),
            summary_source: patch.summary_source.unwrap_or(current.summary_source),
            cloud_provider: patch.cloud_provider.unwrap_or(current.cloud_provider),
            cloud_model: patch.cloud_model.clone().unwrap_or(current.cloud_model),
            summary_template: patch
                .summary_template
                .clone()
                .unwrap_or(current.summary_template),
            onboarding_completed: patch
                .onboarding_completed
                .unwrap_or(current.onboarding_completed),
            meeting_detection_enabled: patch
                .meeting_detection_enabled
                .unwrap_or(current.meeting_detection_enabled),
            launch_at_login: patch.launch_at_login.unwrap_or(current.launch_at_login),
            theme: patch.theme.unwrap_or(current.theme),
            microphone_id: patch.microphone_id.clone().unwrap_or(current.microphone_id),
            whisper_model_path: patch
                .whisper_model_path
                .clone()
                .unwrap_or(current.whisper_model_path),
            summary_model_path: patch
                .summary_model_path
                .clone()
                .unwrap_or(current.summary_model_path),
            summary_prompt: patch
                .summary_prompt
                .clone()
                .unwrap_or(current.summary_prompt),
        };
        let connection = self
            .connection
            .lock()
            .map_err(|_| AppError::Storage("database lock poisoned".into()))?;
        connection.execute(
            "UPDATE settings SET onboarding_completed = ?1, meeting_detection_enabled = ?2, launch_at_login = ?3, microphone_id = ?4, whisper_model_path = ?5, summary_model_path = ?6, summary_prompt = ?7, theme = ?8, summary_template = ?9, summary_source = ?10, cloud_provider = ?11, cloud_model = ?12, transcription_source = ?13, auto_transcribe = ?14, auto_summarize = ?15 WHERE singleton = 1",
            params![next.onboarding_completed, next.meeting_detection_enabled, next.launch_at_login, next.microphone_id, next.whisper_model_path, next.summary_model_path, next.summary_prompt, next.theme.as_str(), next.summary_template, next.summary_source.as_str(), next.cloud_provider.as_str(), next.cloud_model, next.transcription_source.as_str(), next.auto_transcribe, next.auto_summarize],
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

    pub fn set_recording_mode(&self, id: &str, mode: CaptureMode) -> AppResult<()> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| AppError::Storage("database lock poisoned".into()))?;
        connection.execute(
            "UPDATE recordings SET mode=?2, updated_at=?3 WHERE id=?1",
            params![id, mode_string(mode), Utc::now().to_rfc3339()],
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

    pub fn list_recordings(&self) -> AppResult<Vec<Recording>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| AppError::Storage("database lock poisoned".into()))?;
        let mut statement = connection.prepare(
            "SELECT id,title,mode,started_at,ended_at,duration_ms,playable_duration_ms,status,size_bytes,codec,detected_app,deleted_at FROM recordings ORDER BY started_at DESC"
        )?;
        let recordings = statement
            .query_map([], recording_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        recordings
            .into_iter()
            .map(|mut recording| {
                recording.highlights = highlights_for(&connection, &recording.id)?;
                recording.transcript = transcript_for(&connection, &recording.id)?;
                recording.summary = summary_for(&connection, &recording.id)?;
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
        recording.summary = summary_for(&connection, id)?;
        Ok(recording)
    }

    pub fn save_transcript(&self, recording_id: &str, transcript: &Transcript) -> AppResult<()> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| AppError::Storage("database lock poisoned".into()))?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO transcripts(recording_id, text, language, created_at, source, model) VALUES(?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(recording_id) DO UPDATE SET text=excluded.text, language=excluded.language, created_at=excluded.created_at, source=excluded.source, model=excluded.model",
            params![recording_id, transcript.text, transcript.language, transcript.created_at, transcript.source.map(|source| source.as_str()), transcript.model],
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

    pub fn save_summary(&self, recording_id: &str, summary: &Summary) -> AppResult<()> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| AppError::Storage("database lock poisoned".into()))?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO summaries(recording_id, suggested_title, overview, model, created_at, markdown, template_id) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(recording_id) DO UPDATE SET suggested_title=excluded.suggested_title, overview=excluded.overview, model=excluded.model, created_at=excluded.created_at, markdown=excluded.markdown, template_id=excluded.template_id",
            params![recording_id, summary.suggested_title, summary.overview, summary.model, summary.created_at, summary.markdown, summary.template_id],
        )?;
        transaction.execute(
            "DELETE FROM summary_points WHERE recording_id=?1",
            [recording_id],
        )?;
        for (kind, points) in [
            ("key_point", &summary.key_points),
            ("decision", &summary.decisions),
            ("action_item", &summary.action_items),
        ] {
            for (position, text) in points.iter().enumerate() {
                transaction.execute(
                    "INSERT INTO summary_points(recording_id, kind, position, text) VALUES(?1, ?2, ?3, ?4)",
                    params![recording_id, kind, position as i64, text],
                )?;
            }
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

    pub fn delete_recordings(&self, ids: &[String]) -> AppResult<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| AppError::Storage("database lock poisoned".into()))?;
        let transaction = connection.transaction()?;
        let mut paths = Vec::new();
        // Validate the entire selection before touching any files.
        for id in ids {
            let (asset, journal) = transaction
                .query_row(
                    "SELECT asset_path, journal_path FROM recordings WHERE id=?1",
                    [id],
                    |row| {
                        Ok((
                            row.get::<_, Option<String>>(0)?,
                            row.get::<_, Option<String>>(1)?,
                        ))
                    },
                )
                .optional()?
                .ok_or(AppError::NotFound)?;
            paths.extend([asset, journal].into_iter().flatten());
        }
        // Keep metadata (and file paths) if cleanup fails so deletion can be
        // retried. Missing files are fine, including after a partial attempt.
        for path in paths {
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(AppError::Storage(format!(
                        "could not delete recording file: {error}"
                    )));
                }
            }
        }
        for id in ids {
            // Foreign keys cascade to transcripts, summaries, highlights,
            // transcript segments, summary points, and delivery history.
            transaction.execute("DELETE FROM recordings WHERE id=?1", [id])?;
        }
        transaction.commit()?;
        Ok(())
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
        summary: None,
    })
}

fn transcript_for(connection: &Connection, recording_id: &str) -> AppResult<Option<Transcript>> {
    let transcript = connection
        .query_row(
            "SELECT text,language,created_at,source,model FROM transcripts WHERE recording_id=?1",
            [recording_id],
            |row| {
                Ok(Transcript {
                    source: match row.get::<_, Option<String>>(3)?.as_deref() {
                        Some("cloud") => {
                            Some(crate::cloud_transcription::TranscriptionSource::Cloud)
                        }
                        Some("local") => {
                            Some(crate::cloud_transcription::TranscriptionSource::Local)
                        }
                        _ => None,
                    },
                    model: row.get(4)?,
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

fn summary_for(connection: &Connection, recording_id: &str) -> AppResult<Option<Summary>> {
    let summary = connection
        .query_row(
            "SELECT suggested_title,overview,model,created_at,markdown,template_id FROM summaries WHERE recording_id=?1",
            [recording_id],
            |row| {
                Ok(Summary {
                    markdown: row.get(4)?,
                    template_id: row.get(5)?,
                    suggested_title: row.get(0)?,
                    overview: row.get(1)?,
                    key_points: Vec::new(),
                    decisions: Vec::new(),
                    action_items: Vec::new(),
                    model: row.get(2)?,
                    created_at: row.get(3)?,
                })
            },
        )
        .optional()?;
    let Some(mut summary) = summary else {
        return Ok(None);
    };
    let mut statement = connection.prepare(
        "SELECT kind,text FROM summary_points WHERE recording_id=?1 ORDER BY kind, position",
    )?;
    for row in statement.query_map([recording_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })? {
        let (kind, text) = row?;
        match kind.as_str() {
            "key_point" => summary.key_points.push(text),
            "decision" => summary.decisions.push(text),
            "action_item" => summary.action_items.push(text),
            _ => {}
        }
    }
    Ok(Some(summary))
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
            summary: None,
        }
    }

    #[test]
    fn automatic_settings_migrate_off_and_persist_independently() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("library.sqlite3");
        let repository = Repository::open(&path).unwrap();
        repository.connection.lock().unwrap().execute_batch(
            "ALTER TABLE settings DROP COLUMN auto_transcribe; ALTER TABLE settings DROP COLUMN auto_summarize;"
        ).unwrap();
        drop(repository);
        let repository = Repository::open(&path).unwrap();
        let settings = repository.settings().unwrap();
        assert!(!settings.auto_transcribe && !settings.auto_summarize);
        repository
            .update_settings(&SettingsPatch {
                auto_transcribe: Some(true),
                ..Default::default()
            })
            .unwrap();
        let settings = repository
            .update_settings(&SettingsPatch {
                auto_summarize: Some(true),
                ..Default::default()
            })
            .unwrap();
        assert!(settings.auto_transcribe && settings.auto_summarize);
        repository
            .update_settings(&SettingsPatch {
                auto_transcribe: Some(false),
                ..Default::default()
            })
            .unwrap();
        drop(repository);
        let settings = Repository::open(&path).unwrap().settings().unwrap();
        assert!(!settings.auto_transcribe && settings.auto_summarize);
    }

    #[test]
    fn migrations_and_settings_are_stable() {
        let temp = tempfile::tempdir().unwrap();
        let repository = Repository::open(&temp.path().join("library.sqlite3")).unwrap();
        assert!(repository.settings().unwrap().meeting_detection_enabled);
        assert_eq!(repository.settings().unwrap().theme, ThemePreference::Light);
        let updated = repository
            .update_settings(&SettingsPatch {
                onboarding_completed: Some(true),
                theme: Some(ThemePreference::Dark),
                whisper_model_path: Some(Some("model.bin".into())),
                summary_prompt: Some(Some("Focus on open risks.".into())),
                ..Default::default()
            })
            .unwrap();
        assert!(updated.onboarding_completed);
        assert_eq!(updated.theme, ThemePreference::Dark);
        assert_eq!(updated.whisper_model_path.as_deref(), Some("model.bin"));
        assert_eq!(
            updated.summary_prompt.as_deref(),
            Some("Focus on open risks.")
        );
        let reopened = Repository::open(&temp.path().join("library.sqlite3")).unwrap();
        assert_eq!(
            reopened.settings().unwrap().whisper_model_path.as_deref(),
            Some("model.bin")
        );
        assert_eq!(
            reopened.settings().unwrap().summary_prompt.as_deref(),
            Some("Focus on open risks.")
        );
        assert_eq!(reopened.settings().unwrap().theme, ThemePreference::Dark);
    }

    #[test]
    fn cloud_settings_migrate_as_local_and_round_trip_without_keys() {
        use crate::cloud_summarization::{CloudProvider, SummarySource};
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("library.sqlite3");
        {
            let repository = Repository::open(&path).unwrap();
            repository.connection.lock().unwrap().execute_batch("ALTER TABLE settings DROP COLUMN summary_source; ALTER TABLE settings DROP COLUMN cloud_provider; ALTER TABLE settings DROP COLUMN cloud_model;").unwrap();
        }
        let repository = Repository::open(&path).unwrap();
        let defaults = repository.settings().unwrap();
        assert_eq!(defaults.summary_source, SummarySource::Local);
        assert_eq!(defaults.cloud_provider, CloudProvider::Openai);
        assert_eq!(defaults.cloud_model, "gpt-5.4-mini");
        repository
            .update_settings(&SettingsPatch {
                summary_source: Some(SummarySource::Cloud),
                cloud_provider: Some(CloudProvider::Anthropic),
                cloud_model: Some("claude-test".into()),
                ..Default::default()
            })
            .unwrap();
        assert!(
            repository
                .update_settings(&SettingsPatch {
                    cloud_model: Some("bad model".into()),
                    ..Default::default()
                })
                .is_err()
        );
        let settings = Repository::open(&path).unwrap().settings().unwrap();
        assert_eq!(settings.summary_source, SummarySource::Cloud);
        assert_eq!(settings.cloud_provider, CloudProvider::Anthropic);
        assert_eq!(settings.cloud_model, "claude-test");
        assert!(!serde_json::to_string(&settings).unwrap().contains("apiKey"));
    }

    #[test]
    fn template_migration_preserves_custom_prompts_and_legacy_summaries() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("library.sqlite3");
        {
            let repository = Repository::open(&path).unwrap();
            repository
                .update_settings(&SettingsPatch {
                    summary_prompt: Some(Some("My existing prompt".into())),
                    ..Default::default()
                })
                .unwrap();
            repository
                .insert_recording(&test_recording("legacy"), "journal", &[0; 32])
                .unwrap();
            let connection = repository.connection.lock().unwrap();
            connection.execute("INSERT INTO summaries(recording_id,suggested_title,overview,model,created_at) VALUES('legacy','Old title','Old overview','local','now')", []).unwrap();
            connection.execute_batch("ALTER TABLE settings DROP COLUMN summary_template; ALTER TABLE summaries DROP COLUMN markdown; ALTER TABLE summaries DROP COLUMN template_id;").unwrap();
        }
        let repository = Repository::open(&path).unwrap();
        assert_eq!(
            repository.settings().unwrap().summary_template,
            "discussion"
        );
        assert_eq!(
            repository.settings().unwrap().summary_prompt.as_deref(),
            Some("My existing prompt")
        );
        let legacy = repository.recording("legacy").unwrap().summary.unwrap();
        assert_eq!(legacy.overview, "Old overview");
        assert_eq!(legacy.markdown, None);
        assert_eq!(legacy.template_id, None);
        repository
            .update_settings(&SettingsPatch {
                summary_template: Some("actions".into()),
                ..Default::default()
            })
            .unwrap();
        assert!(
            repository
                .update_settings(&SettingsPatch {
                    summary_template: Some("missing".into()),
                    ..Default::default()
                })
                .is_err()
        );
        let markdown = Summary {
            markdown: Some("# Pilot\n\n## Konklusion\nA proposal.\n\n- [ ] Follow up".into()),
            template_id: Some("discussion".into()),
            ..legacy.clone()
        };
        repository.save_summary("legacy", &markdown).unwrap();
        let reopened = Repository::open(&path).unwrap();
        assert_eq!(reopened.settings().unwrap().summary_template, "actions");
        assert_eq!(
            reopened.recording("legacy").unwrap().summary,
            Some(markdown)
        );
        // Switching back to a structured template must clear the old Markdown.
        reopened.save_summary("legacy", &legacy).unwrap();
        assert_eq!(reopened.recording("legacy").unwrap().summary, Some(legacy));
    }

    #[test]
    fn legacy_trash_returns_to_the_library_on_upgrade() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("library.sqlite3");
        let repository = Repository::open(&path).unwrap();
        repository
            .insert_recording(&test_recording("legacy"), "journal", &[0; 32])
            .unwrap();
        repository
            .connection
            .lock()
            .unwrap()
            .execute(
                "UPDATE recordings SET deleted_at=?1 WHERE id='legacy'",
                [Utc::now().to_rfc3339()],
            )
            .unwrap();
        drop(repository);
        let reopened = Repository::open(&path).unwrap();
        assert_eq!(reopened.list_recordings().unwrap().len(), 1);
        assert!(reopened.recording("legacy").unwrap().deleted_at.is_none());
    }

    fn insert_deletable_recording(
        repository: &Repository,
        root: &Path,
        id: &str,
    ) -> (std::path::PathBuf, std::path::PathBuf) {
        let asset = root.join(format!("{id}.eav"));
        let journal = root.join(format!("{id}.journal"));
        std::fs::write(&asset, b"encrypted audio").unwrap();
        std::fs::write(&journal, b"recovery fragments").unwrap();
        let mut recording = test_recording(id);
        recording.status = "ready".into();
        repository
            .insert_recording(&recording, journal.to_str().unwrap(), &[0; 32])
            .unwrap();
        repository
            .connection
            .lock()
            .unwrap()
            .execute(
                "UPDATE recordings SET asset_path=?2 WHERE id=?1",
                params![id, asset.to_str().unwrap()],
            )
            .unwrap();
        (asset, journal)
    }

    #[test]
    fn permanent_delete_removes_files_and_all_related_data() {
        let temp = tempfile::tempdir().unwrap();
        let repository = Repository::open(&temp.path().join("library.sqlite3")).unwrap();
        let (asset, journal) = insert_deletable_recording(&repository, temp.path(), "one");
        let (other_asset, _) = insert_deletable_recording(&repository, temp.path(), "keep");
        repository.add_highlight("one", 500).unwrap();
        repository
            .save_transcript(
                "one",
                &Transcript {
                    source: None,
                    model: None,
                    text: "Transcript".into(),
                    language: None,
                    created_at: Utc::now().to_rfc3339(),
                    segments: vec![TranscriptSegment {
                        start_ms: 0,
                        end_ms: 1000,
                        text: "Transcript".into(),
                    }],
                },
            )
            .unwrap();
        repository
            .save_summary(
                "one",
                &Summary {
                    suggested_title: "Summary".into(),
                    overview: "Overview".into(),
                    key_points: vec!["Point".into()],
                    decisions: vec![],
                    action_items: vec![],
                    model: "local".into(),
                    created_at: Utc::now().to_rfc3339(),
                    markdown: None,
                    template_id: None,
                },
            )
            .unwrap();
        repository.connection.lock().unwrap().execute(
            "INSERT INTO delivery_attempts(id,recording_id,destination_id,status,created_at,updated_at) VALUES('delivery','one','oakos','succeeded','now','now')", [],
        ).unwrap();
        repository.delete_recordings(&["one".into()]).unwrap();
        assert!(!asset.exists());
        assert!(!journal.exists());
        assert!(repository.recording("one").is_err());
        assert_eq!(repository.list_recordings().unwrap().len(), 1);
        assert!(other_asset.exists());
        let connection = repository.connection.lock().unwrap();
        for table in [
            "highlights",
            "transcripts",
            "transcript_segments",
            "summaries",
            "summary_points",
            "delivery_attempts",
        ] {
            let count: i64 = connection
                .query_row(
                    &format!("SELECT COUNT(*) FROM {table} WHERE recording_id='one'"),
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 0, "{table} still contains deleted data");
        }
    }

    #[test]
    fn bulk_delete_validates_all_ids_before_removing_files() {
        let temp = tempfile::tempdir().unwrap();
        let repository = Repository::open(&temp.path().join("library.sqlite3")).unwrap();
        let (asset, _) = insert_deletable_recording(&repository, temp.path(), "one");
        let (second_asset, _) = insert_deletable_recording(&repository, temp.path(), "two");
        assert!(matches!(
            repository.delete_recordings(&["one".into(), "missing".into()]),
            Err(AppError::NotFound)
        ));
        assert!(asset.exists());
        assert_eq!(repository.list_recordings().unwrap().len(), 2);
        repository
            .delete_recordings(&["one".into(), "two".into()])
            .unwrap();
        assert!(!asset.exists());
        assert!(!second_asset.exists());
        assert!(repository.list_recordings().unwrap().is_empty());
    }

    #[test]
    fn interrupted_recordings_without_an_audio_asset_can_be_deleted() {
        let temp = tempfile::tempdir().unwrap();
        let repository = Repository::open(&temp.path().join("library.sqlite3")).unwrap();
        let journal = temp.path().join("interrupted.journal");
        std::fs::write(&journal, b"recovery fragments").unwrap();
        repository
            .insert_recording(
                &test_recording("interrupted"),
                journal.to_str().unwrap(),
                &[0; 32],
            )
            .unwrap();
        repository
            .delete_recordings(&["interrupted".into()])
            .unwrap();
        assert!(!journal.exists());
        assert!(repository.recording("interrupted").is_err());
    }

    #[test]
    fn failed_file_cleanup_preserves_metadata_for_retry() {
        let temp = tempfile::tempdir().unwrap();
        let repository = Repository::open(&temp.path().join("library.sqlite3")).unwrap();
        let (asset, journal) = insert_deletable_recording(&repository, temp.path(), "one");
        std::fs::remove_file(&journal).unwrap();
        std::fs::create_dir(&journal).unwrap();
        assert!(repository.delete_recordings(&["one".into()]).is_err());
        assert!(repository.recording("one").is_ok());
        assert!(!asset.exists());
        std::fs::remove_dir(&journal).unwrap();
        repository.delete_recordings(&["one".into()]).unwrap();
        assert!(repository.recording("one").is_err());
    }

    #[test]
    fn summaries_round_trip_with_their_points_in_order() {
        let temp = tempfile::tempdir().unwrap();
        let repository = Repository::open(&temp.path().join("library.sqlite3")).unwrap();
        repository
            .insert_recording(&test_recording("one"), "journal", &[0; 32])
            .unwrap();
        let summary = Summary {
            markdown: None,
            template_id: None,
            suggested_title: "Nordic pricing tiers".into(),
            overview: "The team kept three tiers and set a March launch.".into(),
            key_points: vec![
                "Tier three sits too close to tier two".into(),
                "Support load is the risk".into(),
            ],
            decisions: vec!["Launch in March".into()],
            action_items: vec!["Mette rewrites the pricing page".into()],
            model: "qwen2.5-1.5b-instruct-q4_k_m.gguf".into(),
            created_at: Utc::now().to_rfc3339(),
        };

        repository.save_summary("one", &summary).unwrap();

        assert_eq!(
            repository.recording("one").unwrap().summary,
            Some(summary.clone())
        );
        // Re-summarizing replaces the previous points rather than appending.
        let shorter = Summary {
            key_points: vec!["Only one point now".into()],
            ..summary
        };
        repository.save_summary("one", &shorter).unwrap();
        assert_eq!(repository.recording("one").unwrap().summary, Some(shorter));
    }

    #[test]
    fn cloud_transcription_migrates_as_local_and_preserves_text_only_provenance() {
        use crate::cloud_transcription::TranscriptionSource;
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("library.sqlite3");
        let repository = Repository::open(&path).unwrap();
        repository
            .insert_recording(&test_recording("one"), "journal", &[0; 32])
            .unwrap();
        repository
            .save_transcript(
                "one",
                &Transcript {
                    text: "Legacy transcript".into(),
                    language: None,
                    created_at: "now".into(),
                    segments: vec![],
                    source: None,
                    model: None,
                },
            )
            .unwrap();
        repository.connection.lock().unwrap().execute_batch(
            "ALTER TABLE settings DROP COLUMN transcription_source; ALTER TABLE transcripts DROP COLUMN source; ALTER TABLE transcripts DROP COLUMN model;"
        ).unwrap();
        drop(repository);
        let repository = Repository::open(&path).unwrap();
        assert_eq!(
            repository.settings().unwrap().transcription_source,
            TranscriptionSource::Local
        );
        assert_eq!(
            repository
                .recording("one")
                .unwrap()
                .transcript
                .unwrap()
                .text,
            "Legacy transcript"
        );
        let cloud = Transcript {
            text: "Cloud transcript".into(),
            language: Some("da".into()),
            created_at: "now".into(),
            segments: vec![],
            source: Some(TranscriptionSource::Cloud),
            model: Some("gpt-transcribe".into()),
        };
        repository.save_transcript("one", &cloud).unwrap();
        repository
            .update_settings(&SettingsPatch {
                transcription_source: Some(TranscriptionSource::Cloud),
                ..Default::default()
            })
            .unwrap();
        drop(repository);
        let repository = Repository::open(&path).unwrap();
        assert_eq!(repository.recording("one").unwrap().transcript, Some(cloud));
        assert_eq!(
            repository.settings().unwrap().transcription_source,
            TranscriptionSource::Cloud
        );
        assert_eq!(
            repository.settings().unwrap().summary_source,
            crate::cloud_summarization::SummarySource::Local
        );
    }

    #[test]
    fn timestamped_transcripts_round_trip() {
        let temp = tempfile::tempdir().unwrap();
        let repository = Repository::open(&temp.path().join("library.sqlite3")).unwrap();
        repository
            .insert_recording(&test_recording("one"), "journal", &[0; 32])
            .unwrap();
        let transcript = Transcript {
            source: None,
            model: None,
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
