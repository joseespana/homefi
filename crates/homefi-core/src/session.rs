//! Ties together the app directory, config, credentials and API client.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures::FutureExt;

use crate::api::{DiskCache, TidalApi, TidalClient};
use crate::auth::{AuthClient, AuthData, AuthStore, DeviceAuth};
use crate::config::Config;
use crate::error::{Error, Result};
use crate::paths;

/// Result of one poll of the device login.
#[derive(Debug, Clone)]
pub enum LoginPoll {
    /// The user has not approved the device yet.
    Pending,
    /// Logged in; credentials were saved.
    Success(AuthData),
    /// The device code expired.
    Expired,
}

/// A HomeFi installation: app directory, config and saved credentials.
#[derive(Clone)]
pub struct Session {
    app_dir: PathBuf,
    config: Config,
    store: AuthStore,
    auth: AuthClient,
    omit_cache: bool,
    debug: bool,
}

impl Session {
    pub fn new(app_dir: impl Into<PathBuf>, config: Config) -> Self {
        let app_dir = app_dir.into();
        Self {
            store: AuthStore::new(app_dir.join(paths::AUTH_FILENAME)),
            omit_cache: !config.enable_cache,
            debug: config.debug,
            app_dir,
            config,
            auth: AuthClient::default(),
        }
    }

    /// Opens `$HOMEFI_PATH` (or `~/.homefi`) and its `config.toml`.
    pub fn open_default() -> Result<Self> {
        let app_dir = paths::app_dir();
        std::fs::create_dir_all(&app_dir)?;
        let config = Config::load(&app_dir.join(paths::CONFIG_FILENAME))?;
        Ok(Self::new(app_dir, config))
    }

    /// Serve every API response fresh (responses are still cached).
    pub fn omit_cache(mut self, omit: bool) -> Self {
        self.omit_cache = omit;
        self
    }

    pub fn debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }

    pub fn app_dir(&self) -> &Path {
        &self.app_dir
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn config_mut(&mut self) -> &mut Config {
        &mut self.config
    }

    pub fn cache(&self) -> DiskCache {
        DiskCache::new(self.app_dir.join(paths::CACHE_DIRNAME))
    }

    pub fn auth_store(&self) -> &AuthStore {
        &self.store
    }

    pub fn auth_data(&self) -> Result<AuthData> {
        self.store.load()
    }

    pub fn is_logged_in(&self) -> bool {
        self.store.load().map(|a| a.is_logged_in()).unwrap_or(false)
    }

    /// Starts the device login: show `DeviceAuth::url()` / `user_code` to
    /// the user, then call [`Session::poll_login`] every `interval` seconds.
    pub async fn start_login(&self) -> Result<DeviceAuth> {
        self.auth.device_auth().await
    }

    pub async fn poll_login(&self, device_code: &str) -> Result<LoginPoll> {
        match self.auth.token(device_code).await {
            Ok(res) => {
                let data = AuthData::from_response(&res);
                self.store.save(&data)?;
                Ok(LoginPoll::Success(data))
            }
            Err(e) if e.is_authorization_pending() => Ok(LoginPoll::Pending),
            Err(e) if e.is_expired_token() => Ok(LoginPoll::Expired),
            Err(e) => Err(e),
        }
    }

    /// Revokes the token on Tidal and clears local credentials. With
    /// `force`, local credentials are cleared even if the request fails.
    pub async fn logout(&self, force: bool) -> Result<()> {
        let data = self.store.load()?;
        let Some(token) = data.token else {
            return Ok(());
        };
        let remote = self.auth.logout(&token).await;
        if remote.is_ok() || force {
            self.store.clear()?;
        }
        remote
    }

    /// Refreshes the access token when it expires within `margin` seconds
    /// (or always, with `force`). Returns `true` if it was refreshed.
    pub async fn refresh(&self, margin: i64, force: bool) -> Result<bool> {
        let mut data = self.store.load()?;
        let refresh_token = data.refresh_token.clone().ok_or(Error::NotLoggedIn("no refresh token"))?;
        if !force && !data.expires_within(margin) {
            return Ok(false);
        }
        let res = self.auth.refresh(&refresh_token).await?;
        data.token = Some(res.access_token);
        data.expires_at = crate::auth::now_secs() + res.expires_in;
        self.store.save(&data)?;
        Ok(true)
    }

    /// Builds the API client. Expired tokens are refreshed transparently
    /// (and saved) when Tidal answers `401`.
    pub fn api(&self) -> Result<TidalApi> {
        let data = self.store.load()?;
        let token = data.token.clone().ok_or(Error::NotLoggedIn("auth token is missing"))?;
        let user_id = data.user_id.clone().ok_or(Error::NotLoggedIn("user id is missing"))?;
        let country = data.country_code.clone().ok_or(Error::NotLoggedIn("country code is missing"))?;
        if data.refresh_token.is_none() {
            return Err(Error::NotLoggedIn("refresh token is missing"));
        }

        let (store, auth) = (self.store.clone(), self.auth.clone());
        let refresher = Arc::new(move || {
            let (store, auth) = (store.clone(), auth.clone());
            async move {
                let mut data = store.load()?;
                let refresh = data.refresh_token.clone().ok_or(Error::NotLoggedIn("no refresh token"))?;
                let res = auth.refresh(&refresh).await?;
                data.token = Some(res.access_token.clone());
                data.expires_at = crate::auth::now_secs() + res.expires_in;
                store.save(&data)?;
                tracing::debug!("access token refreshed");
                Ok(res.access_token)
            }
            .boxed()
        });

        let mut client = TidalClient::new(token).with_refresher(refresher).with_cache(self.cache(), self.omit_cache);
        if self.debug {
            client = client.with_debug_dir(Some(self.app_dir.join(paths::DEBUG_DIRNAME)));
        }

        Ok(TidalApi::new(Arc::new(client), user_id, country))
    }
}
