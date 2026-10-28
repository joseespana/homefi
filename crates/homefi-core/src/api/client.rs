use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use futures::future::BoxFuture;
use serde::de::DeserializeOwned;
use serde_json::Value;

use super::cache::DiskCache;
use crate::error::{Error, Result};

pub const API_URL: &str = "https://api.tidal.com/v1";
pub const API_V2_URL: &str = "https://api.tidal.com/v2";

const MAX_RETRIES: u32 = 5;
const RETRY_DELAY: Duration = Duration::from_millis(500);

/// How long a response may be served from the disk cache.
#[derive(Debug, Clone, Copy)]
pub enum CachePolicy {
    NoStore,
    Ttl(Duration),
}

/// Called when the API answers `401`; returns the new access token.
pub type TokenRefresher = Arc<dyn Fn() -> BoxFuture<'static, Result<String>> + Send + Sync>;

/// Low-level HTTP client: auth header, caching, retries and token refresh.
pub struct TidalClient {
    http: reqwest::Client,
    api_url: String,
    api_v2_url: String,
    token: RwLock<String>,
    cache: Option<DiskCache>,
    omit_cache: bool,
    debug_dir: Option<PathBuf>,
    refresher: Option<TokenRefresher>,
    refresh_lock: tokio::sync::Mutex<()>,
}

impl TidalClient {
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::builder()
                .user_agent(concat!("homefi/", env!("CARGO_PKG_VERSION")))
                // no total timeout (downloads take long), but a stalled
                // connection is given up instead of hanging forever
                .connect_timeout(std::time::Duration::from_secs(15))
                .read_timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("reqwest client"),
            api_url: API_URL.to_owned(),
            api_v2_url: API_V2_URL.to_owned(),
            token: RwLock::new(token.into()),
            cache: None,
            omit_cache: false,
            debug_dir: None,
            refresher: None,
            refresh_lock: tokio::sync::Mutex::new(()),
        }
    }

    /// Points the client to another server (tests, proxies).
    pub fn with_base_urls(mut self, v1: impl Into<String>, v2: impl Into<String>) -> Self {
        self.api_url = v1.into();
        self.api_v2_url = v2.into();
        self
    }

    /// Enables the on-disk response cache. With `omit_reads`, responses are
    /// still stored but never served from the cache.
    pub fn with_cache(mut self, cache: DiskCache, omit_reads: bool) -> Self {
        self.cache = Some(cache);
        self.omit_cache = omit_reads;
        self
    }

    /// Dumps every v1 response to `<dir>/<endpoint>.json`.
    pub fn with_debug_dir(mut self, dir: Option<PathBuf>) -> Self {
        self.debug_dir = dir;
        self
    }

    pub fn with_refresher(mut self, refresher: TokenRefresher) -> Self {
        self.refresher = Some(refresher);
        self
    }

    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }

    pub fn token(&self) -> String {
        self.token.read().expect("token lock").clone()
    }

    pub fn set_token(&self, token: String) {
        *self.token.write().expect("token lock") = token;
    }

    fn cache_key(url: &str, params: &[(&str, String)]) -> String {
        let mut sorted: Vec<_> = params.iter().collect();
        sorted.sort();
        let query: Vec<String> = sorted.iter().map(|(k, v)| format!("{k}={v}")).collect();
        format!("{url}?{}", query.join("&"))
    }

    /// Refreshes the token once even if many requests hit `401` together.
    async fn refresh_token(&self, stale: &str) -> Result<bool> {
        let Some(refresher) = &self.refresher else {
            return Ok(false);
        };
        let _guard = self.refresh_lock.lock().await;
        if self.token() != stale {
            return Ok(true); // someone else already refreshed it
        }
        let token = refresher().await?;
        self.set_token(token);
        Ok(true)
    }

    async fn get(&self, url: &str, params: &[(&str, String)]) -> Result<(u16, String)> {
        let mut refreshed = false;
        loop {
            let token = self.token();
            let res = self
                .http
                .get(url)
                .query(params)
                .bearer_auth(&token)
                .header("Accept", "application/json")
                .send()
                .await?;

            let status = res.status().as_u16();
            if status == 401 && !refreshed && self.refresh_token(&token).await? {
                refreshed = true;
                continue;
            }
            return Ok((status, res.text().await?));
        }
    }

    /// GETs a v1 endpoint and decodes it into `T`.
    pub async fn fetch<T: DeserializeOwned>(
        &self,
        endpoint: &str,
        params: &[(&str, String)],
        policy: CachePolicy,
    ) -> Result<T> {
        let url = format!("{}/{endpoint}", self.api_url);
        let key = Self::cache_key(&url, params);
        let decode = |data: Value| {
            serde_json::from_value::<T>(data)
                .map_err(|e| Error::Decode { endpoint: endpoint.to_owned(), message: e.to_string() })
        };

        if let (CachePolicy::Ttl(ttl), Some(cache), false) = (policy, &self.cache, self.omit_cache) {
            if let Some(body) = cache.get(&key, ttl) {
                if let Ok(data) = serde_json::from_str::<Value>(&body) {
                    tracing::debug!("{endpoint} HIT");
                    return decode(data);
                }
            }
        }

        let mut attempt = 1;
        let (status, body, data) = loop {
            let (status, body) = self.get(&url, params).await?;
            match serde_json::from_str::<Value>(&body) {
                Ok(data) => break (status, body, data),
                Err(_) if attempt < MAX_RETRIES => {
                    tracing::warn!("invalid json from {endpoint}, retrying {attempt}/{MAX_RETRIES}");
                    attempt += 1;
                    tokio::time::sleep(RETRY_DELAY).await;
                }
                Err(_) => {
                    return Err(Error::Api {
                        status,
                        sub_status: "0".into(),
                        user_message: "Response body does not contain valid json.".into(),
                    })
                }
            }
        };

        tracing::debug!("{endpoint} MISS [{status}]");
        self.dump_debug(endpoint, params, status, &data);

        if status != 200 {
            return Err(api_error(status, &data));
        }

        if let (CachePolicy::Ttl(_), Some(cache)) = (policy, &self.cache) {
            cache.put(&key, &body);
        }

        decode(data)
    }

    /// Sends a write request (`POST`, `PUT`, `DELETE`) with a form body.
    /// Returns the JSON answer (`Null` when empty). `etag` goes in
    /// `If-None-Match`, which Tidal requires to edit a playlist.
    pub async fn send(
        &self,
        method: reqwest::Method,
        v2: bool,
        endpoint: &str,
        query: &[(&str, String)],
        form: &[(&str, String)],
        etag: Option<&str>,
    ) -> Result<Value> {
        let url = format!("{}/{endpoint}", if v2 { &self.api_v2_url } else { &self.api_url });
        let mut refreshed = false;
        loop {
            let token = self.token();
            let mut req = self
                .http
                .request(method.clone(), &url)
                .query(query)
                .bearer_auth(&token)
                .header("Accept", "application/json");
            if !form.is_empty() {
                req = req.form(form);
            }
            if let Some(tag) = etag {
                req = req.header("If-None-Match", tag);
            }
            let res = req.send().await?;
            let status = res.status().as_u16();
            if status == 401 && !refreshed && self.refresh_token(&token).await? {
                refreshed = true;
                continue;
            }
            let body = res.text().await?;
            let data =
                if body.trim().is_empty() { Value::Null } else { serde_json::from_str(&body).unwrap_or(Value::Null) };
            tracing::debug!("{method} {endpoint} [{status}]");
            if !(200..300).contains(&status) {
                return Err(api_error(status, &data));
            }
            return Ok(data);
        }
    }

    /// `ETag` of a v1 resource (needed before editing a playlist).
    pub async fn etag(&self, endpoint: &str, params: &[(&str, String)]) -> Result<Option<String>> {
        let url = format!("{}/{endpoint}", self.api_url);
        let res = self.http.get(&url).query(params).bearer_auth(self.token()).send().await?;
        let status = res.status().as_u16();
        let tag = res.headers().get("etag").and_then(|v| v.to_str().ok()).map(str::to_owned);
        if status != 200 {
            let data = res.json::<Value>().await.unwrap_or(Value::Null);
            return Err(api_error(status, &data));
        }
        Ok(tag)
    }

    /// GETs a v2 (JSON:API) endpoint. Non-200 answers yield an empty object.
    pub async fn fetch_v2(&self, endpoint: &str, params: &[(&str, String)], policy: CachePolicy) -> Result<Value> {
        let url = format!("{}/{endpoint}", self.api_v2_url);
        let key = Self::cache_key(&url, params);

        if let (CachePolicy::Ttl(ttl), Some(cache), false) = (policy, &self.cache, self.omit_cache) {
            if let Some(data) = cache.get(&key, ttl).and_then(|b| serde_json::from_str(&b).ok()) {
                return Ok(data);
            }
        }

        let (status, body) = self.get(&url, params).await?;
        let data: Value = match serde_json::from_str(&body) {
            Ok(v @ Value::Object(_)) if status == 200 => v,
            _ => return Ok(Value::Object(Default::default())),
        };

        if let (CachePolicy::Ttl(_), Some(cache)) = (policy, &self.cache) {
            cache.put(&key, &body);
        }
        Ok(data)
    }

    fn dump_debug(&self, endpoint: &str, params: &[(&str, String)], status: u16, data: &Value) {
        let Some(dir) = &self.debug_dir else { return };
        let file = dir.join(format!("{endpoint}.json"));
        let params: serde_json::Map<String, Value> =
            params.iter().map(|(k, v)| (k.to_string(), Value::String(v.clone()))).collect();
        let dump = serde_json::json!({
            "status_code": status,
            "endpoint": endpoint,
            "params": params,
            "data": data,
        });
        let result = file
            .parent()
            .map(std::fs::create_dir_all)
            .transpose()
            .and_then(|_| std::fs::write(&file, serde_json::to_vec_pretty(&dump).unwrap_or_default()));
        if let Err(e) = result {
            tracing::debug!("could not write {}: {e}", file.display());
        }
    }
}

fn api_error(status: u16, data: &Value) -> Error {
    let sub_status = match data.get("subStatus") {
        Some(Value::String(s)) => s.clone(),
        Some(v) => v.to_string(),
        None => "0".into(),
    };
    let user_message = data
        .get("userMessage")
        .or_else(|| data.get("message"))
        .and_then(Value::as_str)
        .unwrap_or("Unknown API error")
        .to_owned();
    let status = data.get("status").and_then(Value::as_u64).map(|s| s as u16).unwrap_or(status);
    Error::Api { status, sub_status, user_message }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_errors_are_parsed() {
        let err = api_error(404, &serde_json::json!({"status": 404, "subStatus": 2001, "userMessage": "Not found"}));
        assert_eq!(err.to_string(), "Not found, 404/2001");

        let err = api_error(500, &serde_json::json!({"message": "boom"}));
        assert_eq!(err.to_string(), "boom, 500/0");
    }

    #[test]
    fn cache_key_ignores_param_order() {
        let a = TidalClient::cache_key("u", &[("b", "2".into()), ("a", "1".into())]);
        let b = TidalClient::cache_key("u", &[("a", "1".into()), ("b", "2".into())]);
        assert_eq!(a, b);
    }
}
