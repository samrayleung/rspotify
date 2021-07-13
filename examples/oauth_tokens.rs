//! This example is specially useful for the OAuth tests. It simply obtains an
//! access token and a refresh token with all available scopes.
//!
//! Set RSPOTIFY_CLIENT_ID, RSPOTIFY_CLIENT_SECRET and RSPOTIFY_REDIRECT_URI in
//! an .env file or export them manually as environmental variables for this to
//! work.

use rspotify::{http::ReqwestClient, prelude::*, scopes, AuthCodeSpotify, Credentials, OAuth};

#[tokio::main]
async fn main() {
    // You can use any logger for debugging.
    env_logger::init();

    // The credentials must be available in the environment. Enable
    // `env-file` in order to read them from an `.env` file.
    let creds = Credentials::from_env().unwrap();

    // Using every possible scope
    let scopes = scopes!(
        "user-read-email",
        "user-read-private",
        "user-top-read",
        "user-read-recently-played",
        "user-follow-read",
        "user-library-read",
        "user-read-currently-playing",
        "user-read-playback-state",
        "user-read-playback-position",
        "playlist-read-collaborative",
        "playlist-read-private",
        "user-follow-modify",
        "user-library-modify",
        "user-modify-playback-state",
        "playlist-modify-public",
        "playlist-modify-private",
        "ugc-image-upload"
    );
    let oauth = OAuth::from_env(scopes).unwrap();

    let mut spotify = AuthCodeSpotify::<ReqwestClient>::new(creds, oauth);

    let url = spotify.get_authorize_url(false).unwrap();
    spotify.prompt_for_token(&url).await.unwrap();

    let token = spotify.token.as_ref().unwrap();
    println!("Access token: {}", &token.access_token);
    println!("Refresh token: {}", token.refresh_token.as_ref().unwrap());
}
