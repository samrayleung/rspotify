//! Refresh tokens aren't meant to expire, so they can be used as a persistent
//! authentication method without the need for user's interaction for oauth
//! requests. You still need to authenticate the usual way at least once to
//! obtain the refresh token, and you may need to obtain a new one if you change
//! the required scope.
//!
//! The cache generated by `get_token` uses the refresh token under the hood to
//! automatically authenticate the user. This example shows how it's done
//! because sometimes it's not possible to use this cache file (a web server for
//! example).
//!
//! *Note*: refresh tokens can actually expire, [as the OAuth2 spec
//! indicates](https://tools.ietf.org/html/rfc6749#section-6), but this [hasn't
//! actually happened in months with some
//! tokens](https://github.com/felix-hilden/tekore/issues/86), so in the case of
//! Spotify it doesn't seem to revoke them at all.

use rspotify::{model::Id, prelude::*, scopes, CodeAuthSpotify, Credentials, OAuth};

// Sample request that will follow some artists, print the user's
// followed artists, and then unfollow the artists.
async fn do_things(spotify: CodeAuthSpotify) {
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

#[tokio::main]
async fn main() {
    // You can use any logger for debugging.
    env_logger::init();

    // The default credentials from the `.env` file will be used by default.
    let creds = Credentials::from_env().unwrap();
    let oauth = OAuth::from_env(scopes!("user-follow-read user-follow-modify")).unwrap();
    let mut spotify = CodeAuthSpotify::new(creds.clone(), oauth.clone());

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
        .as_ref()
        .unwrap()
        .refresh_token
        .as_ref()
        .unwrap()
        .clone();
    do_things(spotify).await;

    // At a different time, the refresh token can be used to refresh an access
    // token directly and run requests:
    println!(">>> Session two, running some requests:");
    let mut spotify = CodeAuthSpotify::new(creds.clone(), oauth.clone());
    // No `prompt_for_user_token_without_cache` needed.
    spotify
        .refresh_token(&refresh_token)
        .await
        .expect("couldn't refresh user token");
    do_things(spotify).await;

    // This process can now be repeated multiple times by using only the
    // refresh token that was obtained at the beginning.
    println!(">>> Session three, running some requests:");
    let mut spotify = CodeAuthSpotify::new(creds, oauth);
    spotify
        .refresh_token(&refresh_token)
        .await
        .expect("couldn't refresh user token");
    do_things(spotify).await;
}
