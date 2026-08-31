use reqwest::{StatusCode, Url, blocking::Client, header};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

const API_BASE_URL: &str = "https://os.oak.dk/api/v1/";
const KEYRING_SERVICE: &str = "com.eavesdrop.recorder";
const KEYRING_ACCOUNT: &str = "oakos-personal-access-token";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OakOsIntegration {
    pub connected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OakOsProject {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OakOsPublishResult {
    pub location: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Collection<T> {
    data: Vec<T>,
    pagination: Option<Pagination>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Pagination {
    next_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ErrorEnvelope {
    error: ApiError,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    message: String,
}

pub fn integration() -> AppResult<OakOsIntegration> {
    match token_entry()?.get_password() {
        Ok(_) => Ok(OakOsIntegration { connected: true }),
        Err(keyring::Error::NoEntry) => Ok(OakOsIntegration { connected: false }),
        Err(error) => Err(AppError::Crypto(error.to_string())),
    }
}

pub fn connect(token: &str) -> AppResult<OakOsIntegration> {
    let normalized = token.trim();
    if normalized.is_empty() {
        return Err(AppError::Other(
            "enter an OakOS personal access token".into(),
        ));
    }
    let client = OakOsClient::new(normalized)?;
    client.list_projects()?;
    token_entry()?
        .set_password(normalized)
        .map_err(|error| AppError::Crypto(error.to_string()))?;
    Ok(OakOsIntegration { connected: true })
}

pub fn disconnect() -> AppResult<()> {
    match token_entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(AppError::Crypto(error.to_string())),
    }
}

pub fn list_projects() -> AppResult<Vec<OakOsProject>> {
    client_from_keyring()?.list_projects()
}

pub fn publish_recording(
    project_id: &str,
    recording_id: &str,
    title: &str,
    audio: Vec<u8>,
) -> AppResult<OakOsPublishResult> {
    client_from_keyring()?.publish_recording(project_id, recording_id, title, audio)
}

fn token_entry() -> AppResult<keyring::Entry> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
        .map_err(|error| AppError::Crypto(error.to_string()))
}

fn client_from_keyring() -> AppResult<OakOsClient> {
    let token = token_entry()?.get_password().map_err(|error| match error {
        keyring::Error::NoEntry => AppError::Other(
            "OakOS is not connected. Add a personal access token in Integrations.".into(),
        ),
        other => AppError::Crypto(other.to_string()),
    })?;
    OakOsClient::new(&token)
}

struct OakOsClient {
    http: Client,
    base_url: Url,
    token: String,
}

impl OakOsClient {
    fn new(token: &str) -> AppResult<Self> {
        let http = Client::builder()
            .user_agent(concat!("Eavesdrop/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(network_error)?;
        let base_url = Url::parse(API_BASE_URL)
            .map_err(|error| AppError::Other(format!("invalid OakOS API URL: {error}")))?;
        Ok(Self {
            http,
            base_url,
            token: token.into(),
        })
    }

    fn list_projects(&self) -> AppResult<Vec<OakOsProject>> {
        let mut projects = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let mut url = self.endpoint(&["projects"])?;
            {
                let mut query = url.query_pairs_mut();
                query.append_pair("limit", "200");
                if let Some(value) = cursor.as_deref() {
                    query.append_pair("cursor", value);
                }
            }
            let request = self.http.get(url).bearer_auth(&self.token);
            let response = request.send().map_err(network_error)?;
            let response = checked(response)?;
            let page: Collection<OakOsProject> = response.json().map_err(response_error)?;
            projects.extend(page.data);
            cursor = page
                .pagination
                .and_then(|pagination| pagination.next_cursor);
            if cursor.is_none() {
                break;
            }
        }
        projects.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
        Ok(projects)
    }

    fn publish_recording(
        &self,
        project_id: &str,
        recording_id: &str,
        title: &str,
        audio: Vec<u8>,
    ) -> AppResult<OakOsPublishResult> {
        if project_id.trim().is_empty() {
            return Err(AppError::Other(
                "choose an OakOS project before publishing".into(),
            ));
        }
        let url = self.endpoint(&["projects", project_id, "recordings"])?;
        let filename = safe_filename(title);
        let response = self
            .http
            .post(url)
            .bearer_auth(&self.token)
            .header(header::CONTENT_TYPE, "audio/mp4")
            .header(
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}.m4a\""),
            )
            .header(
                "Idempotency-Key",
                format!("eavesdrop-{recording_id}-{project_id}"),
            )
            .body(audio)
            .send()
            .map_err(network_error)?;
        let response = checked(response)?;
        if response.status() != StatusCode::ACCEPTED {
            return Err(AppError::Other(format!(
                "OakOS returned an unexpected {} response",
                response.status()
            )));
        }
        Ok(OakOsPublishResult {
            location: response
                .headers()
                .get(header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned),
        })
    }

    fn endpoint(&self, segments: &[&str]) -> AppResult<Url> {
        let mut url = self.base_url.clone();
        url.path_segments_mut()
            .map_err(|_| AppError::Other("invalid OakOS API URL".into()))?
            .pop_if_empty()
            .extend(segments);
        Ok(url)
    }
}

fn checked(response: reqwest::blocking::Response) -> AppResult<reqwest::blocking::Response> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status();
    let body = response.text().unwrap_or_default();
    let message = serde_json::from_str::<ErrorEnvelope>(&body)
        .map(|envelope| envelope.error.message)
        .unwrap_or_else(|_| {
            status
                .canonical_reason()
                .unwrap_or("request failed")
                .to_string()
        });
    let hint = match status {
        StatusCode::UNAUTHORIZED => " Check the personal access token in Integrations.",
        StatusCode::FORBIDDEN => " The token does not have permission for this operation.",
        StatusCode::TOO_MANY_REQUESTS => " Try again in a moment.",
        _ => "",
    };
    Err(AppError::Other(format!("OakOS: {message}.{hint}")))
}

fn network_error(error: reqwest::Error) -> AppError {
    AppError::Other(format!("could not reach OakOS: {error}"))
}

fn response_error(error: reqwest::Error) -> AppError {
    AppError::Other(format!("OakOS returned an unreadable response: {error}"))
}

fn safe_filename(title: &str) -> String {
    let normalized: String = title
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | ' ') {
                character
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        "recording".into()
    } else {
        trimmed.chars().take(120).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filenames_are_safe_for_content_disposition() {
        assert_eq!(
            safe_filename("Design / status \"review\""),
            "Design _ status _review_"
        );
        assert_eq!(safe_filename("***"), "___");
        assert_eq!(safe_filename("   "), "recording");
    }

    #[test]
    fn endpoint_encodes_project_ids() {
        let client = OakOsClient::new("token").unwrap();
        let url = client
            .endpoint(&["projects", "client work/2026", "recordings"])
            .unwrap();
        assert_eq!(
            url.as_str(),
            "https://os.oak.dk/api/v1/projects/client%20work%2F2026/recordings"
        );
    }
}
