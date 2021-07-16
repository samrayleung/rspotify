//! Automatically re-authentication means you only need to authenticate the
//! usual way at most once to get token. Then everytime you send a request to
//! Spotify server, it will check whether the token is expired and automatically
//! re-authenticate by refresh_token if set `Token.token_refreshing` to true.

use chrono::offset::Utc;
use chrono::Duration;
use rspotify::{
    model::Id, prelude::*, scopes, AuthCodeSpotify, ClientCredsSpotify, Config, Credentials, OAuth,
};

// Sample request that will follow some artists, print the user's
// followed artists, and then unfollow the artists.
async fn auth_code_do_things(spotify: AuthCodeSpotify) {
    let artists = vec![
        Id::from_id("3RGLhK1IP9jnYFH4BRFJBS").unwrap(), // The Clash
        Id::from_id("0yNLKJebCb8Aueb54LYya3").unwrap(), // New Order
        Id::from_id("2jzc5TC5TVFLXQlBNiIUzE").unwrap(), // a-ha
    ];
    spotify
        .user_follow_artists(artists.clone())
        .await
        .expect("couldn't follow artists");
    println!("Followed {} artists successfully.", artists.len());

    // Printing the followed artists
    let followed = spotify
        .current_user_followed_artists(None, None)
        .await
        .expect("couldn't get user followed artists");
    println!(
        "User currently follows at least {} artists.",
        followed.items.len()
    );

    spotify
        .user_unfollow_artists(artists.clone())
        .await
        .expect("couldn't unfollow artists");
    println!("Unfollowed {} artists successfully.", artists.len());
}

async fn client_creds_do_things(spotify: &ClientCredsSpotify) {
    // Running the requests
    let birdy_uri = Id::from_uri("spotify:album:0sNOF9WDwhWunNAHPD3Baj").unwrap();
    let albums = spotify.album(birdy_uri).await;
    println!("Get ablums: {}", albums.unwrap().uri);
}

#[tokio::main]
async fn main() {
    // You can use any logger for debugging.
    env_logger::init();

    // The default credentials from the `.env` file will be used by default.
    let creds = Credentials::from_env().unwrap();
    let oauth = OAuth::from_env(scopes!("user-follow-read user-follow-modify")).unwrap();
    let mut spotify = AuthCodeSpotify::new(creds.clone(), oauth.clone());

    // In the first session of the application we authenticate and obtain the
    // refresh token. We can also do some requests here.
    println!(">>> Session one, obtaining refresh token and running some requests:");
    let url = spotify.get_authorize_url(false).unwrap();
    spotify
        .prompt_for_token(&url)
        .await
        .expect("couldn't authenticate successfully");
    let refresh_token = spotify
        .token
        .read()
        .unwrap()
        .as_ref()
        .unwrap()
        .refresh_token
        .as_ref()
        .unwrap()
        .clone();
    auth_code_do_things(spotify).await;

    // Expiring the token, then it should automatical re-authenticate with refresh_token
    let mut config = Config::default();
    config.token_refreshing = true;
    let spotify = AuthCodeSpotify::with_config(creds.clone(), oauth, config.clone());
    spotify
        .refresh_token(&refresh_token)
        .await
        .expect("couldn't refresh user token");

    let now = Utc::now();
    now.checked_sub_signed(Duration::seconds(10));
    spotify.get_token_mut().as_mut().unwrap().expires_at = Some(now.clone());
    println!(">>> Session two, the token should expire, then re-auth automatically");
    auth_code_do_things(spotify).await;

    // Client-credential based spotify client
    let spotify = ClientCredsSpotify::with_config(creds.clone(), config);

    // Obtaining the access token. Requires to be mutable because the internal
    // token will be modified. We don't need OAuth for this specific endpoint,
    // so `...` is used instead of `prompt_for_user_token`.
    spotify.request_token().await.unwrap();
    println!("token: {:?}", spotify.get_token().await);
    println!(">>> New Session one from ClientCredsSpotify, obtaining token and do things");
    client_creds_do_things(&spotify).await;

    spotify.get_token_mut().as_mut().unwrap().expires_at = Some(now);
    println!(">>> New Session two from ClientCredsSpotify, expiring the token and then re-auth automatically");
    client_creds_do_things(&spotify).await;
}
