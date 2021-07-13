use crate::{
    clients::BaseClient,
    headers,
    http::{BaseHttpClient, Form},
    ClientResult, Config, Credentials, Token,
};

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
#[derive(Clone, Debug, Default)]
pub struct ClientCredsSpotify<Http: BaseHttpClient> {
    pub config: Config,
    pub creds: Credentials,
    pub token: Option<Token>,
    pub(in crate) http: Http,
}

/// This client has access to the base methods.
impl<Http: BaseHttpClient> BaseClient<Http> for ClientCredsSpotify<Http> {
    fn get_http(&self) -> &Http {
        &self.http
    }

    fn get_token(&self) -> Option<&Token> {
        self.token.as_ref()
    }

    fn get_token_mut(&mut self) -> Option<&mut Token> {
        self.token.as_mut()
    }

    fn get_creds(&self) -> &Credentials {
        &self.creds
    }

    fn get_config(&self) -> &Config {
        &self.config
    }
}

impl<Http: BaseHttpClient> ClientCredsSpotify<Http> {
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
            token: Some(token),
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
    pub async fn request_token(&mut self) -> ClientResult<()> {
        let mut data = Form::new();
        data.insert(headers::GRANT_TYPE, headers::GRANT_CLIENT_CREDS);

        self.token = Some(self.fetch_access_token(&data).await?);

        self.write_token_cache()
    }
}
