//! Client to Spotify API endpoint

use chrono::prelude::*;
use derive_builder::Builder;
use log::error;
use maybe_async::maybe_async;
use serde::Deserialize;
use serde_json::map::Map;
use serde_json::{json, Value};
use thiserror::Error;

use std::path::PathBuf;

use super::http::{HTTPClient, Query};
use super::json_insert;
use super::model::*;
use super::oauth2::{Credentials, OAuth, Token};
use crate::model::idtypes::{IdType, PlayContextIdType};

/// Possible errors returned from the `rspotify` client.
#[derive(Debug, Error)]
pub enum ClientError {
    /// Raised when the authentication isn't configured properly.
    #[error("invalid client authentication: {0}")]
    InvalidAuth(String),

    #[error("request unauthorized")]
    Unauthorized,

    #[error("exceeded request limit")]
    RateLimited(Option<usize>),

    #[error("request error: {0}")]
    Request(String),

    #[error("status code {0}: {1}")]
    StatusCode(u16, String),

    #[error("spotify error: {0}")]
    Api(#[from] ApiError),

    #[error("json parse error: {0}")]
    ParseJson(#[from] serde_json::Error),

    #[error("url parse error: {0}")]
    ParseUrl(#[from] url::ParseError),

    #[error("input/output error: {0}")]
    Io(#[from] std::io::Error),

    #[cfg(feature = "cli")]
    #[error("cli error: {0}")]
    Cli(String),

    #[error("cache file error: {0}")]
    CacheFile(String),
}

pub type ClientResult<T> = Result<T, ClientError>;

/// Matches errors that are returned from the Spotfiy
/// API as part of the JSON response object.
#[derive(Debug, Error, Deserialize)]
pub enum ApiError {
    /// See [Error Object](https://developer.spotify.com/documentation/web-api/reference/#object-errorobject)
    #[error("{status}: {message}")]
    #[serde(alias = "error")]
    Regular { status: u16, message: String },

    /// See [Play Error Object](https://developer.spotify.com/documentation/web-api/reference/#object-playererrorobject)
    #[error("{status} ({reason}): {message}")]
    #[serde(alias = "error")]
    Player {
        status: u16,
        message: String,
        reason: String,
    },
}

pub const DEFAULT_API_PREFIX: &str = "https://api.spotify.com/v1/";
pub const DEFAULT_CACHE_PATH: &str = ".spotify_token_cache.json";

/// Spotify API object
#[derive(Builder, Debug, Clone)]
pub struct Spotify {
    /// Internal member to perform requests to the Spotify API.
    #[builder(setter(skip))]
    pub(in crate) http: HTTPClient,

    /// The access token information required for requests to the Spotify API.
    #[builder(setter(strip_option), default)]
    pub token: Option<Token>,

    /// The credentials needed for obtaining a new access token, for requests.
    /// without OAuth authentication.
    #[builder(setter(strip_option), default)]
    pub credentials: Option<Credentials>,

    /// The OAuth information required for obtaining a new access token, for
    /// requests with OAuth authentication. `credentials` also needs to be
    /// set up.
    #[builder(setter(strip_option), default)]
    pub oauth: Option<OAuth>,

    /// The Spotify API prefix, [`DEFAULT_API_PREFIX`
    /// ](DEFAULT_CACHE_PATH) by default.
    #[builder(setter(into), default = "String::from(DEFAULT_API_PREFIX)")]
    pub prefix: String,

    /// The cache file path, in case it's used. By default it's
    /// [`DEFAULT_CACHE_PATH`](DEFAULT_API_PREFIX).
    #[builder(default = r#"PathBuf::from(DEFAULT_CACHE_PATH)"#)]
    pub cache_path: PathBuf,
}

// Endpoint-related methods for the client.
impl Spotify {
    /// Returns the access token, or an error in case it's not configured.
    pub(in crate) fn get_token(&self) -> ClientResult<&Token> {
        self.token
            .as_ref()
            .ok_or_else(|| ClientError::InvalidAuth("no access token configured".to_string()))
    }

    /// Returns the credentials, or an error in case it's not configured.
    pub(in crate) fn get_creds(&self) -> ClientResult<&Credentials> {
        self.credentials
            .as_ref()
            .ok_or_else(|| ClientError::InvalidAuth("no credentials configured".to_string()))
    }

    /// Returns the oauth information, or an error in case it's not configured.
    pub(in crate) fn get_oauth(&self) -> ClientResult<&OAuth> {
        self.oauth
            .as_ref()
            .ok_or_else(|| ClientError::InvalidAuth("no oauth configured".to_string()))
    }

    /// Converts a JSON response from Spotify into its model.
    fn convert_result<'a, T: Deserialize<'a>>(&self, input: &'a str) -> ClientResult<T> {
        serde_json::from_str::<T>(input).map_err(Into::into)
    }

    /// Append device ID to an API path.
    fn append_device_id(&self, path: &str, device_id: Option<String>) -> String {
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

    /// Returns a single track given the track's ID, URI or URL.
    ///
    /// Parameters:
    /// - track_id - a spotify URI, URL or ID
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-track)
    #[maybe_async]
    pub async fn track(&self, track_id: &TrackId) -> ClientResult<FullTrack> {
        let url = format!("tracks/{}", track_id.id());
        let result = self.endpoint_get(&url, &Query::new()).await?;
        self.convert_result(&result)
    }

    /// Returns a list of tracks given a list of track IDs, URIs, or URLs.
    ///
    /// Parameters:
    /// - track_ids - a list of spotify URIs, URLs or IDs
    /// - market - an ISO 3166-1 alpha-2 country code or the string from_token.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-several-tracks)
    #[maybe_async]
    pub async fn tracks<'a>(
        &self,
        track_ids: impl IntoIterator<Item = &'a TrackId>,
        market: Option<Market>,
    ) -> ClientResult<Vec<FullTrack>> {
        let ids = join_ids(track_ids);

        let mut params = Query::new();
        if let Some(market) = market {
            params.insert("market".to_owned(), market.to_string());
        }

        let url = format!("tracks/?ids={}", ids);
        let result = self.endpoint_get(&url, &params).await?;
        self.convert_result::<FullTracks>(&result).map(|x| x.tracks)
    }

    /// Returns a single artist given the artist's ID, URI or URL.
    ///
    /// Parameters:
    /// - artist_id - an artist ID, URI or URL
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-an-artist)
    #[maybe_async]
    pub async fn artist(&self, artist_id: &ArtistId) -> ClientResult<FullArtist> {
        let url = format!("artists/{}", artist_id.id());
        let result = self.endpoint_get(&url, &Query::new()).await?;
        self.convert_result(&result)
    }

    /// Returns a list of artists given the artist IDs, URIs, or URLs.
    ///
    /// Parameters:
    /// - artist_ids - a list of artist IDs, URIs or URLs
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-multiple-artists)
    #[maybe_async]
    pub async fn artists<'a>(
        &self,
        artist_ids: impl IntoIterator<Item = &'a ArtistId>,
    ) -> ClientResult<Vec<FullArtist>> {
        let ids = join_ids(artist_ids);
        let url = format!("artists/?ids={}", ids);
        let result = self.endpoint_get(&url, &Query::new()).await?;

        self.convert_result::<FullArtists>(&result)
            .map(|x| x.artists)
    }

    /// Get Spotify catalog information about an artist's albums.
    ///
    /// Parameters:
    /// - artist_id - the artist ID, URI or URL
    /// - album_type - 'album', 'single', 'appears_on', 'compilation'
    /// - market - limit the response to one particular country.
    /// - limit  - the number of albums to return
    /// - offset - the index of the first album to return
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-an-artists-albums)
    #[maybe_async]
    pub async fn artist_albums(
        &self,
        artist_id: &ArtistId,
        album_type: Option<AlbumType>,
        market: Option<Market>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> ClientResult<Page<SimplifiedAlbum>> {
        let mut params = Query::new();
        if let Some(limit) = limit {
            params.insert("limit".to_owned(), limit.to_string());
        }
        if let Some(album_type) = album_type {
            params.insert("album_type".to_owned(), album_type.to_string());
        }
        if let Some(offset) = offset {
            params.insert("offset".to_owned(), offset.to_string());
        }
        if let Some(market) = market {
            params.insert("market".to_owned(), market.to_string());
        }
        let url = format!("artists/{}/albums", artist_id.id());
        let result = self.endpoint_get(&url, &params).await?;
        self.convert_result(&result)
    }

    /// Get Spotify catalog information about an artist's top 10 tracks by
    /// country.
    ///
    /// Parameters:
    /// - artist_id - the artist ID, URI or URL
    /// - market - limit the response to one particular country.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-an-artists-top-tracks)
    #[maybe_async]
    pub async fn artist_top_tracks(
        &self,
        artist_id: &ArtistId,
        market: Market,
    ) -> ClientResult<Vec<FullTrack>> {
        let mut params = Query::with_capacity(1);

        params.insert("market".to_owned(), market.to_string());

        let url = format!("artists/{}/top-tracks", artist_id.id());
        let result = self.endpoint_get(&url, &params).await?;
        self.convert_result::<FullTracks>(&result).map(|x| x.tracks)
    }

    /// Get Spotify catalog information about artists similar to an identified
    /// artist. Similarity is based on analysis of the Spotify community's
    /// listening history.
    ///
    /// Parameters:
    /// - artist_id - the artist ID, URI or URL
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-an-artists-related-artists)
    #[maybe_async]
    pub async fn artist_related_artists(
        &self,
        artist_id: &ArtistId,
    ) -> ClientResult<Vec<FullArtist>> {
        let url = format!("artists/{}/related-artists", artist_id.id());
        let result = self.endpoint_get(&url, &Query::new()).await?;
        self.convert_result::<FullArtists>(&result)
            .map(|x| x.artists)
    }

    /// Returns a single album given the album's ID, URIs or URL.
    ///
    /// Parameters:
    /// - album_id - the album ID, URI or URL
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-an-album)
    #[maybe_async]
    pub async fn album(&self, album_id: &AlbumId) -> ClientResult<FullAlbum> {
        let url = format!("albums/{}", album_id.id());

        let result = self.endpoint_get(&url, &Query::new()).await?;
        self.convert_result(&result)
    }

    /// Returns a list of albums given the album IDs, URIs, or URLs.
    ///
    /// Parameters:
    /// - albums_ids - a list of album IDs, URIs or URLs
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-multiple-albums)
    #[maybe_async]
    pub async fn albums<'a>(
        &self,
        album_ids: impl IntoIterator<Item = &'a AlbumId>,
    ) -> ClientResult<Vec<FullAlbum>> {
        let ids = join_ids(album_ids);
        let url = format!("albums/?ids={}", ids);
        let result = self.endpoint_get(&url, &Query::new()).await?;
        self.convert_result::<FullAlbums>(&result).map(|x| x.albums)
    }

    /// Search for an Item. Get Spotify catalog information about artists,
    /// albums, tracks or playlists that match a keyword string.
    ///
    /// Parameters:
    /// - q - the search query
    /// - limit  - the number of items to return
    /// - offset - the index of the first item to return
    /// - type - the type of item to return. One of 'artist', 'album', 'track',
    ///  'playlist', 'show' or 'episode'
    /// - market - An ISO 3166-1 alpha-2 country code or the string from_token.
    /// - include_external: Optional.Possible values: audio. If
    ///   include_external=audio is specified the response will include any
    ///   relevant audio content that is hosted externally.  
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#category-search)
    #[maybe_async]
    pub async fn search<L: Into<Option<u32>>, O: Into<Option<u32>>>(
        &self,
        q: &str,
        _type: SearchType,
        limit: L,
        offset: O,
        market: Option<Market>,
        include_external: Option<IncludeExternal>,
    ) -> ClientResult<SearchResult> {
        let mut params = Query::with_capacity(4);
        params.insert("limit".to_owned(), limit.into().unwrap_or(10).to_string());
        params.insert("offset".to_owned(), offset.into().unwrap_or(0).to_string());
        params.insert("q".to_owned(), q.to_owned());
        params.insert("type".to_owned(), _type.to_string());
        if let Some(market) = market {
            params.insert("market".to_owned(), market.to_string());
        }
        if let Some(include_external) = include_external {
            params.insert("include_external".to_owned(), include_external.to_string());
        }

        let result = self.endpoint_get("search", &params).await?;
        self.convert_result(&result)
    }

    /// Get Spotify catalog information about an album's tracks.
    ///
    /// Parameters:
    /// - album_id - the album ID, URI or URL
    /// - limit  - the number of items to return
    /// - offset - the index of the first item to return
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-an-albums-tracks)
    #[maybe_async]
    pub async fn album_track<L: Into<Option<u32>>, O: Into<Option<u32>>>(
        &self,
        album_id: &AlbumId,
        limit: L,
        offset: O,
    ) -> ClientResult<Page<SimplifiedTrack>> {
        let mut params = Query::with_capacity(2);
        params.insert("limit".to_owned(), limit.into().unwrap_or(50).to_string());
        params.insert("offset".to_owned(), offset.into().unwrap_or(0).to_string());
        let url = format!("albums/{}/tracks", album_id.id());
        let result = self.endpoint_get(&url, &params).await?;
        self.convert_result(&result)
    }

    /// Gets basic profile information about a Spotify User.
    ///
    /// Parameters:
    /// - user - the id of the usr
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-users-profile)
    #[maybe_async]
    pub async fn user(&self, user_id: &UserId) -> ClientResult<PublicUser> {
        let url = format!("users/{}", user_id.id());
        let result = self.endpoint_get(&url, &Query::new()).await?;
        self.convert_result(&result)
    }

    /// Get full details about Spotify playlist.
    ///
    /// Parameters:
    /// - playlist_id - the id of the playlist
    /// - market - an ISO 3166-1 alpha-2 country code or the string from_token.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-playlist)
    #[maybe_async]
    pub async fn playlist(
        &self,
        playlist_id: &PlaylistId,
        fields: Option<&str>,
        market: Option<Market>,
    ) -> ClientResult<FullPlaylist> {
        let mut params = Query::new();
        if let Some(fields) = fields {
            params.insert("fields".to_owned(), fields.to_owned());
        }
        if let Some(market) = market {
            params.insert("market".to_owned(), market.to_string());
        }

        let url = format!("playlists/{}", playlist_id.id());
        let result = self.endpoint_get(&url, &params).await?;
        self.convert_result(&result)
    }

    /// Get current user playlists without required getting his profile.
    ///
    /// Parameters:
    /// - limit  - the number of items to return
    /// - offset - the index of the first item to return
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-a-list-of-current-users-playlists)
    #[maybe_async]
    pub async fn current_user_playlists<L: Into<Option<u32>>, O: Into<Option<u32>>>(
        &self,
        limit: L,
        offset: O,
    ) -> ClientResult<Page<SimplifiedPlaylist>> {
        let mut params = Query::with_capacity(2);
        params.insert("limit".to_owned(), limit.into().unwrap_or(50).to_string());
        params.insert("offset".to_owned(), offset.into().unwrap_or(0).to_string());

        let result = self.endpoint_get("me/playlists", &params).await?;
        self.convert_result(&result)
    }

    /// Gets playlists of a user.
    ///
    /// Parameters:
    /// - user_id - the id of the usr
    /// - limit  - the number of items to return
    /// - offset - the index of the first item to return
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-list-users-playlists)
    #[maybe_async]
    pub async fn user_playlists<L: Into<Option<u32>>, O: Into<Option<u32>>>(
        &self,
        user_id: &UserId,
        limit: L,
        offset: O,
    ) -> ClientResult<Page<SimplifiedPlaylist>> {
        let mut params = Query::with_capacity(2);
        params.insert("limit".to_owned(), limit.into().unwrap_or(50).to_string());
        params.insert("offset".to_owned(), offset.into().unwrap_or(0).to_string());
        let url = format!("users/{}/playlists", user_id.id());
        let result = self.endpoint_get(&url, &params).await?;
        self.convert_result(&result)
    }

    /// Gets playlist of a user.
    ///
    /// Parameters:
    /// - user_id - the id of the user
    /// - playlist_id - the id of the playlist
    /// - fields - which fields to return
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-list-users-playlists)
    #[maybe_async]
    pub async fn user_playlist(
        &self,
        user_id: &UserId,
        playlist_id: Option<&PlaylistId>,
        fields: Option<&str>,
    ) -> ClientResult<FullPlaylist> {
        let mut params = Query::new();
        if let Some(fields) = fields {
            params.insert("fields".to_owned(), fields.to_string());
        }
        match playlist_id {
            Some(playlist_id) => {
                let url = format!("users/{}/playlists/{}", user_id.id(), playlist_id.id());
                let result = self.endpoint_get(&url, &params).await?;
                self.convert_result(&result)
            }
            None => {
                let url = format!("users/{}/starred", user_id.id());
                let result = self.endpoint_get(&url, &params).await?;
                self.convert_result(&result)
            }
        }
    }

    /// Get full details of the tracks of a playlist owned by a user.
    ///
    /// Parameters:
    /// - playlist_id - the id of the playlist
    /// - fields - which fields to return
    /// - limit - the maximum number of tracks to return
    /// - offset - the index of the first track to return
    /// - market - an ISO 3166-1 alpha-2 country code or the string from_token.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-playlists-tracks)
    #[maybe_async]
    pub async fn playlist_tracks<L: Into<Option<u32>>, O: Into<Option<u32>>>(
        &self,
        playlist_id: &PlaylistId,
        fields: Option<&str>,
        limit: L,
        offset: O,
        market: Option<Market>,
    ) -> ClientResult<Page<PlaylistItem>> {
        let mut params = Query::with_capacity(2);
        params.insert("limit".to_owned(), limit.into().unwrap_or(50).to_string());
        params.insert("offset".to_owned(), offset.into().unwrap_or(0).to_string());
        if let Some(market) = market {
            params.insert("market".to_owned(), market.to_string());
        }
        if let Some(fields) = fields {
            params.insert("fields".to_owned(), fields.to_owned());
        }
        let url = format!("playlists/{}/tracks", playlist_id.id());
        let result = self.endpoint_get(&url, &params).await?;
        self.convert_result(&result)
    }

    /// Creates a playlist for a user.
    ///
    /// Parameters:
    /// - user_id - the id of the user
    /// - name - the name of the playlist
    /// - public - is the created playlist public
    /// - description - the description of the playlist
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-create-playlist)
    #[maybe_async]
    pub async fn user_playlist_create<P: Into<Option<bool>>, D: Into<Option<String>>>(
        &self,
        user_id: &UserId,
        name: &str,
        public: P,
        description: D,
    ) -> ClientResult<FullPlaylist> {
        let public = public.into().unwrap_or(true);
        let description = description.into().unwrap_or_else(|| "".to_owned());
        let params = json!({
            "name": name,
            "public": public,
            "description": description
        });
        let url = format!("users/{}/playlists", user_id.id());
        let result = self.endpoint_post(&url, &params).await?;
        self.convert_result(&result)
    }

    /// Changes a playlist's name and/or public/private state.
    ///
    /// Parameters:
    /// - playlist_id - the id of the playlist
    /// - name - optional name of the playlist
    /// - public - optional is the playlist public
    /// - collaborative - optional is the playlist collaborative
    /// - description - optional description of the playlist
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-change-playlist-details)
    #[maybe_async]
    pub async fn playlist_change_detail(
        &self,
        playlist_id: &str,
        name: Option<&str>,
        public: Option<bool>,
        description: Option<String>,
        collaborative: Option<bool>,
    ) -> ClientResult<String> {
        let mut params = json!({});
        if let Some(name) = name {
            json_insert!(params, "name", name);
        }
        if let Some(public) = public {
            json_insert!(params, "public", public);
        }
        if let Some(collaborative) = collaborative {
            json_insert!(params, "collaborative", collaborative);
        }
        if let Some(description) = description {
            json_insert!(params, "description", description);
        }
        let url = format!("playlists/{}", playlist_id);
        self.endpoint_put(&url, &params).await
    }

    /// Unfollows (deletes) a playlist for a user.
    ///
    /// Parameters:
    /// - playlist_id - the id of the playlist
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-unfollow-playlist)
    #[maybe_async]
    pub async fn playlist_unfollow(&self, playlist_id: &str) -> ClientResult<String> {
        let url = format!("playlists/{}/followers", playlist_id);
        self.endpoint_delete(&url, &json!({})).await
    }

    /// Adds tracks to a playlist.
    ///
    /// Parameters:
    /// - playlist_id - the id of the playlist
    /// - track_ids - a list of track URIs, URLs or IDs
    /// - position - the position to add the tracks
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-add-tracks-to-playlist)
    #[maybe_async]
    pub async fn playlist_add_tracks<'a>(
        &self,
        playlist_id: &PlaylistId,
        track_ids: impl IntoIterator<Item = &'a TrackId>,
        position: Option<i32>,
    ) -> ClientResult<PlaylistResult> {
        let uris = track_ids.into_iter().map(|id| id.uri()).collect::<Vec<_>>();

        let mut params = json!({ "uris": uris });
        if let Some(position) = position {
            json_insert!(params, "position", position);
        }
        let url = format!("playlists/{}/tracks", playlist_id.id());
        let result = self.endpoint_post(&url, &params).await?;
        self.convert_result(&result)
    }

    /// Replace all tracks in a playlist
    ///
    /// Parameters:
    /// - user - the id of the user
    /// - playlist_id - the id of the playlist
    /// - tracks - the list of track ids to add to the playlist
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-reorder-or-replace-playlists-tracks)
    #[maybe_async]
    pub async fn playlist_replace_tracks<'a>(
        &self,
        playlist_id: &PlaylistId,
        track_ids: impl IntoIterator<Item = &'a TrackId>,
    ) -> ClientResult<()> {
        let uris = track_ids.into_iter().map(|id| id.uri()).collect::<Vec<_>>();

        let params = json!({ "uris": uris });
        let url = format!("playlists/{}/tracks", playlist_id.id());
        self.endpoint_put(&url, &params).await?;

        Ok(())
    }

    /// Reorder tracks in a playlist.
    ///
    /// Parameters:
    /// - playlist_id - the id of the playlist
    /// - range_start - the position of the first track to be reordered
    /// - range_length - optional the number of tracks to be reordered (default:
    ///   1)
    /// - insert_before - the position where the tracks should be inserted
    /// - snapshot_id - optional playlist's snapshot ID
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-reorder-or-replace-playlists-tracks)
    #[maybe_async]
    pub async fn playlist_reorder_tracks<R: Into<Option<u32>>>(
        &self,
        playlist_id: &PlaylistId,
        range_start: i32,
        range_length: R,
        insert_before: i32,
        snapshot_id: Option<String>,
    ) -> ClientResult<PlaylistResult> {
        let mut params = json! ({
            "range_start": range_start,
            "range_length": range_length.into().unwrap_or(1),
            "insert_before": insert_before
        });
        if let Some(snapshot_id) = snapshot_id {
            json_insert!(params, "snapshot_id", snapshot_id);
        }

        let url = format!("playlists/{}/tracks", playlist_id.id());
        let result = self.endpoint_put(&url, &params).await?;
        self.convert_result(&result)
    }

    /// Removes all occurrences of the given tracks from the given playlist.
    ///
    /// Parameters:
    /// - playlist_id - the id of the playlist
    /// - track_ids - the list of track ids to add to the playlist
    /// - snapshot_id - optional id of the playlist snapshot
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-remove-tracks-playlist)
    #[maybe_async]
    pub async fn playlist_remove_all_occurrences_of_tracks<'a>(
        &self,
        playlist_id: &PlaylistId,
        track_ids: impl IntoIterator<Item = &'a TrackId>,
        snapshot_id: Option<String>,
    ) -> ClientResult<PlaylistResult> {
        let tracks = track_ids
            .into_iter()
            .map(|id| {
                let mut map = Map::with_capacity(1);
                map.insert("uri".to_owned(), id.uri().into());
                map
            })
            .collect::<Vec<_>>();

        let mut params = json!({ "tracks": tracks });

        if let Some(snapshot_id) = snapshot_id {
            json_insert!(params, "snapshot_id", snapshot_id);
        }

        let url = format!("playlists/{}/tracks", playlist_id.id());
        let result = self.endpoint_delete(&url, &params).await?;
        self.convert_result(&result)
    }

    /// Removes specfic occurrences of the given tracks from the given playlist.
    ///
    /// Parameters:
    /// - playlist_id: the id of the playlist
    /// - tracks: an array of map containing Spotify URIs of the tracks to
    ///   remove with their current positions in the playlist. For example:
    ///
    /// ```json
    /// {
    ///    "tracks":[
    ///       {
    ///          "uri":"spotify:track:4iV5W9uYEdYUVa79Axb7Rh",
    ///          "positions":[
    ///             0,
    ///             3
    ///          ]
    ///       },
    ///       {
    ///          "uri":"spotify:track:1301WleyT98MSxVHPZCA6M",
    ///          "positions":[
    ///             7
    ///          ]
    ///       }
    ///    ]
    /// }
    /// ```
    /// - snapshot_id: optional id of the playlist snapshot
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-remove-tracks-playlist)
    #[maybe_async]
    pub async fn playlist_remove_specific_occurrences_of_tracks(
        &self,
        playlist_id: &PlaylistId,
        tracks: Vec<TrackPositions<'_>>,
        snapshot_id: Option<String>,
    ) -> ClientResult<PlaylistResult> {
        let ftracks = tracks
            .into_iter()
            .map(|track| {
                let mut map = Map::new();
                map.insert("uri".to_owned(), track.id.uri().into());
                map.insert("positions".to_owned(), track.positions.into());
                map
            })
            .collect::<Vec<_>>();

        let mut params = json!({ "tracks": ftracks });
        if let Some(snapshot_id) = snapshot_id {
            json_insert!(params, "snapshot_id", snapshot_id);
        }
        let url = format!("playlists/{}/tracks", playlist_id.id());
        let result = self.endpoint_delete(&url, &params).await?;
        self.convert_result(&result)
    }

    /// Add the current authenticated user as a follower of a playlist.
    ///
    /// Parameters:
    /// - playlist_id - the id of the playlist
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-follow-playlist)
    #[maybe_async]
    pub async fn playlist_follow<P: Into<Option<bool>>>(
        &self,
        playlist_id: &PlaylistId,
        public: P,
    ) -> ClientResult<()> {
        let url = format!("playlists/{}/followers", playlist_id.id());

        self.endpoint_put(
            &url,
            &json! ({
                "public": public.into().unwrap_or(true)
            }),
        )
        .await?;

        Ok(())
    }

    /// Check to see if the given users are following the given playlist.
    ///
    /// Parameters:
    /// - playlist_id - the id of the playlist
    /// - user_ids - the ids of the users that you want to
    /// check to see if they follow the playlist. Maximum: 5 ids.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-check-if-user-follows-playlist)
    #[maybe_async]
    pub async fn playlist_check_follow<'a>(
        &self,
        playlist_id: &PlaylistId,
        user_ids: &'a [&'a UserId],
    ) -> ClientResult<Vec<bool>> {
        if user_ids.len() > 5 {
            error!("The maximum length of user ids is limited to 5 :-)");
        }
        let url = format!(
            "playlists/{}/followers/contains?ids={}",
            playlist_id.id(),
            user_ids
                .iter()
                .map(|id| id.id())
                .collect::<Vec<_>>()
                .join(","),
        );
        let result = self.endpoint_get(&url, &Query::new()).await?;
        self.convert_result(&result)
    }

    /// Get detailed profile information about the current user.
    /// An alias for the 'current_user' method.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-current-users-profile)
    #[maybe_async]
    pub async fn me(&self) -> ClientResult<PrivateUser> {
        let result = self.endpoint_get("me/", &Query::new()).await?;
        self.convert_result(&result)
    }

    /// Get detailed profile information about the current user.
    /// An alias for the 'me' method.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-current-users-profile)
    #[maybe_async]
    pub async fn current_user(&self) -> ClientResult<PrivateUser> {
        self.me().await
    }

    /// Get information about the current users currently playing track.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-recently-played)
    #[maybe_async]
    pub async fn current_user_playing_track(
        &self,
    ) -> ClientResult<Option<CurrentlyPlayingContext>> {
        let result = self
            .get("me/player/currently-playing", None, &Query::new())
            .await?;
        if result.is_empty() {
            Ok(None)
        } else {
            self.convert_result(&result)
        }
    }

    /// Gets a list of the albums saved in the current authorized user's
    /// "Your Music" library
    ///
    /// Parameters:
    /// - limit - the number of albums to return
    /// - offset - the index of the first album to return
    /// - market - Provide this parameter if you want to apply Track Relinking.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-users-saved-albums)
    #[maybe_async]
    pub async fn current_user_saved_albums<L: Into<Option<u32>>, O: Into<Option<u32>>>(
        &self,
        limit: L,
        offset: O,
    ) -> ClientResult<Page<SavedAlbum>> {
        let mut params = Query::with_capacity(2);
        params.insert("limit".to_owned(), limit.into().unwrap_or(20).to_string());
        params.insert("offset".to_owned(), offset.into().unwrap_or(0).to_string());
        let result = self.endpoint_get("me/albums", &params).await?;
        self.convert_result(&result)
    }

    /// Get a list of the songs saved in the current Spotify user's "Your Music"
    /// library.
    ///
    /// Parameters:
    /// - limit - the number of tracks to return
    /// - offset - the index of the first track to return
    /// - market - Provide this parameter if you want to apply Track Relinking.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-users-saved-tracks)
    #[maybe_async]
    pub async fn current_user_saved_tracks<L: Into<Option<u32>>, O: Into<Option<u32>>>(
        &self,
        limit: L,
        offset: O,
    ) -> ClientResult<Page<SavedTrack>> {
        let mut params = Query::with_capacity(2);
        params.insert("limit".to_owned(), limit.into().unwrap_or(20).to_string());
        params.insert("offset".to_owned(), offset.into().unwrap_or(0).to_string());
        let result = self.endpoint_get("me/tracks", &params).await?;
        self.convert_result(&result)
    }

    /// Gets a list of the artists followed by the current authorized user.
    ///
    /// Parameters:
    /// - limit - the number of tracks to return
    /// - after - the last artist ID retrieved from the previous request
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-followed)
    #[maybe_async]
    pub async fn current_user_followed_artists<L: Into<Option<u32>>>(
        &self,
        limit: L,
        after: Option<String>,
    ) -> ClientResult<CursorBasedPage<FullArtist>> {
        let mut params = Query::with_capacity(2);
        params.insert("limit".to_owned(), limit.into().unwrap_or(20).to_string());
        params.insert("type".to_owned(), Type::Artist.to_string());
        if let Some(after) = after {
            params.insert("after".to_owned(), after);
        }

        let result = self.endpoint_get("me/following", &params).await?;
        self.convert_result::<CursorPageFullArtists>(&result)
            .map(|x| x.artists)
    }

    /// Remove one or more tracks from the current user's "Your Music" library.
    ///
    /// Parameters:
    /// - track_ids - a list of track URIs, URLs or IDs
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-remove-tracks-user)
    #[maybe_async]
    pub async fn current_user_saved_tracks_delete<'a>(
        &self,
        track_ids: impl IntoIterator<Item = &'a TrackId>,
    ) -> ClientResult<()> {
        let url = format!("me/tracks/?ids={}", join_ids(track_ids));
        self.endpoint_delete(&url, &json!({})).await?;

        Ok(())
    }

    /// Check if one or more tracks is already saved in the current Spotify
    /// user’s "Your Music" library.
    ///
    /// Parameters:
    /// - track_ids - a list of track URIs, URLs or IDs
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-check-users-saved-tracks)
    #[maybe_async]
    pub async fn current_user_saved_tracks_contains<'a>(
        &self,
        track_ids: impl IntoIterator<Item = &'a TrackId>,
    ) -> ClientResult<Vec<bool>> {
        let url = format!("me/tracks/contains/?ids={}", join_ids(track_ids));
        let result = self.endpoint_get(&url, &Query::new()).await?;
        self.convert_result(&result)
    }

    /// Save one or more tracks to the current user's "Your Music" library.
    ///
    /// Parameters:
    /// - track_ids - a list of track URIs, URLs or IDs
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-save-tracks-user)
    #[maybe_async]
    pub async fn current_user_saved_tracks_add<'a>(
        &self,
        track_ids: impl IntoIterator<Item = &'a TrackId>,
    ) -> ClientResult<()> {
        let url = format!("me/tracks/?ids={}", join_ids(track_ids));
        self.endpoint_put(&url, &json!({})).await?;

        Ok(())
    }

    /// Get the current user's top artists.
    ///
    /// Parameters:
    /// - limit - the number of entities to return
    /// - offset - the index of the first entity to return
    /// - time_range - Over what time frame are the affinities computed
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-users-top-artists-and-tracks)
    #[maybe_async]
    pub async fn current_user_top_artists<
        L: Into<Option<u32>>,
        O: Into<Option<u32>>,
        T: Into<Option<TimeRange>>,
    >(
        &self,
        limit: L,
        offset: O,
        time_range: T,
    ) -> ClientResult<Page<FullArtist>> {
        let mut params = Query::with_capacity(3);
        params.insert("limit".to_owned(), limit.into().unwrap_or(20).to_string());
        params.insert("offset".to_owned(), offset.into().unwrap_or(0).to_string());
        params.insert(
            "time_range".to_owned(),
            time_range
                .into()
                .unwrap_or(TimeRange::MediumTerm)
                .to_string(),
        );
        let result = self.endpoint_get(&"me/top/artists", &params).await?;
        self.convert_result(&result)
    }

    /// Get the current user's top tracks.
    ///
    /// Parameters:
    /// - limit - the number of entities to return
    /// - offset - the index of the first entity to return
    /// - time_range - Over what time frame are the affinities computed
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-users-top-artists-and-tracks)
    #[maybe_async]
    pub async fn current_user_top_tracks<
        L: Into<Option<u32>>,
        O: Into<Option<u32>>,
        T: Into<Option<TimeRange>>,
    >(
        &self,
        limit: L,
        offset: O,
        time_range: T,
    ) -> ClientResult<Page<FullTrack>> {
        let mut params = Query::with_capacity(3);
        params.insert("limit".to_owned(), limit.into().unwrap_or(20).to_string());
        params.insert("offset".to_owned(), offset.into().unwrap_or(0).to_string());
        params.insert(
            "time_range".to_owned(),
            time_range
                .into()
                .unwrap_or(TimeRange::MediumTerm)
                .to_string(),
        );
        let result = self.endpoint_get("me/top/tracks", &params).await?;
        self.convert_result(&result)
    }

    /// Get the current user's recently played tracks.
    ///
    /// Parameters:
    /// - limit - the number of entities to return
    /// - time_limit - A Unix timestamp in milliseconds.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-the-users-currently-playing-track)
    #[maybe_async]
    pub async fn current_user_recently_played<L: Into<Option<u32>>, T: Into<Option<TimeLimits>>>(
        &self,
        limit: L,
        time_limit: T,
    ) -> ClientResult<CursorBasedPage<PlayHistory>> {
        let mut params = Query::with_capacity(2);
        params.insert("limit".to_owned(), limit.into().unwrap_or(50).to_string());
        if let Some(l) = time_limit.into() {
            match l {
                TimeLimits::Before(y) => {
                    params.insert("before".to_owned(), y.to_string());
                }
                TimeLimits::After(y) => {
                    params.insert("after".to_owned(), y.to_string());
                }
            };
        }

        let result = self
            .endpoint_get("me/player/recently-played", &params)
            .await?;
        self.convert_result(&result)
    }

    /// Add one or more albums to the current user's "Your Music" library.
    ///
    /// Parameters:
    /// - album_ids - a list of album URIs, URLs or IDs
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-save-albums-user)
    #[maybe_async]
    pub async fn current_user_saved_albums_add<'a>(
        &self,
        album_ids: impl IntoIterator<Item = &'a AlbumId>,
    ) -> ClientResult<()> {
        let url = format!("me/albums/?ids={}", join_ids(album_ids));
        self.endpoint_put(&url, &json!({})).await?;

        Ok(())
    }

    /// Remove one or more albums from the current user's "Your Music" library.
    ///
    /// Parameters:
    /// - album_ids - a list of album URIs, URLs or IDs
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-remove-albums-user)
    #[maybe_async]
    pub async fn current_user_saved_albums_delete<'a>(
        &self,
        album_ids: impl IntoIterator<Item = &'a AlbumId>,
    ) -> ClientResult<()> {
        let url = format!("me/albums/?ids={}", join_ids(album_ids));
        self.endpoint_delete(&url, &json!({})).await?;

        Ok(())
    }

    /// Check if one or more albums is already saved in the current Spotify
    /// user’s "Your Music” library.
    ///
    /// Parameters:
    /// - album_ids - a list of album URIs, URLs or IDs
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-check-users-saved-albums)
    #[maybe_async]
    pub async fn current_user_saved_albums_contains<'a>(
        &self,
        album_ids: impl IntoIterator<Item = &'a AlbumId>,
    ) -> ClientResult<Vec<bool>> {
        let url = format!("me/albums/contains/?ids={}", join_ids(album_ids));
        let result = self.endpoint_get(&url, &Query::new()).await?;
        self.convert_result(&result)
    }

    /// Follow one or more artists.
    ///
    /// Parameters:
    /// - artist_ids - a list of artist IDs
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-follow-artists-users)
    #[maybe_async]
    pub async fn user_follow_artists<'a>(
        &self,
        artist_ids: impl IntoIterator<Item = &'a ArtistId>,
    ) -> ClientResult<()> {
        let url = format!("me/following?type=artist&ids={}", join_ids(artist_ids));
        self.endpoint_put(&url, &json!({})).await?;

        Ok(())
    }

    /// Unfollow one or more artists.
    ///
    /// Parameters:
    /// - artist_ids - a list of artist IDs
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-unfollow-artists-users)
    #[maybe_async]
    pub async fn user_unfollow_artists<'a>(
        &self,
        artist_ids: impl IntoIterator<Item = &'a ArtistId>,
    ) -> ClientResult<()> {
        let url = format!("me/following?type=artist&ids={}", join_ids(artist_ids));
        self.endpoint_delete(&url, &json!({})).await?;

        Ok(())
    }

    /// Check to see if the current user is following one or more artists or
    /// other Spotify users.
    ///
    /// Parameters:
    /// - artist_ids - the ids of the users that you want to
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-check-current-user-follows)
    #[maybe_async]
    pub async fn user_artist_check_follow<'a>(
        &self,
        artist_ids: impl IntoIterator<Item = &'a ArtistId>,
    ) -> ClientResult<Vec<bool>> {
        let url = format!(
            "me/following/contains?type=artist&ids={}",
            join_ids(artist_ids)
        );
        let result = self.endpoint_get(&url, &Query::new()).await?;
        self.convert_result(&result)
    }

    /// Follow one or more users.
    ///
    /// Parameters:
    /// - user_ids - a list of artist IDs
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-follow-artists-users)
    #[maybe_async]
    pub async fn user_follow_users<'a>(
        &self,
        user_ids: impl IntoIterator<Item = &'a UserId>,
    ) -> ClientResult<()> {
        let url = format!("me/following?type=user&ids={}", join_ids(user_ids));
        self.endpoint_put(&url, &json!({})).await?;

        Ok(())
    }

    /// Unfollow one or more users.
    ///
    /// Parameters:
    /// - user_ids - a list of artist IDs
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-unfollow-artists-users)
    #[maybe_async]
    pub async fn user_unfollow_users<'a>(
        &self,
        user_ids: impl IntoIterator<Item = &'a UserId>,
    ) -> ClientResult<()> {
        let url = format!("me/following?type=user&ids={}", join_ids(user_ids));
        self.endpoint_delete(&url, &json!({})).await?;

        Ok(())
    }

    /// Get a list of Spotify featured playlists.
    ///
    /// Parameters:
    /// - locale - The desired language, consisting of a lowercase ISO 639
    ///   language code and an uppercase ISO 3166-1 alpha-2 country code,
    ///   joined by an underscore.
    /// - country - An ISO 3166-1 alpha-2 country code or the string from_token.
    /// - timestamp - A timestamp in ISO 8601 format: yyyy-MM-ddTHH:mm:ss. Use
    ///   this parameter to specify the user's local time to get results
    ///   tailored for that specific date and time in the day
    /// - limit - The maximum number of items to return. Default: 20.
    ///   Minimum: 1. Maximum: 50
    /// - offset - The index of the first item to return. Default: 0
    ///   (the first object). Use with limit to get the next set of
    ///   items.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-featured-playlists)
    #[maybe_async]
    pub async fn featured_playlists<L: Into<Option<u32>>, O: Into<Option<u32>>>(
        &self,
        locale: Option<String>,
        country: Option<Market>,
        timestamp: Option<DateTime<Utc>>,
        limit: L,
        offset: O,
    ) -> ClientResult<FeaturedPlaylists> {
        let mut params = Query::with_capacity(2);
        params.insert("limit".to_owned(), limit.into().unwrap_or(20).to_string());
        params.insert("offset".to_owned(), offset.into().unwrap_or(0).to_string());
        if let Some(locale) = locale {
            params.insert("locale".to_owned(), locale);
        }
        if let Some(market) = country {
            params.insert("country".to_owned(), market.to_string());
        }
        if let Some(timestamp) = timestamp {
            params.insert("timestamp".to_owned(), timestamp.to_rfc3339());
        }
        let result = self
            .endpoint_get("browse/featured-playlists", &params)
            .await?;
        self.convert_result(&result)
    }

    /// Get a list of new album releases featured in Spotify.
    ///
    /// Parameters:
    /// - country - An ISO 3166-1 alpha-2 country code or string from_token.
    /// - limit - The maximum number of items to return. Default: 20.
    ///   Minimum: 1. Maximum: 50
    /// - offset - The index of the first item to return. Default: 0 (the first
    ///   object). Use with limit to get the next set of items.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-new-releases)
    #[maybe_async]
    pub async fn new_releases<L: Into<Option<u32>>, O: Into<Option<u32>>>(
        &self,
        country: Option<Market>,
        limit: L,
        offset: O,
    ) -> ClientResult<Page<SimplifiedAlbum>> {
        let mut params = Query::with_capacity(2);
        params.insert("limit".to_owned(), limit.into().unwrap_or(20).to_string());
        params.insert("offset".to_owned(), offset.into().unwrap_or(0).to_string());
        if let Some(market) = country {
            params.insert("country".to_owned(), market.to_string());
        }

        let result = self.endpoint_get("browse/new-releases", &params).await?;
        self.convert_result::<PageSimpliedAlbums>(&result)
            .map(|x| x.albums)
    }

    /// Get a list of new album releases featured in Spotify
    ///
    /// Parameters:
    /// - country - An ISO 3166-1 alpha-2 country code or string from_token.
    /// - locale - The desired language, consisting of an ISO 639 language code
    ///   and an ISO 3166-1 alpha-2 country code, joined by an underscore.
    /// - limit - The maximum number of items to return. Default: 20.
    ///   Minimum: 1. Maximum: 50
    /// - offset - The index of the first item to return. Default: 0 (the first
    ///   object). Use with limit to get the next set of items.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-categories)
    #[maybe_async]
    pub async fn categories<L: Into<Option<u32>>, O: Into<Option<u32>>>(
        &self,
        locale: Option<String>,
        country: Option<Market>,
        limit: L,
        offset: O,
    ) -> ClientResult<Page<Category>> {
        let mut params = Query::with_capacity(2);
        params.insert("limit".to_owned(), limit.into().unwrap_or(20).to_string());
        params.insert("offset".to_owned(), offset.into().unwrap_or(0).to_string());
        if let Some(locale) = locale {
            params.insert("locale".to_owned(), locale);
        }
        if let Some(market) = country {
            params.insert("country".to_owned(), market.to_string());
        }
        let result = self.endpoint_get("browse/categories", &params).await?;
        self.convert_result::<PageCategory>(&result)
            .map(|x| x.categories)
    }

    /// Get a list of playlists in a category in Spotify
    ///
    /// Parameters:
    /// - category_id - The category id to get playlists from.
    /// - country - An ISO 3166-1 alpha-2 country code or the string from_token.
    /// - limit - The maximum number of items to return. Default: 20.
    ///   Minimum: 1. Maximum: 50
    /// - offset - The index of the first item to return. Default: 0 (the first
    ///   object). Use with limit to get the next set of items.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-a-categories-playlists)
    #[maybe_async]
    pub async fn category_playlists<L: Into<Option<u32>>, O: Into<Option<u32>>>(
        &self,
        category_id: &str,
        country: Option<Market>,
        limit: L,
        offset: O,
    ) -> ClientResult<Page<SimplifiedPlaylist>> {
        let mut params = Query::with_capacity(2);
        params.insert("limit".to_owned(), limit.into().unwrap_or(20).to_string());
        params.insert("offset".to_owned(), offset.into().unwrap_or(0).to_string());
        if let Some(market) = country {
            params.insert("country".to_owned(), market.to_string());
        }

        let url = format!("browse/categories/{}/playlists", category_id);
        let result = self.endpoint_get(&url, &params).await?;
        self.convert_result::<CategoryPlaylists>(&result)
            .map(|x| x.playlists)
    }

    /// Get Recommendations Based on Seeds
    ///
    /// Parameters:
    /// - seed_artists - a list of artist IDs, URIs or URLs
    /// - seed_tracks - a list of artist IDs, URIs or URLs
    /// - seed_genres - a list of genre names. Available genres for
    /// - market - An ISO 3166-1 alpha-2 country code or the string from_token. If provided, all
    ///   results will be playable in this country.
    /// - limit - The maximum number of items to return. Default: 20.
    ///   Minimum: 1. Maximum: 100
    /// - min/max/target_<attribute> - For the tuneable track attributes listed
    ///   in the documentation, these values provide filters and targeting on
    ///   results.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-recommendations)
    #[maybe_async]
    pub async fn recommendations<L: Into<Option<u32>>>(
        &self,
        seed_artists: Option<Vec<&ArtistId>>,
        seed_genres: Option<Vec<String>>,
        seed_tracks: Option<Vec<&TrackId>>,
        limit: L,
        market: Option<Market>,
        payload: &Map<String, Value>,
    ) -> ClientResult<Recommendations> {
        let mut params = Query::with_capacity(payload.len() + 1);
        params.insert("limit".to_owned(), limit.into().unwrap_or(20).to_string());
        // TODO: this probably can be improved.
        let attributes = [
            "acousticness",
            "danceability",
            "duration_ms",
            "energy",
            "instrumentalness",
            "key",
            "liveness",
            "loudness",
            "mode",
            "popularity",
            "speechiness",
            "tempo",
            "time_signature",
            "valence",
        ];
        let prefixes = ["min", "max", "target"];
        for attribute in attributes.iter() {
            for prefix in prefixes.iter() {
                let param = format!("{}_{}", prefix, attribute);
                if let Some(value) = payload.get(&param) {
                    // TODO: not sure if this `to_string` is what we want. It
                    // might add quotes to the strings.
                    params.insert(param, value.to_string());
                }
            }
        }

        if let Some(seed_artists) = seed_artists {
            params.insert("seed_artists".to_owned(), join_ids(seed_artists));
        }

        if let Some(seed_genres) = seed_genres {
            params.insert("seed_genres".to_owned(), seed_genres.join(","));
        }

        if let Some(seed_tracks) = seed_tracks {
            params.insert("seed_tracks".to_owned(), join_ids(seed_tracks));
        }
        if let Some(market) = market {
            params.insert("market".to_owned(), market.to_string());
        }
        let result = self.endpoint_get("recommendations", &params).await?;
        self.convert_result(&result)
    }

    /// Get audio features for a track
    ///
    /// Parameters:
    /// - track - track URI, URL or ID
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-audio-features)
    #[maybe_async]
    pub async fn track_features(&self, track_id: &TrackId) -> ClientResult<AudioFeatures> {
        let url = format!("audio-features/{}", track_id.id());
        let result = self.endpoint_get(&url, &Query::new()).await?;
        self.convert_result(&result)
    }

    /// Get Audio Features for Several Tracks
    ///
    /// Parameters:
    /// - tracks a list of track URIs, URLs or IDs
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-several-audio-features)
    #[maybe_async]
    pub async fn tracks_features<'a>(
        &self,
        track_ids: impl IntoIterator<Item = &'a TrackId>,
    ) -> ClientResult<Option<Vec<AudioFeatures>>> {
        let url = format!("audio-features/?ids={}", join_ids(track_ids));

        let result = self.endpoint_get(&url, &Query::new()).await?;
        if result.is_empty() {
            Ok(None)
        } else {
            self.convert_result::<Option<AudioFeaturesPayload>>(&result)
                .map(|option_payload| option_payload.map(|x| x.audio_features))
        }
    }

    /// Get Audio Analysis for a Track
    ///
    /// Parameters:
    /// - track_id - a track URI, URL or ID
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-audio-analysis)
    #[maybe_async]
    pub async fn track_analysis(&self, track_id: &TrackId) -> ClientResult<AudioAnalysis> {
        let url = format!("audio-analysis/{}", track_id.id());
        let result = self.endpoint_get(&url, &Query::new()).await?;
        self.convert_result(&result)
    }

    /// Get a User’s Available Devices
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-a-users-available-devices)
    #[maybe_async]
    pub async fn device(&self) -> ClientResult<Vec<Device>> {
        let result = self
            .endpoint_get("me/player/devices", &Query::new())
            .await?;
        self.convert_result::<DevicePayload>(&result)
            .map(|x| x.devices)
    }

    /// Get Information About The User’s Current Playback
    ///
    /// Parameters:
    /// - market: Optional. an ISO 3166-1 alpha-2 country code or the string from_token.
    /// - additional_types: Optional. A comma-separated list of item types that
    ///   your client supports besides the default track type. Valid types are:
    ///   `track` and `episode`.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-information-about-the-users-current-playback)
    #[maybe_async]
    pub async fn current_playback(
        &self,
        market: Option<Market>,
        additional_types: Option<Vec<AdditionalType>>,
    ) -> ClientResult<Option<CurrentPlaybackContext>> {
        let mut params = Query::new();
        if let Some(market) = market {
            params.insert("country".to_owned(), market.to_string());
        }
        if let Some(additional_types) = additional_types {
            params.insert(
                "additional_types".to_owned(),
                additional_types
                    .iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            );
        }

        let result = self.endpoint_get("me/player", &params).await?;
        if result.is_empty() {
            Ok(None)
        } else {
            self.convert_result(&result)
        }
    }

    /// Get the User’s Currently Playing Track
    ///
    /// Parameters:
    /// - market: Optional. an ISO 3166-1 alpha-2 country code or the string from_token.
    /// - additional_types: Optional. A comma-separated list of item types that
    ///   your client supports besides the default track type. Valid types are:
    ///   `track` and `episode`.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-recently-played)
    #[maybe_async]
    pub async fn current_playing(
        &self,
        market: Option<Market>,
        additional_types: Option<Vec<AdditionalType>>,
    ) -> ClientResult<Option<CurrentlyPlayingContext>> {
        let mut params = Query::new();
        if let Some(market) = market {
            params.insert("market".to_owned(), market.to_string());
        }
        if let Some(additional_types) = additional_types {
            params.insert(
                "additional_types".to_owned(),
                additional_types
                    .iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            );
        }

        let result = self
            .get("me/player/currently-playing", None, &params)
            .await?;
        if result.is_empty() {
            Ok(None)
        } else {
            self.convert_result(&result)
        }
    }

    /// Transfer a User’s Playback.
    ///
    /// Note: Although an array is accepted, only a single device_id is
    /// currently supported. Supplying more than one will return 400 Bad Request
    ///
    /// Parameters:
    /// - device_id - transfer playback to this device
    /// - force_play - true: after transfer, play. false:
    ///   keep current state.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-transfer-a-users-playback)
    #[maybe_async]
    pub async fn transfer_playback<T: Into<Option<bool>>>(
        &self,
        device_id: &str,
        force_play: T,
    ) -> ClientResult<()> {
        self.endpoint_put(
            "me/player",
            &json! ({
                "device_ids": vec![device_id.to_owned()],
                "play": force_play.into().unwrap_or(true)
            }),
        )
        .await?;

        Ok(())
    }

    /// Start/Resume a User’s Playback.
    ///
    /// Provide a `context_uri` to start playback or a album, artist, or
    /// playlist. Provide a `uris` list to start playback of one or more tracks.
    /// Provide `offset` as {"position": <int>} or {"uri": "<track uri>"} to
    /// start playback at a particular offset.
    ///
    /// Parameters:
    /// - device_id - device target for playback
    /// - context_uri - spotify context uri to play
    /// - uris - spotify track uris
    /// - offset - offset into context by index or track
    /// - position_ms - Indicates from what position to start playback.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-start-a-users-playback)
    #[maybe_async]
    pub async fn start_context_playback<T: PlayContextIdType, U: PlayableIdType>(
        &self,
        context_uri: &Id<T>,
        device_id: Option<String>,
        offset: Option<super::model::Offset<U>>,
        position_ms: Option<std::time::Duration>,
    ) -> ClientResult<()> {
        use super::model::Offset;

        let mut params = json!({});
        json_insert!(params, "context_uri", context_uri.uri());
        if let Some(offset) = offset {
            match offset {
                Offset::Position(position) => {
                    json_insert!(params, "offset", json!({ "position": position }));
                }
                Offset::Uri(uri) => {
                    json_insert!(params, "offset", json!({ "uri": uri.uri() }));
                }
            }
        }
        if let Some(position_ms) = position_ms {
            json_insert!(params, "position_ms", position_ms);
        };
        let url = self.append_device_id("me/player/play", device_id);
        self.put(&url, None, &params).await?;

        Ok(())
    }

    #[maybe_async]
    pub async fn start_uris_playback<T: PlayableIdType>(
        &self,
        uris: &[&Id<T>],
        device_id: Option<String>,
        offset: Option<super::model::Offset<T>>,
        position_ms: Option<u32>,
    ) -> ClientResult<()> {
        use super::model::Offset;

        let mut params = json!({});
        json_insert!(
            params,
            "uris",
            uris.iter().map(|id| id.uri()).collect::<Vec<_>>()
        );
        if let Some(offset) = offset {
            match offset {
                Offset::Position(position) => {
                    json_insert!(params, "offset", json!({ "position": position }));
                }
                Offset::Uri(uri) => {
                    json_insert!(params, "offset", json!({ "uri": uri.uri() }));
                }
            }
        }
        if let Some(position_ms) = position_ms {
            json_insert!(params, "position_ms", position_ms);
        };
        let url = self.append_device_id("me/player/play", device_id);
        self.endpoint_put(&url, &params).await?;

        Ok(())
    }

    /// Pause a User’s Playback.
    ///
    /// Parameters:
    /// - device_id - device target for playback
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-pause-a-users-playback)
    #[maybe_async]
    pub async fn pause_playback(&self, device_id: Option<String>) -> ClientResult<()> {
        let url = self.append_device_id("me/player/pause", device_id);
        self.endpoint_put(&url, &json!({})).await?;

        Ok(())
    }

    /// Skip User’s Playback To Next Track.
    ///
    /// Parameters:
    /// - device_id - device target for playback
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-skip-users-playback-to-next-track)
    #[maybe_async]
    pub async fn next_track(&self, device_id: Option<String>) -> ClientResult<()> {
        let url = self.append_device_id("me/player/next", device_id);
        self.endpoint_post(&url, &json!({})).await?;

        Ok(())
    }

    /// Skip User’s Playback To Previous Track.
    ///
    /// Parameters:
    /// - device_id - device target for playback
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-skip-users-playback-to-previous-track)
    #[maybe_async]
    pub async fn previous_track(&self, device_id: Option<String>) -> ClientResult<()> {
        let url = self.append_device_id("me/player/previous", device_id);
        self.endpoint_post(&url, &json!({})).await?;

        Ok(())
    }

    /// Seek To Position In Currently Playing Track.
    ///
    /// Parameters:
    /// - position_ms - position in milliseconds to seek to
    /// - device_id - device target for playback
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-seek-to-position-in-currently-playing-track)
    #[maybe_async]
    pub async fn seek_track(
        &self,
        position_ms: u32,
        device_id: Option<String>,
    ) -> ClientResult<()> {
        let url = self.append_device_id(
            &format!("me/player/seek?position_ms={}", position_ms),
            device_id,
        );
        self.endpoint_put(&url, &json!({})).await?;

        Ok(())
    }

    /// Set Repeat Mode On User’s Playback.
    ///
    /// Parameters:
    /// - state - `track`, `context`, or `off`
    /// - device_id - device target for playback
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-set-repeat-mode-on-users-playback)
    #[maybe_async]
    pub async fn repeat(&self, state: RepeatState, device_id: Option<String>) -> ClientResult<()> {
        let url = self.append_device_id(
            &format!("me/player/repeat?state={}", state.to_string()),
            device_id,
        );
        self.endpoint_put(&url, &json!({})).await?;

        Ok(())
    }

    /// Set Volume For User’s Playback.
    ///
    /// Parameters:
    /// - volume_percent - volume between 0 and 100
    /// - device_id - device target for playback
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-set-volume-for-users-playback)
    #[maybe_async]
    pub async fn volume(&self, volume_percent: u8, device_id: Option<String>) -> ClientResult<()> {
        if volume_percent > 100u8 {
            error!("volume must be between 0 and 100, inclusive");
        }
        let url = self.append_device_id(
            &format!("me/player/volume?volume_percent={}", volume_percent),
            device_id,
        );
        self.endpoint_put(&url, &json!({})).await?;

        Ok(())
    }

    /// Toggle Shuffle For User’s Playback.
    ///
    /// Parameters:
    /// - state - true or false
    /// - device_id - device target for playback
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-toggle-shuffle-for-users-playback)
    #[maybe_async]
    pub async fn shuffle(&self, state: bool, device_id: Option<String>) -> ClientResult<()> {
        let url = self.append_device_id(&format!("me/player/shuffle?state={}", state), device_id);
        self.endpoint_put(&url, &json!({})).await?;

        Ok(())
    }

    /// Add an item to the end of the user's playback queue.
    ///
    /// Parameters:
    /// - uri - The uri of the item to add, Track or Episode
    /// - device id - The id of the device targeting
    /// - If no device ID provided the user's currently active device is
    ///   targeted
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-add-to-queue)
    #[maybe_async]
    pub async fn add_item_to_queue<T: PlayableIdType>(
        &self,
        item: &Id<T>,
        device_id: Option<String>,
    ) -> ClientResult<()> {
        let url = self.append_device_id(&format!("me/player/queue?uri={}", item), device_id);
        self.endpoint_post(&url, &json!({})).await?;

        Ok(())
    }

    /// Add a show or a list of shows to a user’s library.
    ///
    /// Parameters:
    /// - ids(Required) A comma-separated list of Spotify IDs for the shows to
    ///   be added to the user’s library.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-save-shows-user)
    #[maybe_async]
    pub async fn save_shows<'a>(
        &self,
        show_ids: impl IntoIterator<Item = &'a ShowId>,
    ) -> ClientResult<()> {
        let url = format!("me/shows/?ids={}", join_ids(show_ids));
        self.endpoint_put(&url, &json!({})).await?;

        Ok(())
    }

    /// Get a list of shows saved in the current Spotify user’s library.
    /// Optional parameters can be used to limit the number of shows returned.
    ///
    /// Parameters:
    /// - limit(Optional). The maximum number of shows to return. Default: 20.
    ///   Minimum: 1. Maximum: 50.
    /// - offset(Optional). The index of the first show to return. Default: 0
    ///   (the first object). Use with limit to get the next set of shows.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-users-saved-shows)
    #[maybe_async]
    pub async fn get_saved_show<L: Into<Option<u32>>, O: Into<Option<u32>>>(
        &self,
        limit: L,
        offset: O,
    ) -> ClientResult<Page<Show>> {
        let mut params = Query::with_capacity(2);
        params.insert("limit".to_owned(), limit.into().unwrap_or(20).to_string());
        params.insert("offset".to_owned(), offset.into().unwrap_or(0).to_string());
        let result = self.endpoint_get("me/shows", &params).await?;
        self.convert_result(&result)
    }

    /// Get Spotify catalog information for a single show identified by its unique Spotify ID.
    ///
    /// Path Parameters:
    /// - id: The Spotify ID for the show.
    ///
    /// Query Parameters
    /// - market(Optional): An ISO 3166-1 alpha-2 country code or the string from_token.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-a-show)
    #[maybe_async]
    pub async fn get_a_show(&self, id: &ShowId, market: Option<Market>) -> ClientResult<FullShow> {
        let mut params = Query::new();
        if let Some(market) = market {
            params.insert("market".to_owned(), market.to_string());
        }
        let url = format!("shows/{}", id.id());
        let result = self.endpoint_get(&url, &params).await?;
        self.convert_result(&result)
    }

    /// Get Spotify catalog information for multiple shows based on their
    /// Spotify IDs.
    ///
    /// Query Parameters
    /// - ids(Required) A comma-separated list of the Spotify IDs for the shows. Maximum: 50 IDs.
    /// - market(Optional) An ISO 3166-1 alpha-2 country code or the string from_token.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-multiple-shows)
    #[maybe_async]
    pub async fn get_several_shows<'a>(
        &self,
        ids: impl IntoIterator<Item = &'a ShowId>,
        market: Option<Market>,
    ) -> ClientResult<Vec<SimplifiedShow>> {
        let mut params = Query::with_capacity(1);
        params.insert("ids".to_owned(), join_ids(ids));
        if let Some(market) = market {
            params.insert("market".to_owned(), market.to_string());
        }
        let result = self.endpoint_get("shows", &params).await?;
        self.convert_result::<SeversalSimplifiedShows>(&result)
            .map(|x| x.shows)
    }

    /// Get Spotify catalog information about an show’s episodes. Optional
    /// parameters can be used to limit the number of episodes returned.
    ///
    /// Path Parameters
    /// - id: The Spotify ID for the show.
    ///
    /// Query Parameters
    /// - limit: Optional. The maximum number of episodes to return. Default: 20. Minimum: 1. Maximum: 50.
    /// - offset: Optional. The index of the first episode to return. Default: 0 (the first object). Use with limit to get the next set of episodes.
    /// - market: Optional. An ISO 3166-1 alpha-2 country code or the string from_token.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-a-shows-episodes)
    #[maybe_async]
    pub async fn get_shows_episodes<L: Into<Option<u32>>, O: Into<Option<u32>>>(
        &self,
        id: &ShowId,
        limit: L,
        offset: O,
        market: Option<Market>,
    ) -> ClientResult<Page<SimplifiedEpisode>> {
        let mut params = Query::with_capacity(2);
        params.insert("limit".to_owned(), limit.into().unwrap_or(20).to_string());
        params.insert("offset".to_owned(), offset.into().unwrap_or(0).to_string());
        if let Some(market) = market {
            params.insert("market".to_owned(), market.to_string());
        }
        let url = format!("shows/{}/episodes", id.id());
        let result = self.endpoint_get(&url, &params).await?;
        self.convert_result(&result)
    }

    /// Get Spotify catalog information for a single episode identified by its unique Spotify ID.
    ///
    /// Path Parameters
    /// - id: The Spotify ID for the episode.
    ///
    /// Query Parameters
    /// - market: Optional. An ISO 3166-1 alpha-2 country code or the string from_token.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-an-episode)
    #[maybe_async]
    pub async fn get_an_episode(
        &self,
        id: &EpisodeId,
        market: Option<Market>,
    ) -> ClientResult<FullEpisode> {
        let url = format!("episodes/{}", id.id());
        let mut params = Query::new();
        if let Some(market) = market {
            params.insert("market".to_owned(), market.to_string());
        }

        let result = self.endpoint_get(&url, &params).await?;
        self.convert_result(&result)
    }

    /// Get Spotify catalog information for multiple episodes based on their Spotify IDs.
    ///
    /// Query Parameters
    /// - ids: Required. A comma-separated list of the Spotify IDs for the episodes. Maximum: 50 IDs.
    /// - market: Optional. An ISO 3166-1 alpha-2 country code or the string from_token.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-multiple-episodes)
    #[maybe_async]
    pub async fn get_several_episodes<'a>(
        &self,
        ids: impl IntoIterator<Item = &'a EpisodeId>,
        market: Option<Market>,
    ) -> ClientResult<Vec<FullEpisode>> {
        let mut params = Query::with_capacity(1);
        params.insert("ids".to_owned(), join_ids(ids));
        if let Some(market) = market {
            params.insert("market".to_owned(), market.to_string());
        }

        let result = self.endpoint_get("episodes", &params).await?;
        self.convert_result::<EpisodesPayload>(&result)
            .map(|x| x.episodes)
    }

    /// Check if one or more shows is already saved in the current Spotify user’s library.
    ///
    /// Query Parameters
    /// - ids: Required. A comma-separated list of the Spotify IDs for the shows. Maximum: 50 IDs.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-check-users-saved-shows)
    #[maybe_async]
    pub async fn check_users_saved_shows<'a>(
        &self,
        ids: impl IntoIterator<Item = &'a ShowId>,
    ) -> ClientResult<Vec<bool>> {
        let mut params = Query::with_capacity(1);
        params.insert("ids".to_owned(), join_ids(ids));
        let result = self.endpoint_get("me/shows/contains", &params).await?;
        self.convert_result(&result)
    }

    /// Delete one or more shows from current Spotify user's library.
    /// Changes to a user's saved shows may not be visible in other Spotify applications immediately.
    ///
    /// Query Parameters
    /// - ids: Required. A comma-separated list of Spotify IDs for the shows to be deleted from the user’s library.
    /// - market: Optional. An ISO 3166-1 alpha-2 country code or the string from_token.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-remove-shows-user)
    #[maybe_async]
    pub async fn remove_users_saved_shows<'a>(
        &self,
        show_ids: impl IntoIterator<Item = &'a ShowId>,
        market: Option<Market>,
    ) -> ClientResult<()> {
        let url = format!("me/shows?ids={}", join_ids(show_ids));
        let mut params = json!({});
        if let Some(market) = market {
            json_insert!(params, "country", market.to_string());
        }
        self.endpoint_delete(&url, &params).await?;

        Ok(())
    }
}

#[inline]
fn join_ids<'a, T: 'a + IdType>(ids: impl IntoIterator<Item = &'a Id<T>>) -> String {
    ids.into_iter().collect::<Vec<_>>().join(",")
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
        let device_id = Some("fdafdsadfa".to_owned());
        let spotify = SpotifyBuilder::default().build().unwrap();
        let new_path = spotify.append_device_id(path, device_id);
        assert_eq!(new_path, "me/player/play?device_id=fdafdsadfa");
    }

    #[test]
    fn test_append_device_id_with_question_mark() {
        let path = "me/player/shuffle?state=true";
        let device_id = Some("fdafdsadfa".to_owned());
        let spotify = SpotifyBuilder::default().build().unwrap();
        let new_path = spotify.append_device_id(path, device_id);
        assert_eq!(
            new_path,
            "me/player/shuffle?state=true&device_id=fdafdsadfa"
        );
    }
}
