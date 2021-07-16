use crate::{
    auth_urls,
    clients::{BaseClient, OAuthClient},
    headers,
    http::{Form, HttpClient},
    ClientResult, Config, Credentials, OAuth, Token,
};

use std::{
    collections::HashMap,
    sync::{RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use maybe_async::maybe_async;
use url::Url;

/// The [Authorization Code Flow with Proof Key for Code Exchange
/// (PKCE)][reference] client for the Spotify API.
///
/// This flow is very similar to the regular Authorization Code Flow, so please
/// read [`AuthCodeSpotify`](crate::AuthCodeSpotify) for more information about
/// it. The main difference in this case is that you can avoid storing your
/// client secret by generating a *code verifier* and a *code challenge*.
///
/// There's an [example][example-main] available to learn how to use this
/// client.
///
/// [reference]: https://developer.spotify.com/documentation/general/guides/authorization-guide/#authorization-code-flow-with-proof-key-for-code-exchange-pkce
/// [example-main]: https://github.com/ramsayleung/rspotify/blob/master/examples/auth_code_pkce.rs
#[derive(Debug, Default)]
pub struct AuthCodePkceSpotify {
    pub creds: Credentials,
    pub oauth: OAuth,
    pub config: Config,
    pub token: RwLock<Option<Token>>,
    pub(in crate) http: HttpClient,
}

/// This client has access to the base methods.
#[maybe_async(?Send)]
impl BaseClient for AuthCodePkceSpotify {
    fn get_http(&self) -> &HttpClient {
        &self.http
    }

    async fn get_token(&self) -> RwLockReadGuard<Option<Token>> {
        self.auto_reauth()
            .await
            .expect("Failed to re-authenticate automatically, please authenticate");
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

/// This client includes user authorization, so it has access to the user
/// private endpoints in [`OAuthClient`].
#[maybe_async(?Send)]
impl OAuthClient for AuthCodePkceSpotify {
    fn get_oauth(&self) -> &OAuth {
        &self.oauth
    }

    async fn auto_reauth(&self) -> ClientResult<()> {
        // You could not have read lock and write lock at the same time, which
        // will result in deadlock, so obtain the write lock and use it in the
        // whole process.
        let mut token = self.get_token_mut();
        if self.config.token_refreshing && token.as_ref().map_or(false, |tok| tok.can_reauth()) {
            if let Some(re_tok) = token
                .as_ref()
                .map(|tok| tok.refresh_token.as_ref())
                .flatten()
            {
                let fetched_token = self.refetch_token(re_tok).await?;
                *token = Some(fetched_token);
                self.write_token_cache().await?
            };
        }
        Ok(())
    }

    async fn request_token(&self, code: &str) -> ClientResult<()> {
        // TODO
        let mut data = Form::new();
        let oauth = self.get_oauth();
        let scopes = oauth
            .scopes
            .clone()
            .into_iter()
            .collect::<Vec<_>>()
            .join(" ");
        data.insert(headers::GRANT_TYPE, headers::GRANT_AUTH_CODE);
        data.insert(headers::REDIRECT_URI, oauth.redirect_uri.as_ref());
        data.insert(headers::CODE, code);
        data.insert(headers::SCOPE, scopes.as_ref());
        data.insert(headers::STATE, oauth.state.as_ref());

        let token = self.fetch_access_token(&data).await?;
        *self.get_token_mut() = Some(token);

        self.write_token_cache().await
    }

    async fn refetch_token(&self, refresh_token: &str) -> ClientResult<Token> {
        let mut data = Form::new();
        data.insert(headers::REFRESH_TOKEN, refresh_token);
        data.insert(headers::GRANT_TYPE, headers::GRANT_REFRESH_TOKEN);

        let mut token = self.fetch_access_token(&data).await?;
        token.refresh_token = Some(refresh_token.to_string());
        Ok(token)
    }

    async fn refresh_token(&self, refresh_token: &str) -> ClientResult<()> {
        // TODO
        let token = self.refetch_token(refresh_token).await?;

        *self.get_token_mut() = Some(token);

        self.write_token_cache().await
    }
}

impl AuthCodePkceSpotify {
    /// Builds a new [`AuthCodePkceSpotify`] given a pair of client credentials
    /// and OAuth information.
    pub fn new(creds: Credentials, oauth: OAuth) -> Self {
        AuthCodePkceSpotify {
            creds,
            oauth,
            ..Default::default()
        }
    }

    /// Build a new [`AuthCodePkceSpotify`] from an already generated token.
    /// Note that once the token expires this will fail to make requests, as the
    /// client credentials aren't known.
    pub fn from_token(token: Token) -> Self {
        AuthCodePkceSpotify {
            token: RwLock::new(Some(token)),
            ..Default::default()
        }
    }

    /// Same as [`Self::new`] but with an extra parameter to configure the
    /// client.
    pub fn with_config(creds: Credentials, oauth: OAuth, config: Config) -> Self {
        AuthCodePkceSpotify {
            creds,
            oauth,
            config,
            ..Default::default()
        }
    }

    /// Returns the URL needed to authorize the current client as the first step
    /// in the authorization flow.
    pub fn get_authorize_url(&self) -> ClientResult<String> {
        // TODO
        let mut payload: HashMap<&str, &str> = HashMap::new();
        let oauth = self.get_oauth();
        let scopes = oauth
            .scopes
            .clone()
            .into_iter()
            .collect::<Vec<_>>()
            .join(" ");
        payload.insert(headers::CLIENT_ID, &self.get_creds().id);
        payload.insert(headers::RESPONSE_TYPE, headers::RESPONSE_CODE);
        payload.insert(headers::REDIRECT_URI, &oauth.redirect_uri);
        payload.insert(headers::SCOPE, &scopes);
        payload.insert(headers::STATE, &oauth.state);
        // payload.insert(headers::CODE_CHALLENGE, todo!());
        // payload.insert(headers::CODE_CHALLENGE_METHOD, "S256");

        let parsed = Url::parse_with_params(auth_urls::AUTHORIZE, payload)?;
        Ok(parsed.into())
    }
}
