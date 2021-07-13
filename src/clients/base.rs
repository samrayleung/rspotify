use crate::{
    auth_urls,
    clients::{
        basic_auth, bearer_auth, convert_result, join_ids,
        pagination::{paginate, Paginator},
    },
    http::{BaseHttpClient, Form, Headers, Query},
    macros::build_map,
    model::*,
    ClientResult, Config, Credentials, Token,
};

use std::{collections::HashMap, fmt};

use chrono::Utc;
use maybe_async::maybe_async;
use serde_json::{Map, Value};

/// This trait implements the basic endpoints from the Spotify API that may be
/// accessed without user authorization, including parts of the authentication
/// flow that are shared, and the endpoints.
#[maybe_async(?Send)]
pub trait BaseClient<Http>
where
    Self: Default + Clone + fmt::Debug,
    Http: BaseHttpClient,
{
    fn get_config(&self) -> &Config;
    fn get_http(&self) -> &Http;
    fn get_token(&self) -> Option<&Token>;
    fn get_token_mut(&mut self) -> Option<&mut Token>;
    fn get_creds(&self) -> &Credentials;

    /// If it's a relative URL like "me", the prefix is appended to it.
    /// Otherwise, the same URL is returned.
    fn endpoint_url(&self, url: &str) -> String {
        // Using the client's prefix in case it's a relative route.
        if !url.starts_with("http") {
            self.get_config().prefix.clone() + url
        } else {
            url.to_string()
        }
    }

    /// The headers required for authenticated requests to the API
    fn auth_headers(&self) -> ClientResult<Headers> {
        let mut auth = Headers::new();
        let (key, val) = bearer_auth(self.get_token().expect("Rspotify not authenticated"));
        auth.insert(key, val);

        Ok(auth)
    }

    // HTTP-related methods for the Spotify client. It wraps the basic HTTP
    // client with features needed of higher level.
    //
    // The Spotify client has two different wrappers to perform requests:
    //
    // * Basic wrappers: `get`, `post`, `put`, `delete`, `post_form`. These only
    //   append the configured Spotify API URL to the relative URL provided so
    //   that it's not forgotten. They're used in the authentication process to
    //   request an access token and similars.
    // * Endpoint wrappers: `endpoint_get`, `endpoint_post`, `endpoint_put`,
    //   `endpoint_delete`. These append the authentication headers for endpoint
    //   requests to reduce the code needed for endpoints and make them as
    //   concise as possible.

    #[inline]
    async fn get(
        &self,
        url: &str,
        headers: Option<&Headers>,
        payload: &Query<'_>,
    ) -> ClientResult<String> {
        let url = self.endpoint_url(url);
        Ok(self.get_http().get(&url, headers, payload).await?)
    }

    #[inline]
    async fn post(
        &self,
        url: &str,
        headers: Option<&Headers>,
        payload: &Value,
    ) -> ClientResult<String> {
        let url = self.endpoint_url(url);
        Ok(self.get_http().post(&url, headers, payload).await?)
    }

    #[inline]
    async fn post_form(
        &self,
        url: &str,
        headers: Option<&Headers>,
        payload: &Form<'_>,
    ) -> ClientResult<String> {
        let url = self.endpoint_url(url);
        Ok(self.get_http().post_form(&url, headers, payload).await?)
    }

    #[inline]
    async fn put(
        &self,
        url: &str,
        headers: Option<&Headers>,
        payload: &Value,
    ) -> ClientResult<String> {
        let url = self.endpoint_url(url);
        Ok(self.get_http().put(&url, headers, payload).await?)
    }

    #[inline]
    async fn delete(
        &self,
        url: &str,
        headers: Option<&Headers>,
        payload: &Value,
    ) -> ClientResult<String> {
        let url = self.endpoint_url(url);
        Ok(self.get_http().delete(&url, headers, payload).await?)
    }

    /// The wrapper for the endpoints, which also includes the required
    /// autentication.
    #[inline]
    async fn endpoint_get(&self, url: &str, payload: &Query<'_>) -> ClientResult<String> {
        let headers = self.auth_headers()?;
        self.get(url, Some(&headers), payload).await
    }

    #[inline]
    async fn endpoint_post(&self, url: &str, payload: &Value) -> ClientResult<String> {
        let headers = self.auth_headers()?;
        self.post(url, Some(&headers), payload).await
    }

    #[inline]
    async fn endpoint_put(&self, url: &str, payload: &Value) -> ClientResult<String> {
        let headers = self.auth_headers()?;
        self.put(url, Some(&headers), payload).await
    }

    #[inline]
    async fn endpoint_delete(&self, url: &str, payload: &Value) -> ClientResult<String> {
        let headers = self.auth_headers()?;
        self.delete(url, Some(&headers), payload).await
    }

    /// Updates the cache file at the internal cache path.
    ///
    /// This should be used whenever it's possible to, even if the cached token
    /// isn't configured, because this will already check `Config::token_cached`
    /// and do nothing in that case already.
    fn write_token_cache(&self) -> ClientResult<()> {
        if !self.get_config().token_cached {
            return Ok(());
        }

        if let Some(tok) = self.get_token().as_ref() {
            tok.write_cache(&self.get_config().cache_path)?;
        }

        Ok(())
    }

    /// Sends a request to Spotify for an access token.
    async fn fetch_access_token(&self, payload: &Form<'_>) -> ClientResult<Token> {
        // This request uses a specific content type, and the client ID/secret
        // as the authentication, since the access token isn't available yet.
        let mut head = Headers::new();
        let (key, val) = basic_auth(&self.get_creds().id, &self.get_creds().secret);
        head.insert(key, val);

        let response = self
            .post_form(auth_urls::TOKEN, Some(&head), payload)
            .await?;
        let mut tok = serde_json::from_str::<Token>(&response)?;
        tok.expires_at = Utc::now().checked_add_signed(tok.expires_in);
        Ok(tok)
    }

    /// Returns a single track given the track's ID, URI or URL.
    ///
    /// Parameters:
    /// - track_id - a spotify URI, URL or ID
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-track)
    async fn track(&self, track_id: &TrackId) -> ClientResult<FullTrack> {
        let url = format!("tracks/{}", track_id.id());
        let result = self.endpoint_get(&url, &Query::new()).await?;
        convert_result(&result)
    }

    /// Returns a list of tracks given a list of track IDs, URIs, or URLs.
    ///
    /// Parameters:
    /// - track_ids - a list of spotify URIs, URLs or IDs
    /// - market - an ISO 3166-1 alpha-2 country code or the string from_token.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-several-tracks)
    async fn tracks<'a>(
        &self,
        track_ids: impl IntoIterator<Item = &'a TrackId> + 'a,
        market: Option<&Market>,
    ) -> ClientResult<Vec<FullTrack>> {
        let ids = join_ids(track_ids);
        let params = build_map! {
            optional "market": market.map(|x| x.as_ref()),
        };

        let url = format!("tracks/?ids={}", ids);
        let result = self.endpoint_get(&url, &params).await?;
        convert_result::<FullTracks>(&result).map(|x| x.tracks)
    }

    /// Returns a single artist given the artist's ID, URI or URL.
    ///
    /// Parameters:
    /// - artist_id - an artist ID, URI or URL
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-an-artist)
    async fn artist(&self, artist_id: &ArtistId) -> ClientResult<FullArtist> {
        let url = format!("artists/{}", artist_id.id());
        let result = self.endpoint_get(&url, &Query::new()).await?;
        convert_result(&result)
    }

    /// Returns a list of artists given the artist IDs, URIs, or URLs.
    ///
    /// Parameters:
    /// - artist_ids - a list of artist IDs, URIs or URLs
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-multiple-artists)
    async fn artists<'a, Artists: IntoIterator<Item = &'a ArtistId>>(
        &self,
        artist_ids: Artists,
    ) -> ClientResult<Vec<FullArtist>> {
        let ids = join_ids(artist_ids);
        let url = format!("artists/?ids={}", ids);
        let result = self.endpoint_get(&url, &Query::new()).await?;

        convert_result::<FullArtists>(&result).map(|x| x.artists)
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
    /// See [`Self::artist_albums_manual`] for a manually paginated version of
    /// this.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-an-artists-albums)
    fn artist_albums<'a>(
        &'a self,
        artist_id: &'a ArtistId,
        album_type: Option<&'a AlbumType>,
        market: Option<&'a Market>,
    ) -> Paginator<'_, ClientResult<SimplifiedAlbum>> {
        paginate(
            move |limit, offset| {
                self.artist_albums_manual(artist_id, album_type, market, Some(limit), Some(offset))
            },
            self.get_config().pagination_chunks,
        )
    }

    /// The manually paginated version of [`Self::artist_albums`].
    async fn artist_albums_manual(
        &self,
        artist_id: &ArtistId,
        album_type: Option<&AlbumType>,
        market: Option<&Market>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> ClientResult<Page<SimplifiedAlbum>> {
        let limit = limit.map(|x| x.to_string());
        let offset = offset.map(|x| x.to_string());
        let params = build_map! {
            optional "album_type": album_type.map(|x| x.as_ref()),
            optional "market": market.map(|x| x.as_ref()),
            optional "limit": limit.as_deref(),
            optional "offset": offset.as_deref(),
        };

        let url = format!("artists/{}/albums", artist_id.id());
        let result = self.endpoint_get(&url, &params).await?;
        convert_result(&result)
    }

    /// Get Spotify catalog information about an artist's top 10 tracks by
    /// country.
    ///
    /// Parameters:
    /// - artist_id - the artist ID, URI or URL
    /// - market - limit the response to one particular country.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-an-artists-top-tracks)
    async fn artist_top_tracks(
        &self,
        artist_id: &ArtistId,
        market: &Market,
    ) -> ClientResult<Vec<FullTrack>> {
        let params = build_map! {
            "market": market.as_ref()
        };

        let url = format!("artists/{}/top-tracks", artist_id.id());
        let result = self.endpoint_get(&url, &params).await?;
        convert_result::<FullTracks>(&result).map(|x| x.tracks)
    }

    /// Get Spotify catalog information about artists similar to an identified
    /// artist. Similarity is based on analysis of the Spotify community's
    /// listening history.
    ///
    /// Parameters:
    /// - artist_id - the artist ID, URI or URL
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-an-artists-related-artists)
    async fn artist_related_artists(&self, artist_id: &ArtistId) -> ClientResult<Vec<FullArtist>> {
        let url = format!("artists/{}/related-artists", artist_id.id());
        let result = self.endpoint_get(&url, &Query::new()).await?;
        convert_result::<FullArtists>(&result).map(|x| x.artists)
    }

    /// Returns a single album given the album's ID, URIs or URL.
    ///
    /// Parameters:
    /// - album_id - the album ID, URI or URL
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-an-album)
    async fn album(&self, album_id: &AlbumId) -> ClientResult<FullAlbum> {
        let url = format!("albums/{}", album_id.id());

        let result = self.endpoint_get(&url, &Query::new()).await?;
        convert_result(&result)
    }

    /// Returns a list of albums given the album IDs, URIs, or URLs.
    ///
    /// Parameters:
    /// - albums_ids - a list of album IDs, URIs or URLs
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-multiple-albums)
    async fn albums<'a, Albums: IntoIterator<Item = &'a AlbumId>>(
        &self,
        album_ids: Albums,
    ) -> ClientResult<Vec<FullAlbum>> {
        let ids = join_ids(album_ids);
        let url = format!("albums/?ids={}", ids);
        let result = self.endpoint_get(&url, &Query::new()).await?;
        convert_result::<FullAlbums>(&result).map(|x| x.albums)
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
    async fn search(
        &self,
        q: &str,
        _type: &SearchType,
        market: Option<&Market>,
        include_external: Option<&IncludeExternal>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> ClientResult<SearchResult> {
        let limit = limit.map(|s| s.to_string());
        let offset = offset.map(|s| s.to_string());
        let params = build_map! {
            "q": q,
            "type": _type.as_ref(),
            optional "market": market.map(|x| x.as_ref()),
            optional "include_external": include_external.map(|x| x.as_ref()),
            optional "limit": limit.as_deref(),
            optional "offset": offset.as_deref(),
        };

        let result = self.endpoint_get("search", &params).await?;
        convert_result(&result)
    }

    /// Get Spotify catalog information about an album's tracks.
    ///
    /// Parameters:
    /// - album_id - the album ID, URI or URL
    /// - limit  - the number of items to return
    /// - offset - the index of the first item to return
    ///
    /// See [`Self::album_track_manual`] for a manually paginated version of
    /// this.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-an-albums-tracks)
    fn album_track<'a>(
        &'a self,
        album_id: &'a AlbumId,
    ) -> Paginator<'_, ClientResult<SimplifiedTrack>> {
        paginate(
            move |limit, offset| self.album_track_manual(album_id, Some(limit), Some(offset)),
            self.get_config().pagination_chunks,
        )
    }

    /// The manually paginated version of [`Self::album_track`].
    async fn album_track_manual(
        &self,
        album_id: &AlbumId,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> ClientResult<Page<SimplifiedTrack>> {
        let limit = limit.map(|s| s.to_string());
        let offset = offset.map(|s| s.to_string());
        let params = build_map! {
            optional "limit": limit.as_deref(),
            optional "offset": offset.as_deref(),
        };

        let url = format!("albums/{}/tracks", album_id.id());
        let result = self.endpoint_get(&url, &params).await?;
        convert_result(&result)
    }

    /// Gets basic profile information about a Spotify User.
    ///
    /// Parameters:
    /// - user - the id of the usr
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-users-profile)
    async fn user(&self, user_id: &UserId) -> ClientResult<PublicUser> {
        let url = format!("users/{}", user_id.id());
        let result = self.endpoint_get(&url, &Query::new()).await?;
        convert_result(&result)
    }

    /// Get full details about Spotify playlist.
    ///
    /// Parameters:
    /// - playlist_id - the id of the playlist
    /// - market - an ISO 3166-1 alpha-2 country code or the string from_token.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-playlist)
    async fn playlist(
        &self,
        playlist_id: &PlaylistId,
        fields: Option<&str>,
        market: Option<&Market>,
    ) -> ClientResult<FullPlaylist> {
        let params = build_map! {
            optional "fields": fields,
            optional "market": market.map(|x| x.as_ref()),
        };

        let url = format!("playlists/{}", playlist_id.id());
        let result = self.endpoint_get(&url, &params).await?;
        convert_result(&result)
    }

    /// Gets playlist of a user.
    ///
    /// Parameters:
    /// - user_id - the id of the user
    /// - playlist_id - the id of the playlist
    /// - fields - which fields to return
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-list-users-playlists)
    async fn user_playlist(
        &self,
        user_id: &UserId,
        playlist_id: Option<&PlaylistId>,
        fields: Option<&str>,
    ) -> ClientResult<FullPlaylist> {
        let params = build_map! {
            optional "fields": fields,
        };

        let url = match playlist_id {
            Some(playlist_id) => format!("users/{}/playlists/{}", user_id.id(), playlist_id.id()),
            None => format!("users/{}/starred", user_id.id()),
        };
        let result = self.endpoint_get(&url, &params).await?;
        convert_result(&result)
    }

    /// Check to see if the given users are following the given playlist.
    ///
    /// Parameters:
    /// - playlist_id - the id of the playlist
    /// - user_ids - the ids of the users that you want to
    /// check to see if they follow the playlist. Maximum: 5 ids.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-check-if-user-follows-playlist)
    async fn playlist_check_follow(
        &self,
        playlist_id: &PlaylistId,
        user_ids: &[&UserId],
    ) -> ClientResult<Vec<bool>> {
        if user_ids.len() > 5 {
            log::error!("The maximum length of user ids is limited to 5 :-)");
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
        convert_result(&result)
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
    async fn get_a_show(&self, id: &ShowId, market: Option<&Market>) -> ClientResult<FullShow> {
        let params = build_map! {
            optional "market": market.map(|x| x.as_ref()),
        };

        let url = format!("shows/{}", id.id());
        let result = self.endpoint_get(&url, &params).await?;
        convert_result(&result)
    }

    /// Get Spotify catalog information for multiple shows based on their
    /// Spotify IDs.
    ///
    /// Query Parameters
    /// - ids(Required) A comma-separated list of the Spotify IDs for the shows. Maximum: 50 IDs.
    /// - market(Optional) An ISO 3166-1 alpha-2 country code or the string from_token.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-multiple-shows)
    async fn get_several_shows<'a>(
        &self,
        ids: impl IntoIterator<Item = &'a ShowId> + 'a,
        market: Option<&Market>,
    ) -> ClientResult<Vec<SimplifiedShow>> {
        let ids = join_ids(ids);
        let params = build_map! {
            "ids": &ids,
            optional "market": market.map(|x| x.as_ref()),
        };

        let result = self.endpoint_get("shows", &params).await?;
        convert_result::<SeversalSimplifiedShows>(&result).map(|x| x.shows)
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
    /// See [`Self::get_shows_episodes_manual`] for a manually paginated version
    /// of this.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-a-shows-episodes)
    fn get_shows_episodes<'a>(
        &'a self,
        id: &'a ShowId,
        market: Option<&'a Market>,
    ) -> Paginator<'_, ClientResult<SimplifiedEpisode>> {
        paginate(
            move |limit, offset| {
                self.get_shows_episodes_manual(id, market, Some(limit), Some(offset))
            },
            self.get_config().pagination_chunks,
        )
    }

    /// The manually paginated version of [`Self::get_shows_episodes`].
    async fn get_shows_episodes_manual(
        &self,
        id: &ShowId,
        market: Option<&Market>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> ClientResult<Page<SimplifiedEpisode>> {
        let limit = limit.map(|x| x.to_string());
        let offset = offset.map(|x| x.to_string());
        let params = build_map! {
            optional "market": market.map(|x| x.as_ref()),
            optional "limit": limit.as_ref(),
            optional "offset": offset.as_ref(),
        };

        let url = format!("shows/{}/episodes", id.id());
        let result = self.endpoint_get(&url, &params).await?;
        convert_result(&result)
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
    async fn get_an_episode(
        &self,
        id: &EpisodeId,
        market: Option<&Market>,
    ) -> ClientResult<FullEpisode> {
        let url = format!("episodes/{}", id.id());
        let params = build_map! {
            optional "market": market.map(|x| x.as_ref()),
        };

        let result = self.endpoint_get(&url, &params).await?;
        convert_result(&result)
    }

    /// Get Spotify catalog information for multiple episodes based on their Spotify IDs.
    ///
    /// Query Parameters
    /// - ids: Required. A comma-separated list of the Spotify IDs for the episodes. Maximum: 50 IDs.
    /// - market: Optional. An ISO 3166-1 alpha-2 country code or the string from_token.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-multiple-episodes)
    async fn get_several_episodes<'a>(
        &self,
        ids: impl IntoIterator<Item = &'a EpisodeId> + 'a,
        market: Option<&Market>,
    ) -> ClientResult<Vec<FullEpisode>> {
        let ids = join_ids(ids);
        let params = build_map! {
            "ids": &ids,
            optional "market": market.map(|x| x.as_ref()),
        };

        let result = self.endpoint_get("episodes", &params).await?;
        convert_result::<EpisodesPayload>(&result).map(|x| x.episodes)
    }

    /// Get audio features for a track
    ///
    /// Parameters:
    /// - track - track URI, URL or ID
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-audio-features)
    async fn track_features(&self, track_id: &TrackId) -> ClientResult<AudioFeatures> {
        let url = format!("audio-features/{}", track_id.id());
        let result = self.endpoint_get(&url, &Query::new()).await?;
        convert_result(&result)
    }

    /// Get Audio Features for Several Tracks
    ///
    /// Parameters:
    /// - tracks a list of track URIs, URLs or IDs
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-several-audio-features)
    async fn tracks_features<'a>(
        &self,
        track_ids: impl IntoIterator<Item = &'a TrackId> + 'a,
    ) -> ClientResult<Option<Vec<AudioFeatures>>> {
        let url = format!("audio-features/?ids={}", join_ids(track_ids));

        let result = self.endpoint_get(&url, &Query::new()).await?;
        if result.is_empty() {
            Ok(None)
        } else {
            convert_result::<Option<AudioFeaturesPayload>>(&result)
                .map(|option_payload| option_payload.map(|x| x.audio_features))
        }
    }

    /// Get Audio Analysis for a Track
    ///
    /// Parameters:
    /// - track_id - a track URI, URL or ID
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-audio-analysis)
    async fn track_analysis(&self, track_id: &TrackId) -> ClientResult<AudioAnalysis> {
        let url = format!("audio-analysis/{}", track_id.id());
        let result = self.endpoint_get(&url, &Query::new()).await?;
        convert_result(&result)
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
    /// See [`Self::categories_manual`] for a manually paginated version of
    /// this.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-categories)
    fn categories<'a>(
        &'a self,
        locale: Option<&'a str>,
        country: Option<&'a Market>,
    ) -> Paginator<'_, ClientResult<Category>> {
        paginate(
            move |limit, offset| self.categories_manual(locale, country, Some(limit), Some(offset)),
            self.get_config().pagination_chunks,
        )
    }

    /// The manually paginated version of [`Self::categories`].
    async fn categories_manual(
        &self,
        locale: Option<&str>,
        country: Option<&Market>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> ClientResult<Page<Category>> {
        let limit = limit.map(|x| x.to_string());
        let offset = offset.map(|x| x.to_string());
        let params = build_map! {
            optional "locale": locale,
            optional "country": country.map(|x| x.as_ref()),
            optional "limit": limit.as_deref(),
            optional "offset": offset.as_deref(),
        };
        let result = self.endpoint_get("browse/categories", &params).await?;
        convert_result::<PageCategory>(&result).map(|x| x.categories)
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
    /// See [`Self::category_playlists_manual`] for a manually paginated version
    /// of this.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-a-categories-playlists)
    fn category_playlists<'a>(
        &'a self,
        category_id: &'a str,
        country: Option<&'a Market>,
    ) -> Paginator<'_, ClientResult<SimplifiedPlaylist>> {
        paginate(
            move |limit, offset| {
                self.category_playlists_manual(category_id, country, Some(limit), Some(offset))
            },
            self.get_config().pagination_chunks,
        )
    }

    /// The manually paginated version of [`Self::category_playlists`].
    async fn category_playlists_manual(
        &self,
        category_id: &str,
        country: Option<&Market>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> ClientResult<Page<SimplifiedPlaylist>> {
        let limit = limit.map(|x| x.to_string());
        let offset = offset.map(|x| x.to_string());
        let params = build_map! {
            optional "country": country.map(|x| x.as_ref()),
            optional "limit": limit.as_deref(),
            optional "offset": offset.as_deref(),
        };

        let url = format!("browse/categories/{}/playlists", category_id);
        let result = self.endpoint_get(&url, &params).await?;
        convert_result::<CategoryPlaylists>(&result).map(|x| x.playlists)
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
    async fn featured_playlists(
        &self,
        locale: Option<&str>,
        country: Option<&Market>,
        timestamp: Option<&chrono::DateTime<chrono::Utc>>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> ClientResult<FeaturedPlaylists> {
        let limit = limit.map(|x| x.to_string());
        let offset = offset.map(|x| x.to_string());
        let timestamp = timestamp.map(|x| x.to_rfc3339());
        let params = build_map! {
            optional "locale": locale,
            optional "country": country.map(|x| x.as_ref()),
            optional "timestamp": timestamp.as_deref(),
            optional "limit": limit.as_deref(),
            optional "offset": offset.as_deref(),
        };

        let result = self
            .endpoint_get("browse/featured-playlists", &params)
            .await?;
        convert_result(&result)
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
    /// See [`Self::new_releases_manual`] for a manually paginated version of
    /// this.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-new-releases)
    fn new_releases<'a>(
        &'a self,
        country: Option<&'a Market>,
    ) -> Paginator<'_, ClientResult<SimplifiedAlbum>> {
        paginate(
            move |limit, offset| self.new_releases_manual(country, Some(limit), Some(offset)),
            self.get_config().pagination_chunks,
        )
    }

    /// The manually paginated version of [`Self::new_releases`].
    async fn new_releases_manual(
        &self,
        country: Option<&Market>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> ClientResult<Page<SimplifiedAlbum>> {
        let limit = limit.map(|x| x.to_string());
        let offset = offset.map(|x| x.to_string());
        let params = build_map! {
            optional "country": country.map(|x| x.as_ref()),
            optional "limit": limit.as_deref(),
            optional "offset": offset.as_deref(),
        };

        let result = self.endpoint_get("browse/new-releases", &params).await?;
        convert_result::<PageSimpliedAlbums>(&result).map(|x| x.albums)
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
    async fn recommendations<'a>(
        &self,
        payload: &Map<String, Value>,
        seed_artists: Option<impl IntoIterator<Item = &'a ArtistId> + 'a>,
        seed_genres: Option<impl IntoIterator<Item = &'a str> + 'a>,
        seed_tracks: Option<impl IntoIterator<Item = &'a TrackId> + 'a>,
        market: Option<&Market>,
        limit: Option<u32>,
    ) -> ClientResult<Recommendations> {
        let seed_artists = seed_artists.map(join_ids);
        let seed_genres = seed_genres.map(|x| x.into_iter().collect::<Vec<_>>().join(","));
        let seed_tracks = seed_tracks.map(join_ids);
        let limit = limit.map(|x| x.to_string());
        let mut params = build_map! {
            optional "seed_artists": seed_artists.as_ref(),
            optional "seed_genres": seed_genres.as_ref(),
            optional "seed_tracks": seed_tracks.as_ref(),
            optional "market": market.map(|x| x.as_ref()),
            optional "limit": limit.as_ref(),
        };

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

        // This map is used to store the intermediate data which lives long enough
        // to be borrowed into the `params`
        let map_to_hold_owned_value = attributes
            .iter()
            // create cartesian product for attributes and prefixes
            .flat_map(|attribute| {
                prefixes
                    .iter()
                    .map(move |prefix| format!("{}_{}", prefix, attribute))
            })
            .filter_map(
                // TODO: not sure if this `to_string` is what we want. It
                // might add quotes to the strings.
                |param| payload.get(&param).map(|value| (param, value.to_string())),
            )
            .collect::<HashMap<_, _>>();

        for (ref key, ref value) in &map_to_hold_owned_value {
            params.insert(key, value);
        }

        let result = self.endpoint_get("recommendations", &params).await?;
        convert_result(&result)
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
    /// See [`Self::playlist_tracks`] for a manually paginated version of this.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-playlists-tracks)
    fn playlist_tracks<'a>(
        &'a self,
        playlist_id: &'a PlaylistId,
        fields: Option<&'a str>,
        market: Option<&'a Market>,
    ) -> Paginator<'_, ClientResult<PlaylistItem>> {
        paginate(
            move |limit, offset| {
                self.playlist_tracks_manual(playlist_id, fields, market, Some(limit), Some(offset))
            },
            self.get_config().pagination_chunks,
        )
    }

    /// The manually paginated version of [`Self::playlist_tracks`].
    async fn playlist_tracks_manual(
        &self,
        playlist_id: &PlaylistId,
        fields: Option<&str>,
        market: Option<&Market>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> ClientResult<Page<PlaylistItem>> {
        let limit = limit.map(|s| s.to_string());
        let offset = offset.map(|s| s.to_string());
        let params = build_map! {
            optional "fields": fields,
            optional "market": market.map(|x| x.as_ref()),
            optional "limit": limit.as_deref(),
            optional "offset": offset.as_deref(),
        };

        let url = format!("playlists/{}/tracks", playlist_id.id());
        let result = self.endpoint_get(&url, &params).await?;
        convert_result(&result)
    }

    /// Gets playlists of a user.
    ///
    /// Parameters:
    /// - user_id - the id of the usr
    /// - limit  - the number of items to return
    /// - offset - the index of the first item to return
    ///
    /// See [`Self::user_playlists_manual`] for a manually paginated version of
    /// this.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-list-users-playlists)
    fn user_playlists<'a>(
        &'a self,
        user_id: &'a UserId,
    ) -> Paginator<'_, ClientResult<SimplifiedPlaylist>> {
        paginate(
            move |limit, offset| self.user_playlists_manual(user_id, Some(limit), Some(offset)),
            self.get_config().pagination_chunks,
        )
    }

    /// The manually paginated version of [`Self::user_playlists`].
    async fn user_playlists_manual(
        &self,
        user_id: &UserId,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> ClientResult<Page<SimplifiedPlaylist>> {
        let limit = limit.map(|s| s.to_string());
        let offset = offset.map(|s| s.to_string());
        let params = build_map! {
            optional "limit": limit.as_deref(),
            optional "offset": offset.as_deref(),
        };

        let url = format!("users/{}/playlists", user_id.id());
        let result = self.endpoint_get(&url, &params).await?;
        convert_result(&result)
    }
}
