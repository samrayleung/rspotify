pub mod base;
pub mod oauth;
pub mod pagination;

pub use base::BaseClient;
pub use oauth::OAuthClient;

use crate::{
    model::{idtypes::IdType, Id},
    ClientResult, Token,
};

use serde::Deserialize;

/// Converts a JSON response from Spotify into its model.
pub(in crate) fn convert_result<'a, T: Deserialize<'a>>(input: &'a str) -> ClientResult<T> {
    serde_json::from_str::<T>(input).map_err(Into::into)
}

/// Append device ID to an API path.
pub(in crate) fn append_device_id(path: &str, device_id: Option<&str>) -> String {
    let mut new_path = path.to_string();
    if let Some(_device_id) = device_id {
        if path.contains('?') {
            new_path.push_str(&format!("&device_id={}", _device_id));
        } else {
            new_path.push_str(&format!("?device_id={}", _device_id));
        }
    }
    new_path
}

// TODO: move to `lib.rs`
#[inline]
pub(in crate) fn join_ids<'a, T: 'a + IdType>(ids: impl IntoIterator<Item = &'a Id<T>>) -> String {
    ids.into_iter().collect::<Vec<_>>().join(",")
}

// TODO: move to `lib.rs` or integrate into Token.
/// Generates an HTTP token authorization header with proper formatting
pub fn bearer_auth(tok: &Token) -> (String, String) {
    let auth = "authorization".to_owned();
    let value = format!("Bearer {}", tok.access_token);

    (auth, value)
}

// TODO: move to `lib.rs` or integrate into Credentials.
/// Generates an HTTP basic authorization header with proper formatting
pub fn basic_auth(user: &str, password: &str) -> (String, String) {
    let auth = "authorization".to_owned();
    let value = format!("{}:{}", user, password);
    let value = format!("Basic {}", base64::encode(value));

    (auth, value)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{scopes, ClientCredsSpotify, Token};
    use chrono::{prelude::*, Duration};

    #[test]
    fn test_append_device_id_without_question_mark() {
        let path = "me/player/play";
        let device_id = Some("fdafdsadfa");
        let new_path = append_device_id(path, device_id);
        assert_eq!(new_path, "me/player/play?device_id=fdafdsadfa");
    }

    #[test]
    fn test_append_device_id_with_question_mark() {
        let path = "me/player/shuffle?state=true";
        let device_id = Some("fdafdsadfa");
        let new_path = append_device_id(path, device_id);
        assert_eq!(
            new_path,
            "me/player/shuffle?state=true&device_id=fdafdsadfa"
        );
    }

    #[test]
    fn test_bearer_auth() {
        let tok = Token {
            access_token: "access_token".to_string(),
            ..Default::default()
        };

        let (auth, value) = bearer_auth(&tok);
        assert_eq!(auth, "authorization");
        assert_eq!(value, "Bearer access_token");
    }

    #[test]
    fn test_basic_auth() {
        let (auth, value) = basic_auth("ramsay", "123456");
        assert_eq!(auth, "authorization");
        assert_eq!(value, "Basic cmFtc2F5OjEyMzQ1Ng==");
    }

    #[test]
    fn test_endpoint_url() {
        let spotify = ClientCredsSpotify::default();
        assert_eq!(
            spotify.endpoint_url("me/player/play"),
            "https://api.spotify.com/v1/me/player/play"
        );
        assert_eq!(
            spotify.endpoint_url("http://api.spotify.com/v1/me/player/play"),
            "http://api.spotify.com/v1/me/player/play"
        );
        assert_eq!(
            spotify.endpoint_url("https://api.spotify.com/v1/me/player/play"),
            "https://api.spotify.com/v1/me/player/play"
        );
    }

    #[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
    async fn test_auth_headers() {
        let tok = Token {
            access_token: "test-access_token".to_string(),
            expires_in: Duration::seconds(1),
            expires_at: Some(Utc::now()),
            scopes: scopes!("playlist-read-private"),
            refresh_token: Some("...".to_string()),
        };

        let spotify = ClientCredsSpotify::from_token(tok);
        let headers = spotify.auth_headers().await.unwrap();
        assert_eq!(
            headers.get("authorization"),
            Some(&"Bearer test-access_token".to_owned())
        );
    }
}
