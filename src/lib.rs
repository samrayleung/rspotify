//! Rspotify is a wrapper for the [Spotify Web API
//! ](https://developer.spotify.com/web-api/), inspired by [spotipy
//! ](https://github.com/plamere/spotipy). It includes support for all the
//! [authorization flows](https://developer.spotify.com/documentation/general/guides/authorization-guide/),
//! and helper methods for [all available endpoints
//! ](https://developer.spotify.com/documentation/web-api/reference/).
//!
//! ## Configuration
//!
//! ### HTTP Client
//!
//! By default, Rspotify uses the [`reqwest`] asynchronous HTTP client with its
//! default TLS, but you can customize both the HTTP client and the TLS with the
//! following features:
//!
//! - [`reqwest`](https://github.com/seanmonstar/reqwest): enabling
//!   `client-reqwest`, TLS available:
//!     + `reqwest-default-tls` (reqwest's default)
//!     + `reqwest-rustls-tls`
//!     + `reqwest-native-tls`
//!     + `reqwest-native-tls-vendored`
//! - [`ureq`](https://github.com/algesten/ureq): enabling `client-ureq`, TLS
//!   available:
//!     + `ureq-rustls-tls` (ureq's default)
//!
//! If you want to use a different client or TLS than the default ones, you'll
//! have to disable the default features and enable whichever you want. For
//! example, this would compile Rspotify with `reqwest` and the native TLS:
//!
//! ```toml
//! [dependencies]
//! rspotify = {
//!     version = "...",
//!     default-features = false,
//!     features = ["client-reqwest", "reqwest-native-tls"]
//! }
//! ```
//!
//! [`maybe_async`] internally enables Rspotify to  use both synchronous and
//! asynchronous HTTP clients. You can also use `ureq`, a synchronous client,
//! like so:
//!
//! ```toml
//! [dependencies]
//! rspotify = {
//!     version = "...",
//!     default-features = false,
//!     features = ["client-ureq", "ureq-rustls-tls"]
//! }
//! ```
//!
//! ### Proxies
//!
//! [`reqwest`](reqwest#proxies) supports system proxies by default. It reads
//! the environment variables `HTTP_PROXY` and `HTTPS_PROXY` environmental
//! variables to set HTTP and HTTPS proxies, respectively.
//!
//! ### Environmental variables
//!
//! Rspotify supports the [`dotenv`] crate, which allows you to save credentials
//! in a `.env` file. These will then be automatically available as
//! environmental values when using methods like
//! [`CredentialsBuilder::from_env`](crate::oauth2::CredentialsBuilder::from_env):
//!
//! ```toml
//! [dependencies]
//! rspotify = { version = "...", features = ["env-file"] }
//! ```
//!
//! ### Cli utilities
//!
//! Rspotify includes basic support for Cli apps to obtain access tokens by
//! prompting the user, after enabling the `cli` feature. See the [Authorization
//! ](#authorization) section for more information.
//!
//! ## Getting Started
//!
//! ### Authorization
//!
//! All endpoints require authorization. You will need to generate a token
//! that indicates that the client has been granted permission to perform
//! requests. You will need to [register your app to get the necessary client
//! credentials](https://developer.spotify.com/dashboard/applications). Read
//! the [official guide for a detailed explanation of the different
//! authorization flows available
//! ](https://developer.spotify.com/documentation/general/guides/authorization-guide/).
//!
//! The most basic authentication flow, named the [Client Credentials flow
//! ](https://developer.spotify.com/documentation/general/guides/authorization-guide/#client-credentials-flow),
//! consists on requesting a token to Spotify given some client credentials.
//! This can be done with [`Spotify::request_client_token`
//! ](crate::client::Spotify::request_client_token), as seen in
//! [this example
//! ](https://github.com/ramsayleung/rspotify/blob/master/examples/album.rs).
//!
//! Some of the available endpoints also require access to the user's personal
//! information, meaning that you have to follow the [Authorization Flow
//! ](https://developer.spotify.com/documentation/general/guides/authorization-guide/#authorization-code-flow)
//! instead. In a nutshell, these are the steps you need to make for this:
//!
//! 0. Generate a request URL with [`Spotify::get_authorize_url`
//!    ](crate::client::Spotify::get_authorize_url).
//! 1. The user logs in with the request URL, which redirects to the redirect
//!    URI and provides a code in the parameters. This happens on your side.
//! 2. The code obtained in the previous step is parsed with
//!    [`Spotify::parse_response_code`
//!    ](crate::client::Spotify::parse_response_code).
//! 3. The code is sent to Spotify in order to obtain an access token with
//!    [`Spotify::request_user_token`
//!    ](crate::client::Spotify::request_user_token) or
//!    [`Spotify::request_user_token_without_cache`
//!    ](crate::client::Spotify::prompt_for_user_token_without_cache).
//! 4. Finally, this access token can be used internally for the requests.
//!    This access token may expire relatively soon, so it can be refreshed
//!    with the refresh token (obtained in the third step as well) using
//!    [`Spotify::refresh_user_token`
//!    ](crate::client::Spotify::refresh_user_token) or
//!    [`Spotify::refresh_user_token_without_cache`
//!    ](crate::client::Spotify::refresh_user_token_without_cache).
//!    Otherwise, a new access token may be generated from scratch by repeating
//!    these steps, but the advantage of refreshing it is that this doesn't
//!    require the user to log in, and that it's a simpler procedure.
//!
//! See the [`webapp`
//! ](https://github.com/ramsayleung/rspotify/tree/master/examples/webapp)
//! example for more details on how you can implement it for something like a
//! web server.
//!
//! If you're developing a Cli application, you might be interested in the
//! `cli` feature, which brings the [`Spotify::prompt_for_user_token`
//! ](crate::client::Spotify::prompt_for_user_token) and
//! [`Spotify::prompt_for_user_token_without_cache`
//! ](crate::client::Spotify::prompt_for_user_token_without_cache)
//! methods. These will run all the authentication steps. The user wil log in
//! by opening the request URL in its default browser, and the requests will be
//! performed automatically.
//!
//! An example of the Cli authentication:
//!
//! ![demo](https://raw.githubusercontent.com/ramsayleung/rspotify/master/doc/images/rspotify.gif)
//!
//! Note: even if your script does not have an accessible URL, you will have to
//! specify a redirect URI. It doesn't need to work or be accessible, you can
//! use `http://localhost:8888/callback` for example, which will also have the
//! code appended like so: `http://localhost/?code=...`.
//!
//! In order to help other developers to get used to `rspotify`, there are
//! public credentials available for a dummy account. You can test `rspotify`
//! with this account's `RSPOTIFY_CLIENT_ID` and `RSPOTIFY_CLIENT_SECRET`
//! inside the [`.env` file
//! ](https://github.com/ramsayleung/rspotify/blob/master/.env) for more
//! details.
//!
//! ### Examples
//!
//! There are some [available examples
//! ](https://github.com/ramsayleung/rspotify/tree/master/examples)
//! which can serve as a learning tool.

// Disable all modules when both client features are enabled or when none are.
// This way only the compile error below gets shown instead of a whole list of
// confusing errors..

pub mod client_creds;
pub mod code_auth;
pub mod code_auth_pkce;
pub mod endpoints;

// Subcrate re-exports
pub use rspotify_http as http;
pub use rspotify_macros as macros;
pub use rspotify_model as model;
// Top-level re-exports
pub use macros::scopes;

use std::{
    collections::HashSet,
    env, fs,
    io::{Read, Write},
    path::Path,
    path::PathBuf,
};

use chrono::{DateTime, Duration, Utc};
use derive_builder::Builder;
use getrandom::getrandom;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod prelude {
    pub use crate::endpoints::{BaseClient, OAuthClient};
}

/// Possible errors returned from the `rspotify` client.
#[derive(Debug, Error)]
pub enum ClientError {
    /// Raised when the authentication isn't configured properly.
    #[error("invalid client authentication: {0}")]
    InvalidAuth(String),

    #[error("json parse error: {0}")]
    ParseJson(#[from] serde_json::Error),

    #[error("url parse error: {0}")]
    ParseUrl(#[from] url::ParseError),

    #[error("http error: {0}")]
    Http(#[from] http::Error),

    #[error("input/output error: {0}")]
    Io(#[from] std::io::Error),

    #[cfg(feature = "cli")]
    #[error("cli error: {0}")]
    Cli(String),

    #[error("cache file error: {0}")]
    CacheFile(String),
}

pub type ClientResult<T> = Result<T, ClientError>;

pub const DEFAULT_API_PREFIX: &str = "https://api.spotify.com/v1/";
pub const DEFAULT_CACHE_PATH: &str = ".spotify_token_cache.json";
pub const DEFAULT_PAGINATION_CHUNKS: u32 = 50;

/// Struct to configure the Spotify client.
#[derive(Debug, Clone)]
pub struct Config {
    /// The Spotify API prefix, [`DEFAULT_API_PREFIX`] by default.
    pub prefix: String,

    /// The cache file path, in case it's used. By default it's
    /// [`DEFAULT_CACHE_PATH`]
    pub cache_path: PathBuf,

    /// The pagination chunk size used when performing automatically paginated
    /// requests, like [`Spotify::artist_albums`]. This means that a request
    /// will be performed every `pagination_chunks` items. By default this is
    /// [`DEFAULT_PAGINATION_CHUNKS`].
    ///
    /// Note that most endpoints set a maximum to the number of items per
    /// request, which most times is 50.
    pub pagination_chunks: u32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            prefix: String::from(DEFAULT_API_PREFIX),
            cache_path: PathBuf::from(DEFAULT_CACHE_PATH),
            pagination_chunks: DEFAULT_PAGINATION_CHUNKS,
        }
    }
}

/// Generate `length` random chars
pub(in crate) fn generate_random_string(length: usize) -> String {
    let alphanum: &[u8] =
        "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".as_bytes();
    let mut buf = vec![0u8; length];
    getrandom(&mut buf).unwrap();
    let range = alphanum.len();

    buf.iter()
        .map(|byte| alphanum[*byte as usize % range] as char)
        .collect()
}

mod auth_urls {
    pub const AUTHORIZE: &str = "https://accounts.spotify.com/authorize";
    pub const TOKEN: &str = "https://accounts.spotify.com/api/token";
}

mod duration_second {
    use chrono::Duration;
    use serde::{de, Deserialize, Serializer};

    /// Deserialize `chrono::Duration` from seconds (represented as u64)
    pub(in crate) fn deserialize<'de, D>(d: D) -> Result<Duration, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let duration: i64 = Deserialize::deserialize(d)?;
        Ok(Duration::seconds(duration))
    }

    /// Serialize `chrono::Duration` to seconds (represented as u64)
    pub(in crate) fn serialize<S>(x: &Duration, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        s.serialize_i64(x.num_seconds())
    }
}

mod space_separated_scope {
    use serde::{de, Deserialize, Serializer};
    use std::collections::HashSet;

    pub(crate) fn deserialize<'de, D>(d: D) -> Result<HashSet<String>, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let scope: &str = Deserialize::deserialize(d)?;
        Ok(scope.split_whitespace().map(|x| x.to_owned()).collect())
    }

    pub(crate) fn serialize<S>(scope: &HashSet<String>, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let scope = scope.clone().into_iter().collect::<Vec<_>>().join(" ");
        s.serialize_str(&scope)
    }
}

/// Spotify access token information
/// [Reference](https://developer.spotify.com/documentation/general/guides/authorization-guide/)
#[derive(Builder, Clone, Debug, Serialize, Deserialize)]
pub struct Token {
    /// An access token that can be provided in subsequent calls
    #[builder(setter(into))]
    pub access_token: String,
    /// The time period for which the access token is valid.
    #[builder(default = "Duration::seconds(0)")]
    #[serde(with = "duration_second")]
    pub expires_in: Duration,
    /// The valid time for which the access token is available represented
    /// in ISO 8601 combined date and time.
    #[builder(setter(strip_option), default = "Some(Utc::now())")]
    pub expires_at: Option<DateTime<Utc>>,
    /// A token that can be sent to the Spotify Accounts service
    /// in place of an authorization code
    #[builder(setter(into, strip_option), default)]
    pub refresh_token: Option<String>,
    /// A list of [scopes](https://developer.spotify.com/documentation/general/guides/scopes/)
    /// which have been granted for this `access_token`
    /// You could use macro [scopes!](crate::scopes) to build it at compile time easily
    #[builder(default)]
    #[serde(default, with = "space_separated_scope")]
    pub scope: HashSet<String>,
}

impl TokenBuilder {
    /// Tries to initialize the token from a cache file.
    pub fn from_cache<T: AsRef<Path>>(path: T) -> Self {
        if let Ok(mut file) = fs::File::open(path) {
            let mut tok_str = String::new();
            if file.read_to_string(&mut tok_str).is_ok() {
                if let Ok(tok) = serde_json::from_str::<Token>(&tok_str) {
                    return TokenBuilder {
                        access_token: Some(tok.access_token),
                        expires_in: Some(tok.expires_in),
                        expires_at: Some(tok.expires_at),
                        refresh_token: Some(tok.refresh_token),
                        scope: Some(tok.scope),
                    };
                }
            }
        }

        TokenBuilder::default()
    }
}

impl Token {
    /// Saves the token information into its cache file.
    pub fn write_cache<T: AsRef<Path>>(&self, path: T) -> ClientResult<()> {
        let token_info = serde_json::to_string(&self)?;

        let mut file = fs::OpenOptions::new().write(true).create(true).open(path)?;
        file.set_len(0)?;
        file.write_all(token_info.as_bytes())?;

        Ok(())
    }

    /// Check if the token is expired
    pub fn is_expired(&self) -> bool {
        self.expires_at
            .map_or(true, |x| Utc::now().timestamp() > x.timestamp())
    }
}

/// Simple client credentials object for Spotify.
#[derive(Builder, Debug, Default, Clone, Serialize, Deserialize)]
pub struct Credentials {
    #[builder(setter(into))]
    pub id: String,
    #[builder(setter(into))]
    pub secret: String,
}

impl CredentialsBuilder {
    /// Parses the credentials from the environment variables
    /// `RSPOTIFY_CLIENT_ID` and `RSPOTIFY_CLIENT_SECRET`. You can optionally
    /// activate the `env-file` feature in order to read these variables from
    /// a `.env` file.
    pub fn from_env() -> Self {
        #[cfg(feature = "env-file")]
        {
            dotenv::dotenv().ok();
        }

        CredentialsBuilder {
            id: env::var("RSPOTIFY_CLIENT_ID").ok(),
            secret: env::var("RSPOTIFY_CLIENT_SECRET").ok(),
        }
    }
}

/// Structure that holds the required information for requests with OAuth.
#[derive(Builder, Debug, Default, Clone, Serialize, Deserialize)]
pub struct OAuth {
    #[builder(setter(into))]
    pub redirect_uri: String,
    /// The state is generated by default, as suggested by the OAuth2 spec:
    /// [Cross-Site Request Forgery](https://tools.ietf.org/html/rfc6749#section-10.12)
    #[builder(setter(into), default = "generate_random_string(16)")]
    pub state: String,
    /// You could use macro [scopes!](crate::scopes) to build it at compile time easily
    #[builder(default)]
    pub scope: HashSet<String>,
    #[builder(setter(into, strip_option), default)]
    pub proxies: Option<String>,
}

impl OAuthBuilder {
    /// Parses the credentials from the environment variable
    /// `RSPOTIFY_REDIRECT_URI`. You can optionally activate the `env-file`
    /// feature in order to read these variables from a `.env` file.
    pub fn from_env() -> Self {
        #[cfg(feature = "env-file")]
        {
            dotenv::dotenv().ok();
        }

        OAuthBuilder {
            redirect_uri: env::var("RSPOTIFY_REDIRECT_URI").ok(),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_response_code() {
        let url = "http://localhost:8888/callback?code=AQD0yXvFEOvw&state=sN#_=_";
        let spotify = SpotifyBuilder::default().build().unwrap();
        let code = spotify.parse_response_code(url).unwrap();
        assert_eq!(code, "AQD0yXvFEOvw");
    }

    #[test]
    fn test_append_device_id_without_question_mark() {
        let path = "me/player/play";
        let device_id = Some("fdafdsadfa");
        let spotify = SpotifyBuilder::default().build().unwrap();
        let new_path = spotify.append_device_id(path, device_id);
        assert_eq!(new_path, "me/player/play?device_id=fdafdsadfa");
    }

    #[test]
    fn test_append_device_id_with_question_mark() {
        let path = "me/player/shuffle?state=true";
        let device_id = Some("fdafdsadfa");
        let spotify = SpotifyBuilder::default().build().unwrap();
        let new_path = spotify.append_device_id(path, device_id);
        assert_eq!(
            new_path,
            "me/player/shuffle?state=true&device_id=fdafdsadfa"
        );
    }
}
