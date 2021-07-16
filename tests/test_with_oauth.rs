//! Most of tests currently require a Spotify Premium account where the tests
//! can be ran, which will be ignored in the CI for now. The tests are written
//! so that no user data is modified.
//!
//! You can run all of them with:
//!
//! ```
//! $ cargo test --features=cli,env-file -- --ignored
//! ```
//!
//! The access token must be obtained previously, and this test file will try
//! to authenticate with the access token from the `RSPOTIFY_ACCESS_TOKEN`
//! environment variable or the refresh token from `RSPOTIFY_REFRESH_TOKEN`
//! (these tokens must have been generated for all available scopes, see
//! the `oauth_tokens` example).

use rspotify::{
    model::{
        Country, EpisodeId, Id, Market, Offset, RepeatState, SearchType, ShowId, TimeRange,
        TrackId, TrackPositions,
    },
    prelude::*,
    scopes, AuthCodeSpotify, Credentials, OAuth, Token,
};

use chrono::prelude::*;
use maybe_async::maybe_async;
use serde_json::map::Map;
use std::env;

/// Generating a new OAuth client for the requests.
#[maybe_async]
pub async fn oauth_client() -> AuthCodeSpotify {
    if let Ok(access_token) = env::var("RSPOTIFY_ACCESS_TOKEN") {
        let tok = Token {
            access_token,
            ..Default::default()
        };

        AuthCodeSpotify::from_token(tok)
    } else if let Ok(refresh_token) = env::var("RSPOTIFY_REFRESH_TOKEN") {
        // The credentials must be available in the environment. Enable
        // `env-file` in order to read them from an `.env` file.
        let creds = Credentials::from_env().unwrap_or_else(|| {
            panic!(
                "No credentials configured. Make sure that either the \
                `env-file` feature is enabled, or that the required \
                environment variables are exported (`RSPOTIFY_CLIENT_ID`, \
                `RSPOTIFY_CLIENT_SECRET`)."
            )
        });

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
        // Using every possible scope
        let oauth = OAuth::from_env(scopes).unwrap();

        let spotify = AuthCodeSpotify::new(creds, oauth);
        spotify.refresh_token(&refresh_token).await.unwrap();
        spotify
    } else {
        panic!(
            "No access tokens configured. Please set `RSPOTIFY_ACCESS_TOKEN` \
             or `RSPOTIFY_REFRESH_TOKEN`, which can be obtained with the \
             `oauth_tokens` example."
        )
    }
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_categories() {
    oauth_client()
        .await
        .categories_manual(
            None,
            Some(&Market::Country(Country::UnitedStates)),
            Some(10),
            Some(0),
        )
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_category_playlists() {
    oauth_client()
        .await
        .category_playlists_manual(
            "pop",
            Some(&Market::Country(Country::UnitedStates)),
            Some(10),
            Some(0),
        )
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_current_playback() {
    oauth_client()
        .await
        .current_playback(None, None::<&[_]>)
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_current_playing() {
    oauth_client()
        .await
        .current_playing(None, None::<&[_]>)
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_current_user_followed_artists() {
    oauth_client()
        .await
        .current_user_followed_artists(None, Some(10))
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_current_user_playing_track() {
    oauth_client()
        .await
        .current_user_playing_track()
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_current_user_playlists() {
    oauth_client()
        .await
        .current_user_playlists_manual(Some(10), None)
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_current_user_recently_played() {
    oauth_client()
        .await
        .current_user_recently_played(Some(10))
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_current_user_saved_albums_add() {
    let mut album_ids = vec![];
    let album_id1 = "6akEvsycLGftJxYudPjmqK";
    let album_id2 = "628oezqK2qfmCjC6eXNors";
    album_ids.push(Id::from_id(album_id2).unwrap());
    album_ids.push(Id::from_id(album_id1).unwrap());
    oauth_client()
        .await
        .current_user_saved_albums_add(album_ids)
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_current_user_saved_albums_delete() {
    let mut album_ids = vec![];
    let album_id1 = "6akEvsycLGftJxYudPjmqK";
    let album_id2 = "628oezqK2qfmCjC6eXNors";
    album_ids.push(Id::from_id(album_id2).unwrap());
    album_ids.push(Id::from_id(album_id1).unwrap());
    oauth_client()
        .await
        .current_user_saved_albums_delete(album_ids)
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_current_user_saved_albums() {
    oauth_client()
        .await
        .current_user_saved_albums_manual(Some(10), Some(0))
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_current_user_saved_tracks_add() {
    let mut tracks_ids = vec![];
    let track_id1 = "spotify:track:4iV5W9uYEdYUVa79Axb7Rh";
    let track_id2 = "spotify:track:1301WleyT98MSxVHPZCA6M";
    tracks_ids.push(Id::from_uri(track_id2).unwrap());
    tracks_ids.push(Id::from_uri(track_id1).unwrap());
    oauth_client()
        .await
        .current_user_saved_tracks_add(tracks_ids)
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_current_user_saved_tracks_contains() {
    let mut tracks_ids = vec![];
    let track_id1 = "spotify:track:4iV5W9uYEdYUVa79Axb7Rh";
    let track_id2 = "spotify:track:1301WleyT98MSxVHPZCA6M";
    tracks_ids.push(Id::from_uri(track_id2).unwrap());
    tracks_ids.push(Id::from_uri(track_id1).unwrap());
    oauth_client()
        .await
        .current_user_saved_tracks_contains(tracks_ids)
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_current_user_saved_tracks_delete() {
    let mut tracks_ids = vec![];
    let track_id1 = "spotify:track:4iV5W9uYEdYUVa79Axb7Rh";
    let track_id2 = "spotify:track:1301WleyT98MSxVHPZCA6M";
    tracks_ids.push(Id::from_uri(track_id2).unwrap());
    tracks_ids.push(Id::from_uri(track_id1).unwrap());
    oauth_client()
        .await
        .current_user_saved_tracks_delete(tracks_ids)
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_current_user_saved_tracks() {
    oauth_client()
        .await
        .current_user_saved_tracks_manual(Some(10), Some(0))
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_current_user_top_artists() {
    oauth_client()
        .await
        .current_user_top_artists_manual(Some(&TimeRange::ShortTerm), Some(10), Some(0))
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_current_user_top_tracks() {
    oauth_client()
        .await
        .current_user_top_tracks_manual(Some(&TimeRange::ShortTerm), Some(10), Some(0))
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_device() {
    oauth_client().await.device().await.unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_featured_playlists() {
    let now: DateTime<Utc> = Utc::now();
    oauth_client()
        .await
        .featured_playlists(None, None, Some(&now), Some(10), Some(0))
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_me() {
    oauth_client().await.me().await.unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_new_releases() {
    oauth_client()
        .await
        .new_releases_manual(Some(&Market::Country(Country::Sweden)), Some(10), Some(0))
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_new_releases_with_from_token() {
    oauth_client()
        .await
        .new_releases_manual(Some(&Market::FromToken), Some(10), Some(0))
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_next_playback() {
    let device_id = "74ASZWbe4lXaubB36ztrGX";
    oauth_client()
        .await
        .next_track(Some(device_id))
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_pause_playback() {
    let device_id = "74ASZWbe4lXaubB36ztrGX";
    oauth_client()
        .await
        .pause_playback(Some(device_id))
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_previous_playback() {
    let device_id = "74ASZWbe4lXaubB36ztrGX";
    oauth_client()
        .await
        .previous_track(Some(device_id))
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_recommendations() {
    let mut payload = Map::new();
    let seed_artists = vec![Id::from_id("4NHQUGzhtTLFvgF5SZesLK").unwrap()];
    let seed_tracks = vec![Id::from_id("0c6xIDDpzE81m2q797ordA").unwrap()];
    payload.insert("min_energy".to_owned(), 0.4.into());
    payload.insert("min_popularity".to_owned(), 50.into());
    oauth_client()
        .await
        .recommendations(
            &payload,
            Some(seed_artists),
            None::<Vec<&str>>,
            Some(seed_tracks),
            Some(&Market::Country(Country::UnitedStates)),
            Some(10),
        )
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_repeat() {
    oauth_client()
        .await
        .repeat(&RepeatState::Context, None)
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_search_album() {
    let query = "album:arrival artist:abba";
    oauth_client()
        .await
        .search(query, &SearchType::Album, None, None, Some(10), Some(0))
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_search_artist() {
    let query = "tania bowra";
    oauth_client()
        .await
        .search(
            query,
            &SearchType::Artist,
            Some(&Market::Country(Country::UnitedStates)),
            None,
            Some(10),
            Some(0),
        )
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_search_playlist() {
    let query = "\"doom metal\"";
    oauth_client()
        .await
        .search(
            query,
            &SearchType::Playlist,
            Some(&Market::Country(Country::UnitedStates)),
            None,
            Some(10),
            Some(0),
        )
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_search_track() {
    let query = "abba";
    oauth_client()
        .await
        .search(
            query,
            &SearchType::Track,
            Some(&Market::Country(Country::UnitedStates)),
            None,
            Some(10),
            Some(0),
        )
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_seek_track() {
    oauth_client().await.seek_track(25000, None).await.unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_shuffle() {
    oauth_client().await.shuffle(true, None).await.unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_start_playback() {
    let device_id = "74ASZWbe4lXaubB36ztrGX";
    let uris = vec![TrackId::from_uri("spotify:track:4iV5W9uYEdYUVa79Axb7Rh").unwrap()];
    oauth_client()
        .await
        .start_uris_playback(uris, Some(device_id), Some(Offset::for_position(0)), None)
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_transfer_playback() {
    let device_id = "74ASZWbe4lXaubB36ztrGX";
    oauth_client()
        .await
        .transfer_playback(device_id, Some(true))
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_user_follow_artist() {
    let mut artists = vec![];
    let artist_id1 = "74ASZWbe4lXaubB36ztrGX";
    let artist_id2 = "08td7MxkoHQkXnWAYD8d6Q";
    artists.push(Id::from_id(artist_id2).unwrap());
    artists.push(Id::from_id(artist_id1).unwrap());
    oauth_client()
        .await
        .user_follow_artists(artists)
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_user_unfollow_artist() {
    let mut artists = vec![];
    let artist_id1 = "74ASZWbe4lXaubB36ztrGX";
    let artist_id2 = "08td7MxkoHQkXnWAYD8d6Q";
    artists.push(Id::from_id(artist_id2).unwrap());
    artists.push(Id::from_id(artist_id1).unwrap());
    oauth_client()
        .await
        .user_unfollow_artists(artists)
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_user_follow_users() {
    let mut users = vec![];
    let user_id1 = Id::from_id("exampleuser01").unwrap();
    users.push(user_id1);
    oauth_client().await.user_follow_users(users).await.unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_user_unfollow_users() {
    let mut users = vec![];
    let user_id1 = Id::from_id("exampleuser01").unwrap();
    users.push(user_id1);
    oauth_client()
        .await
        .user_unfollow_users(users)
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_playlist_add_tracks() {
    let playlist_id = Id::from_id("5jAOgWXCBKuinsGiZxjDQ5").unwrap();
    let mut tracks_ids = vec![];
    let track_id1 = Id::from_uri("spotify:track:4iV5W9uYEdYUVa79Axb7Rh").unwrap();
    tracks_ids.push(track_id1);
    let track_id2 = Id::from_uri("spotify:track:1301WleyT98MSxVHPZCA6M").unwrap();
    tracks_ids.push(track_id2);
    oauth_client()
        .await
        .playlist_add_tracks(playlist_id, tracks_ids, None)
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_playlist_change_detail() {
    let playlist_id = "5jAOgWXCBKuinsGiZxjDQ5";
    let playlist_name = "A New Playlist-update";
    oauth_client()
        .await
        .playlist_change_detail(playlist_id, Some(playlist_name), Some(false), None, None)
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_playlist_check_follow() {
    let playlist_id = Id::from_id("2v3iNvBX8Ay1Gt2uXtUKUT").unwrap();
    let mut user_ids: Vec<_> = vec![];
    let user_id1 = Id::from_id("possan").unwrap();
    user_ids.push(user_id1);
    let user_id2 = Id::from_id("elogain").unwrap();
    user_ids.push(user_id2);
    oauth_client()
        .await
        .playlist_check_follow(playlist_id, &user_ids)
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_user_playlist_create() {
    let user_id = Id::from_id("2257tjys2e2u2ygfke42niy2q").unwrap();
    let playlist_name = "A New Playlist";
    oauth_client()
        .await
        .user_playlist_create(user_id, playlist_name, Some(false), None, None)
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_playlist_follow_playlist() {
    let playlist_id = Id::from_id("2v3iNvBX8Ay1Gt2uXtUKUT").unwrap();
    oauth_client()
        .await
        .playlist_follow(playlist_id, Some(true))
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_playlist_recorder_tracks() {
    let uris = Some(vec![EpisodeId::from_id("0lbiy3LKzIY2fnyjioC11p").unwrap()]);
    let playlist_id = Id::from_id("5jAOgWXCBKuinsGiZxjDQ5").unwrap();
    let range_start = 0;
    let insert_before = 1;
    let range_length = 1;
    oauth_client()
        .await
        .playlist_reorder_tracks(
            playlist_id,
            uris,
            Some(range_start),
            Some(insert_before),
            Some(range_length),
            None,
        )
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_playlist_remove_all_occurrences_of_tracks() {
    let playlist_id = Id::from_id("5jAOgWXCBKuinsGiZxjDQ5").unwrap();
    let mut tracks_ids = vec![];
    let track_id1 = Id::from_uri("spotify:track:4iV5W9uYEdYUVa79Axb7Rh").unwrap();
    let track_id2 = Id::from_uri("spotify:track:1301WleyT98MSxVHPZCA6M").unwrap();
    tracks_ids.push(track_id2);
    tracks_ids.push(track_id1);
    oauth_client()
        .await
        .playlist_remove_all_occurrences_of_tracks(playlist_id, tracks_ids, None)
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_playlist_remove_specific_occurrences_of_tracks() {
    let playlist_id = Id::from_id("5jAOgWXCBKuinsGiZxjDQ5").unwrap();
    let track_position_0_3 = TrackPositions::new(
        Id::from_uri("spotify:track:4iV5W9uYEdYUVa79Axb7Rh").unwrap(),
        vec![0, 3],
    );
    let track_position_7 = TrackPositions::new(
        Id::from_uri("spotify:track:1301WleyT98MSxVHPZCA6M").unwrap(),
        vec![7],
    );
    let tracks = vec![&track_position_0_3, &track_position_7];
    oauth_client()
        .await
        .playlist_remove_specific_occurrences_of_tracks(playlist_id, tracks, None::<&str>)
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_playlist_replace_tracks() {
    let playlist_id = Id::from_id("5jAOgWXCBKuinsGiZxjDQ5").unwrap();
    let mut tracks_ids = vec![];
    let track_id1 = Id::from_uri("spotify:track:4iV5W9uYEdYUVa79Axb7Rh").unwrap();
    let track_id2 = Id::from_uri("spotify:track:1301WleyT98MSxVHPZCA6M").unwrap();
    tracks_ids.push(track_id2);
    tracks_ids.push(track_id1);
    oauth_client()
        .await
        .playlist_replace_tracks(playlist_id, tracks_ids)
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_user_playlist() {
    let user_id = Id::from_id("spotify").unwrap();
    let playlist_id = Id::from_id("59ZbFPES4DQwEjBpWHzrtC").unwrap();
    oauth_client()
        .await
        .user_playlist(user_id, Some(playlist_id), None)
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_user_playlists() {
    let user_id = Id::from_id("2257tjys2e2u2ygfke42niy2q").unwrap();
    oauth_client()
        .await
        .user_playlists_manual(user_id, Some(10), None)
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_playlist_tracks() {
    let playlist_id = Id::from_uri("spotify:playlist:59ZbFPES4DQwEjBpWHzrtC").unwrap();
    oauth_client()
        .await
        .playlist_tracks_manual(playlist_id, None, None, Some(2), None)
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_playlist_unfollow() {
    let playlist_id = "65V6djkcVRyOStLd8nza8E";
    oauth_client()
        .await
        .playlist_unfollow(playlist_id)
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_volume() {
    oauth_client().await.volume(78, None).await.unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_add_queue() {
    let birdy_uri = TrackId::from_uri("spotify:track:6rqhFgbbKwnb9MLmUQDhG6").unwrap();
    oauth_client()
        .await
        .add_item_to_queue(birdy_uri, None)
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_get_several_shows() {
    oauth_client()
        .await
        .get_several_shows(
            vec![
                ShowId::from_id("5CfCWKI5pZ28U0uOzXkDHe").unwrap(),
                ShowId::from_id("5as3aKmN2k11yfDDDSrvaZ").unwrap(),
            ],
            None,
        )
        .await
        .unwrap();
}

#[maybe_async::test(feature = "__sync", async(feature = "__async", tokio::test))]
#[ignore]
async fn test_get_several_episodes() {
    oauth_client()
        .await
        .get_several_episodes(
            vec![
                EpisodeId::from_id("0lbiy3LKzIY2fnyjioC11p").unwrap(),
                EpisodeId::from_id("4zugY5eJisugQj9rj8TYuh").unwrap(),
            ],
            None,
        )
        .await
        .unwrap();
}
