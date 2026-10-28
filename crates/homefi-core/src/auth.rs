//! Tidal device-code authentication and persisted credentials.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

pub const AUTH_URL: &str = "https://auth.tidal.com/v1/oauth2";
const LOGOUT_URL: &str = "https://api.tidal.com/v1/logout";
const SCOPE: &str = "r_usr+w_usr+w_sub";

/// Override for the client credentials, formatted as `<CLIENT_ID>;<CLIENT_SECRET>`.
pub const AUTH_ENV: &str = "HOMEFI_AUTH";
/// Legacy variable kept for users migrating from tiddl.
const LEGACY_AUTH_ENV: &str = "TIDDL_AUTH";

const DEFAULT_CREDENTIALS: &str =
    "NE4zbjZRMXg5NUxMNUs3cDtvS09YZkpXMzcxY1g2eGFaMFB5aGdHTkJkTkxsQlpkNEFLS1lvdWdNamlrPQ==";

pub fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Client id / secret used for the OAuth flow.
pub fn credentials() -> (String, String) {
    let from_env = [AUTH_ENV, LEGACY_AUTH_ENV].iter().filter_map(|k| std::env::var(k).ok()).find(|v| v.contains(';'));

    let raw = from_env.unwrap_or_else(|| {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(DEFAULT_CREDENTIALS)
            .expect("embedded credentials are valid base64");
        String::from_utf8(bytes).expect("embedded credentials are utf-8")
    });

    let (id, secret) = raw.split_once(';').unwrap_or((raw.as_str(), ""));
    (id.to_owned(), secret.to_owned())
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceAuth {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: String,
    pub expires_in: i64,
    pub interval: i64,
}

impl DeviceAuth {
    /// Browser URL the user opens to approve this device.
    pub fn url(&self) -> String {
        if self.verification_uri_complete.starts_with("http") {
            self.verification_uri_complete.clone()
        } else {
            format!("https://{}", self.verification_uri_complete)
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthUser {
    pub user_id: i64,
    #[serde(default)]
    pub email: Option<String>,
    pub country_code: String,
    #[serde(default)]
    pub username: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthResponse {
    pub user: AuthUser,
    pub access_token: String,
    pub expires_in: i64,
    pub user_id: i64,
    #[serde(default)]
    pub refresh_token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthErrorBody {
    #[serde(default)]
    status: Option<u16>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default, alias = "sub_status")]
    sub_status: Option<serde_json::Value>,
    #[serde(default, alias = "error_description")]
    error_description: Option<String>,
}

/// Thin client around Tidal's OAuth endpoints.
#[derive(Clone)]
pub struct AuthClient {
    http: reqwest::Client,
    client_id: String,
    client_secret: String,
}

impl Default for AuthClient {
    fn default() -> Self {
        let (client_id, client_secret) = credentials();
        // Bounded: a phone may freeze the app mid-request (e.g. while the user
        // signs in from the browser) and the login poll must not hang on it.
        let http = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .unwrap_or_default();
        Self { http, client_id, client_secret }
    }
}

impl AuthClient {
    pub async fn device_auth(&self) -> Result<DeviceAuth> {
        let res = self
            .http
            .post(format!("{AUTH_URL}/device_authorization"))
            .form(&[("client_id", self.client_id.as_str()), ("scope", SCOPE)])
            .send()
            .await?
            .error_for_status()?;
        Ok(res.json().await?)
    }

    /// Polls once for the token. Returns [`Error::Auth`] with
    /// `authorization_pending` while the user has not approved yet.
    pub async fn token(&self, device_code: &str) -> Result<AuthResponse> {
        let res = self
            .http
            .post(format!("{AUTH_URL}/token"))
            .basic_auth(&self.client_id, Some(&self.client_secret))
            .form(&[
                ("client_id", self.client_id.as_str()),
                ("device_code", device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("scope", SCOPE),
            ])
            .send()
            .await?;

        let status = res.status();
        let body = res.text().await?;

        if status.is_success() {
            return Ok(serde_json::from_str(&body)?);
        }

        let err: AuthErrorBody = serde_json::from_str(&body).unwrap_or(AuthErrorBody {
            status: Some(status.as_u16()),
            error: Some(body),
            sub_status: None,
            error_description: None,
        });

        Err(Error::Auth {
            status: err.status.unwrap_or(status.as_u16()),
            error: err.error.unwrap_or_default(),
            sub_status: err.sub_status.map(|v| v.to_string()).unwrap_or_default(),
            description: err.error_description.unwrap_or_default(),
        })
    }

    pub async fn refresh(&self, refresh_token: &str) -> Result<AuthResponse> {
        let res = self
            .http
            .post(format!("{AUTH_URL}/token"))
            .basic_auth(&self.client_id, Some(&self.client_secret))
            .form(&[
                ("client_id", self.client_id.as_str()),
                ("refresh_token", refresh_token),
                ("grant_type", "refresh_token"),
                ("scope", SCOPE),
            ])
            .send()
            .await?
            .error_for_status()?;
        Ok(res.json().await?)
    }

    pub async fn logout(&self, access_token: &str) -> Result<()> {
        self.http.post(LOGOUT_URL).bearer_auth(access_token).send().await?.error_for_status()?;
        Ok(())
    }
}

/// Credentials persisted in `auth.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AuthData {
    pub token: Option<String>,
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_at: i64,
    pub user_id: Option<String>,
    pub country_code: Option<String>,
}

impl AuthData {
    pub fn from_response(res: &AuthResponse) -> Self {
        Self {
            token: Some(res.access_token.clone()),
            refresh_token: res.refresh_token.clone(),
            expires_at: now_secs() + res.expires_in,
            user_id: Some(res.user_id.to_string()),
            country_code: Some(res.user.country_code.clone()),
        }
    }

    pub fn is_logged_in(&self) -> bool {
        self.token.is_some() && self.refresh_token.is_some()
    }

    /// `true` when the token expires within `margin` seconds.
    pub fn expires_within(&self, margin: i64) -> bool {
        now_secs() >= self.expires_at - margin
    }
}

/// Reads and atomically writes [`AuthData`] (mode `0600` on Unix).
#[derive(Debug, Clone)]
pub struct AuthStore {
    path: PathBuf,
}

impl AuthStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<AuthData> {
        match std::fs::read_to_string(&self.path) {
            Ok(content) => Ok(serde_json::from_str(&content)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(AuthData::default()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn save(&self, data: &AuthData) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
            set_mode(parent, 0o700);
        }
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec(data)?)?;
        set_mode(&tmp, 0o600);
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }

    pub fn clear(&self) -> Result<()> {
        self.save(&AuthData::default())
    }
}

#[cfg(unix)]
pub(crate) fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    if let Err(e) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)) {
        tracing::warn!("chmod {mode:o} on {} failed: {e}", path.display());
    }
}

#[cfg(not(unix))]
pub(crate) fn set_mode(_path: &Path, _mode: u32) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_credentials_decode() {
        let (id, secret) = credentials();
        assert!(!id.is_empty());
        assert!(!secret.is_empty());
    }

    #[test]
    fn store_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = AuthStore::new(dir.path().join("sub/auth.json"));
        assert_eq!(store.load().unwrap(), AuthData::default());

        let data = AuthData {
            token: Some("t".into()),
            refresh_token: Some("r".into()),
            expires_at: 42,
            user_id: Some("1".into()),
            country_code: Some("US".into()),
        };
        store.save(&data).unwrap();
        assert_eq!(store.load().unwrap(), data);
    }

    #[test]
    fn reads_tiddl_auth_file() {
        let json = r#"{"token":"a","refresh_token":"b","expires_at":1,"user_id":"2","country_code":"PL"}"#;
        let data: AuthData = serde_json::from_str(json).unwrap();
        assert!(data.is_logged_in());
    }
}
