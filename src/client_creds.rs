use crate::{
    clients::BaseClient,
    headers,
    http::{Form, HttpClient},
    ClientResult, Config, Credentials, Token,
};

use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use maybe_async::maybe_async;

/// The [Client Credentials Flow][reference] client for the Spotify API.
///
/// This is the most basic flow. It requests a token to Spotify given some
/// client credentials, without user authorization. The only step to take is to
/// call [`Self::request_token`]. See [this example][example-main].
///
/// Note: This flow does not include authorization and therefore cannot be used
/// to access or to manage the endpoints related to user private data in
/// [`OAuthClient`](crate::clients::OAuthClient).
///
/// [reference]: https://developer.spotify.com/documentation/general/guides/authorization-guide/#client-credentials-flow
/// [example-main]: https://github.com/ramsayleung/rspotify/blob/master/examples/client_creds.rs
#[derive(Debug, Default)]
pub struct ClientCredsSpotify {
    pub config: Config,
    pub creds: Credentials,
    pub token: RwLock<Option<Token>>,
    pub(in crate) http: HttpClient,
}

/// This client has access to the base methods.
#[maybe_async(?Send)]
impl BaseClient for ClientCredsSpotify {
    fn get_http(&self) -> &HttpClient {
        &self.http
    }

    async fn get_token(&self) -> RwLockReadGuard<Option<Token>> {
        self.auto_reauth()
            .await
            .expect("Failed to re-authenticate automatically, please obtain the token again");
        self.token
            .read()
            .expect("Failed to read token; the lock has been poisoned")
    }

    fn get_token_mut(&self) -> RwLockWriteGuard<Option<Token>> {
        self.token
            .write()
            .expect("Failed to write token; the lock has been poisoned")
    }

    fn get_creds(&self) -> &Credentials {
        &self.creds
    }

    fn get_config(&self) -> &Config {
        &self.config
    }
}

impl ClientCredsSpotify {
    /// Builds a new [`ClientCredsSpotify`] given a pair of client credentials
    /// and OAuth information.
    pub fn new(creds: Credentials) -> Self {
        ClientCredsSpotify {
            creds,
            ..Default::default()
        }
    }

    /// Build a new [`ClientCredsSpotify`] from an already generated token. Note
    /// that once the token expires this will fail to make requests,
    /// as the client credentials aren't known.
    pub fn from_token(token: Token) -> Self {
        ClientCredsSpotify {
            token: RwLock::new(Some(token)),
            ..Default::default()
        }
    }

    /// Same as [`Self::new`] but with an extra parameter to configure the
    /// client.
    pub fn with_config(creds: Credentials, config: Config) -> Self {
        ClientCredsSpotify {
            config,
            creds,
            ..Default::default()
        }
    }

    /// Tries to read the cache file's token, which may not exist.
    ///
    /// Similarly to [`Self::write_token_cache`], this will already check if the
    /// cached token is enabled and return `None` in case it isn't.
    #[maybe_async]
    pub async fn read_token_cache(&self) -> Option<Token> {
        if !self.get_config().token_cached {
            return None;
        }

        let token = Token::from_cache(&self.get_config().cache_path)?;
        if token.is_expired() {
            // Invalid token, since it doesn't have at least the currently
            // required scopes or it's expired.
            None
        } else {
            Some(token)
        }
    }

    /// Obtains the client access token for the app. The resulting token will be
    /// saved internally.
    #[maybe_async]
    pub async fn request_token(&self) -> ClientResult<()> {
        *self.get_token_mut() = Some(self.fetch_token().await?);

        self.write_token_cache().await
    }

    /// Fetch access token
    #[maybe_async]
    async fn fetch_token(&self) -> ClientResult<Token> {
        let mut data = Form::new();

        data.insert(headers::GRANT_TYPE, headers::GRANT_CLIENT_CREDS);

        let token = self.fetch_access_token(&data).await?;
        Ok(token)
    }

    /// Re-authenticate automatically if it's configured to do so, which
    /// authenticates the usual way to obtain a new access token.
    #[maybe_async]
    async fn auto_reauth(&self) -> ClientResult<()> {
        // You could not have read lock and write lock at the same time, which
        // will result in deadlock, so obtain the write lock and use it in the
        // whole process.
        let mut token = self.get_token_mut();
        if self.config.token_refreshing && token.as_ref().map_or(false, |tok| tok.is_expired()) {
            *token = Some(self.fetch_token().await?);
            self.write_token_cache().await?
        }
        Ok(())
    }
}
