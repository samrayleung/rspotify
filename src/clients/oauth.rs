use crate::{
    clients::{
        append_device_id, convert_result, join_ids,
        pagination::{paginate, Paginator},
        BaseClient,
    },
    http::{BaseHttpClient, Query},
    macros::{build_json, build_map},
    model::*,
    ClientResult, OAuth, Token,
};

use std::time;

use log::error;
use maybe_async::maybe_async;
use rspotify_model::idtypes::PlayContextIdType;
use serde_json::{json, Map};
use url::Url;

/// This trait implements the methods available strictly to clients with user
/// authorization, including some parts of the authentication flow that are
/// shared, and the endpoints.
///
/// Note that the base trait [`BaseClient`](crate::clients::BaseClient) may
/// have endpoints that conditionally require authorization like
/// [`user_playlist`](crate::clients::BaseClient::user_playlist). This trait
/// only separates endpoints that *always* need authorization from the base
/// ones.
#[maybe_async(?Send)]
pub trait OAuthClient<Http>: BaseClient<Http>
where
    Http: BaseHttpClient,
{
    fn get_oauth(&self) -> &OAuth;

    /// Obtains a user access token given a code, as part of the OAuth
    /// authentication. The access token will be saved internally.
    async fn request_token(&mut self, code: &str) -> ClientResult<()>;

    /// Refreshes the current access token given a refresh token. The obtained
    /// token will be saved internally.
    async fn refresh_token(&mut self, refresh_token: &str) -> ClientResult<()>;

    /// Tries to read the cache file's token, which may not exist.
    async fn read_token_cache(&mut self) -> Option<Token> {
        let tok = Token::from_cache(&self.get_config().cache_path)?;

        if !self.get_oauth().scopes.is_subset(&tok.scopes) || tok.is_expired() {
            // Invalid token, since it doesn't have at least the currently
            // required scopes or it's expired.
            None
        } else {
            Some(tok)
        }
    }

    /// Parse the response code in the given response url. If the URL cannot be
    /// parsed or the `code` parameter is not present, this will return `None`.
    fn parse_response_code(&self, url: &str) -> Option<String> {
        let url = Url::parse(url).ok()?;
        let mut params = url.query_pairs();
        let (_, url) = params.find(|(key, _)| key == "code")?;
        Some(url.to_string())
    }

    /// Tries to open the authorization URL in the user's browser, and returns
    /// the obtained code.
    ///
    /// Note: this method requires the `cli` feature.
    #[cfg(feature = "cli")]
    fn get_code_from_user(&self, url: &str) -> ClientResult<String> {
        use crate::ClientError;

        match webbrowser::open(&url) {
            Ok(_) => println!("Opened {} in your browser.", url),
            Err(why) => eprintln!(
                "Error when trying to open an URL in your browser: {:?}. \
                 Please navigate here manually: {}",
                why, url
            ),
        }

        println!("Please enter the URL you were redirected to: ");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let code = self
            .parse_response_code(&input)
            .ok_or_else(|| ClientError::Cli("unable to parse the response code".to_string()))?;

        Ok(code)
    }

    /// Opens up the authorization URL in the user's browser so that it can
    /// authenticate. It also reads from the standard input the redirect URI
    /// in order to obtain the access token information. The resulting access
    /// token will be saved internally once the operation is successful.
    ///
    /// Note: this method requires the `cli` feature.
    #[cfg(feature = "cli")]
    #[maybe_async]
    async fn prompt_for_token(&mut self, url: &str) -> ClientResult<()> {
        match self.read_token_cache().await {
            // TODO: shouldn't this also refresh the obtained token?
            Some(ref mut new_token) => {
                self.get_token_mut().replace(new_token);
            }
            // Otherwise following the usual procedure to get the token.
            None => {
                let code = self.get_code_from_user(url)?;
                // Will write to the cache file if successful
                self.request_token(&code).await?;
            }
        }

        self.write_token_cache()
    }

    /// Get current user playlists without required getting his profile.
    ///
    /// Parameters:
    /// - limit  - the number of items to return
    /// - offset - the index of the first item to return
    ///
    /// See [`Self::current_user_playlists_manual`] for a manually paginated
    /// version of this.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-a-list-of-current-users-playlists)
    fn current_user_playlists(&self) -> Paginator<'_, ClientResult<SimplifiedPlaylist>> {
        paginate(
            move |limit, offset| self.current_user_playlists_manual(Some(limit), Some(offset)),
            self.get_config().pagination_chunks,
        )
    }

    /// The manually paginated version of [`Self::current_user_playlists`].
    async fn current_user_playlists_manual(
        &self,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> ClientResult<Page<SimplifiedPlaylist>> {
        let limit = limit.map(|s| s.to_string());
        let offset = offset.map(|s| s.to_string());
        let params = build_map! {
            optional "limit": limit.as_deref(),
            optional "offset": offset.as_deref(),
        };

        let result = self.endpoint_get("me/playlists", &params).await?;
        convert_result(&result)
    }

    /// Creates a playlist for a user.
    ///
    /// Parameters:
    /// - user_id - the id of the user
    /// - name - the name of the playlist
    /// - public - is the created playlist public
    /// - description - the description of the playlist
    /// - collaborative - if the playlist will be collaborative
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-create-playlist)
    async fn user_playlist_create(
        &self,
        user_id: &UserId,
        name: &str,
        public: Option<bool>,
        collaborative: Option<bool>,
        description: Option<&str>,
    ) -> ClientResult<FullPlaylist> {
        let params = build_json! {
            "name": name,
            optional "public": public,
            optional "collaborative": collaborative,
            optional "description": description,
        };

        let url = format!("users/{}/playlists", user_id.id());
        let result = self.endpoint_post(&url, &params).await?;
        convert_result(&result)
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
    async fn playlist_change_detail(
        &self,
        playlist_id: &str,
        name: Option<&str>,
        public: Option<bool>,
        description: Option<&str>,
        collaborative: Option<bool>,
    ) -> ClientResult<String> {
        let params = build_json! {
            optional "name": name,
            optional "public": public,
            optional "collaborative": collaborative,
            optional "description": description,
        };

        let url = format!("playlists/{}", playlist_id);
        self.endpoint_put(&url, &params).await
    }

    /// Unfollows (deletes) a playlist for a user.
    ///
    /// Parameters:
    /// - playlist_id - the id of the playlist
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-unfollow-playlist)
    async fn playlist_unfollow(&self, playlist_id: &str) -> ClientResult<String> {
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
    async fn playlist_add_tracks<'a>(
        &self,
        playlist_id: &PlaylistId,
        track_ids: impl IntoIterator<Item = &'a TrackId> + 'a,
        position: Option<i32>,
    ) -> ClientResult<PlaylistResult> {
        let uris = track_ids.into_iter().map(|id| id.uri()).collect::<Vec<_>>();
        let params = build_json! {
            "uris": uris,
            optional "position": position,
        };

        let url = format!("playlists/{}/tracks", playlist_id.id());
        let result = self.endpoint_post(&url, &params).await?;
        convert_result(&result)
    }

    /// Replace all tracks in a playlist
    ///
    /// Parameters:
    /// - user - the id of the user
    /// - playlist_id - the id of the playlist
    /// - tracks - the list of track ids to add to the playlist
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-reorder-or-replace-playlists-tracks)
    async fn playlist_replace_tracks<'a>(
        &self,
        playlist_id: &PlaylistId,
        track_ids: impl IntoIterator<Item = &'a TrackId> + 'a,
    ) -> ClientResult<()> {
        let uris = track_ids.into_iter().map(|id| id.uri()).collect::<Vec<_>>();
        let params = build_json! {
            "uris": uris
        };

        let url = format!("playlists/{}/tracks", playlist_id.id());
        self.endpoint_put(&url, &params).await?;

        Ok(())
    }

    /// Reorder tracks in a playlist.
    ///
    /// Parameters:
    /// - playlist_id - the id of the playlist
    /// - uris - a list of Spotify URIs to replace or clear
    /// - range_start - the position of the first track to be reordered
    /// - insert_before - the position where the tracks should be inserted
    /// - range_length - optional the number of tracks to be reordered (default:
    ///   1)
    /// - snapshot_id - optional playlist's snapshot ID
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-reorder-or-replace-playlists-tracks)
    async fn playlist_reorder_tracks<'a, T: PlayableIdType + 'a>(
        &self,
        playlist_id: &PlaylistId,
        uris: Option<impl IntoIterator<Item = &'a Id<T>> + 'a>,
        range_start: Option<i32>,
        insert_before: Option<i32>,
        range_length: Option<u32>,
        snapshot_id: Option<&str>,
    ) -> ClientResult<PlaylistResult> {
        let uris = uris.map(|u| u.into_iter().map(|id| id.uri()).collect::<Vec<_>>());
        let params = build_json! {
            optional "uris": uris,
            optional "range_start": range_start,
            optional "insert_before": insert_before,
            optional "range_length": range_length,
            optional "snapshot_id": snapshot_id,
        };

        let url = format!("playlists/{}/tracks", playlist_id.id());
        let result = self.endpoint_put(&url, &params).await?;
        convert_result(&result)
    }

    /// Removes all occurrences of the given tracks from the given playlist.
    ///
    /// Parameters:
    /// - playlist_id - the id of the playlist
    /// - track_ids - the list of track ids to add to the playlist
    /// - snapshot_id - optional id of the playlist snapshot
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-remove-tracks-playlist)
    async fn playlist_remove_all_occurrences_of_tracks<'a>(
        &self,
        playlist_id: &PlaylistId,
        track_ids: impl IntoIterator<Item = &'a TrackId> + 'a,
        snapshot_id: Option<&str>,
    ) -> ClientResult<PlaylistResult> {
        let tracks = track_ids
            .into_iter()
            .map(|id| {
                let mut map = Map::with_capacity(1);
                map.insert("uri".to_owned(), id.uri().into());
                map
            })
            .collect::<Vec<_>>();

        let params = build_json! {
            "tracks": tracks,
            optional "snapshot_id": snapshot_id,
        };

        let url = format!("playlists/{}/tracks", playlist_id.id());
        let result = self.endpoint_delete(&url, &params).await?;
        convert_result(&result)
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
    async fn playlist_remove_specific_occurrences_of_tracks<'a>(
        &self,
        playlist_id: &PlaylistId,
        tracks: impl IntoIterator<Item = &'a TrackPositions<'a>> + 'a,
        snapshot_id: Option<&str>,
    ) -> ClientResult<PlaylistResult> {
        let tracks = tracks
            .into_iter()
            .map(|track| {
                let mut map = Map::new();
                map.insert("uri".to_owned(), track.id.uri().into());
                map.insert("positions".to_owned(), json!(track.positions));
                map
            })
            .collect::<Vec<_>>();

        let params = build_json! {
            "tracks": tracks,
            optional "snapshot_id": snapshot_id,
        };

        let url = format!("playlists/{}/tracks", playlist_id.id());
        let result = self.endpoint_delete(&url, &params).await?;
        convert_result(&result)
    }

    /// Add the current authenticated user as a follower of a playlist.
    ///
    /// Parameters:
    /// - playlist_id - the id of the playlist
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-follow-playlist)
    async fn playlist_follow(
        &self,
        playlist_id: &PlaylistId,
        public: Option<bool>,
    ) -> ClientResult<()> {
        let url = format!("playlists/{}/followers", playlist_id.id());

        let params = build_json! {
            optional "public": public,
        };

        self.endpoint_put(&url, &params).await?;

        Ok(())
    }

    /// Get detailed profile information about the current user.
    /// An alias for the 'current_user' method.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-current-users-profile)
    async fn me(&self) -> ClientResult<PrivateUser> {
        let result = self.endpoint_get("me/", &Query::new()).await?;
        convert_result(&result)
    }

    /// Get detailed profile information about the current user.
    /// An alias for the 'me' method.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-current-users-profile)
    async fn current_user(&self) -> ClientResult<PrivateUser> {
        self.me().await
    }

    /// Get information about the current users currently playing track.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-recently-played)
    async fn current_user_playing_track(&self) -> ClientResult<Option<CurrentlyPlayingContext>> {
        let result = self
            .get("me/player/currently-playing", None, &Query::new())
            .await?;
        if result.is_empty() {
            Ok(None)
        } else {
            convert_result(&result)
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
    /// See [`Self::current_user_saved_albums`] for a manually paginated version
    /// of this.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-users-saved-albums)
    fn current_user_saved_albums(&self) -> Paginator<'_, ClientResult<SavedAlbum>> {
        paginate(
            move |limit, offset| self.current_user_saved_albums_manual(Some(limit), Some(offset)),
            self.get_config().pagination_chunks,
        )
    }

    /// The manually paginated version of [`Self::current_user_saved_albums`].
    async fn current_user_saved_albums_manual(
        &self,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> ClientResult<Page<SavedAlbum>> {
        let limit = limit.map(|s| s.to_string());
        let offset = offset.map(|s| s.to_string());
        let params = build_map! {
            optional "limit": limit.as_deref(),
            optional "offset": offset.as_deref(),
        };

        let result = self.endpoint_get("me/albums", &params).await?;
        convert_result(&result)
    }

    /// Get a list of the songs saved in the current Spotify user's "Your Music"
    /// library.
    ///
    /// Parameters:
    /// - limit - the number of tracks to return
    /// - offset - the index of the first track to return
    /// - market - Provide this parameter if you want to apply Track Relinking.
    ///
    /// See [`Self::current_user_saved_tracks_manual`] for a manually paginated
    /// version of this.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-users-saved-tracks)
    fn current_user_saved_tracks(&self) -> Paginator<'_, ClientResult<SavedTrack>> {
        paginate(
            move |limit, offset| self.current_user_saved_tracks_manual(Some(limit), Some(offset)),
            self.get_config().pagination_chunks,
        )
    }

    /// The manually paginated version of [`Self::current_user_saved_tracks`].
    async fn current_user_saved_tracks_manual(
        &self,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> ClientResult<Page<SavedTrack>> {
        let limit = limit.map(|s| s.to_string());
        let offset = offset.map(|s| s.to_string());
        let params = build_map! {
            optional "limit": limit.as_deref(),
            optional "offset": offset.as_deref(),
        };

        let result = self.endpoint_get("me/tracks", &params).await?;
        convert_result(&result)
    }

    /// Gets a list of the artists followed by the current authorized user.
    ///
    /// Parameters:
    /// - after - the last artist ID retrieved from the previous request
    /// - limit - the number of tracks to return
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-followed)
    async fn current_user_followed_artists(
        &self,
        after: Option<&str>,
        limit: Option<u32>,
    ) -> ClientResult<CursorBasedPage<FullArtist>> {
        let limit = limit.map(|s| s.to_string());
        let params = build_map! {
            "type": Type::Artist.as_ref(),
            optional "after": after,
            optional "limit": limit.as_deref(),
        };

        let result = self.endpoint_get("me/following", &params).await?;
        convert_result::<CursorPageFullArtists>(&result).map(|x| x.artists)
    }

    /// Remove one or more tracks from the current user's "Your Music" library.
    ///
    /// Parameters:
    /// - track_ids - a list of track URIs, URLs or IDs
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-remove-tracks-user)
    async fn current_user_saved_tracks_delete<'a>(
        &self,
        track_ids: impl IntoIterator<Item = &'a TrackId> + 'a,
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
    async fn current_user_saved_tracks_contains<'a>(
        &self,
        track_ids: impl IntoIterator<Item = &'a TrackId> + 'a,
    ) -> ClientResult<Vec<bool>> {
        let url = format!("me/tracks/contains/?ids={}", join_ids(track_ids));
        let result = self.endpoint_get(&url, &Query::new()).await?;
        convert_result(&result)
    }

    /// Save one or more tracks to the current user's "Your Music" library.
    ///
    /// Parameters:
    /// - track_ids - a list of track URIs, URLs or IDs
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-save-tracks-user)
    async fn current_user_saved_tracks_add<'a>(
        &self,
        track_ids: impl IntoIterator<Item = &'a TrackId> + 'a,
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
    /// See [`Self::current_user_top_artists_manual`] for a manually paginated
    /// version of this.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-users-top-artists-and-tracks)
    fn current_user_top_artists<'a>(
        &'a self,
        time_range: Option<&'a TimeRange>,
    ) -> Paginator<'_, ClientResult<FullArtist>> {
        paginate(
            move |limit, offset| {
                self.current_user_top_artists_manual(time_range, Some(limit), Some(offset))
            },
            self.get_config().pagination_chunks,
        )
    }

    /// The manually paginated version of [`Self::current_user_top_artists`].
    async fn current_user_top_artists_manual(
        &self,
        time_range: Option<&TimeRange>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> ClientResult<Page<FullArtist>> {
        let limit = limit.map(|s| s.to_string());
        let offset = offset.map(|s| s.to_string());
        let params = build_map! {
            optional "time_range": time_range.map(|x| x.as_ref()),
            optional "limit": limit.as_deref(),
            optional "offset": offset.as_deref(),
        };

        let result = self.endpoint_get(&"me/top/artists", &params).await?;
        convert_result(&result)
    }

    /// Get the current user's top tracks.
    ///
    /// Parameters:
    /// - limit - the number of entities to return
    /// - offset - the index of the first entity to return
    /// - time_range - Over what time frame are the affinities computed
    ///
    /// See [`Self::current_user_top_tracks_manual`] for a manually paginated
    /// version of this.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-users-top-artists-and-tracks)
    fn current_user_top_tracks<'a>(
        &'a self,
        time_range: Option<&'a TimeRange>,
    ) -> Paginator<'_, ClientResult<FullTrack>> {
        paginate(
            move |limit, offset| {
                self.current_user_top_tracks_manual(time_range, Some(limit), Some(offset))
            },
            self.get_config().pagination_chunks,
        )
    }

    /// The manually paginated version of [`Self::current_user_top_tracks`].
    async fn current_user_top_tracks_manual(
        &self,
        time_range: Option<&TimeRange>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> ClientResult<Page<FullTrack>> {
        let limit = limit.map(|x| x.to_string());
        let offset = offset.map(|x| x.to_string());
        let params = build_map! {
            optional "time_range": time_range.map(|x| x.as_ref()),
            optional "limit": limit.as_deref(),
            optional "offset": offset.as_deref(),
        };

        let result = self.endpoint_get("me/top/tracks", &params).await?;
        convert_result(&result)
    }

    /// Get the current user's recently played tracks.
    ///
    /// Parameters:
    /// - limit - the number of entities to return
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-the-users-currently-playing-track)
    async fn current_user_recently_played(
        &self,
        limit: Option<u32>,
    ) -> ClientResult<CursorBasedPage<PlayHistory>> {
        let limit = limit.map(|x| x.to_string());
        let params = build_map! {
            optional "limit": limit.as_deref(),
        };

        let result = self
            .endpoint_get("me/player/recently-played", &params)
            .await?;
        convert_result(&result)
    }

    /// Add one or more albums to the current user's "Your Music" library.
    ///
    /// Parameters:
    /// - album_ids - a list of album URIs, URLs or IDs
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-save-albums-user)
    async fn current_user_saved_albums_add<'a>(
        &self,
        album_ids: impl IntoIterator<Item = &'a AlbumId> + 'a,
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
    async fn current_user_saved_albums_delete<'a>(
        &self,
        album_ids: impl IntoIterator<Item = &'a AlbumId> + 'a,
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
    async fn current_user_saved_albums_contains<'a>(
        &self,
        album_ids: impl IntoIterator<Item = &'a AlbumId> + 'a,
    ) -> ClientResult<Vec<bool>> {
        let url = format!("me/albums/contains/?ids={}", join_ids(album_ids));
        let result = self.endpoint_get(&url, &Query::new()).await?;
        convert_result(&result)
    }

    /// Follow one or more artists.
    ///
    /// Parameters:
    /// - artist_ids - a list of artist IDs
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-follow-artists-users)
    async fn user_follow_artists<'a>(
        &self,
        artist_ids: impl IntoIterator<Item = &'a ArtistId> + 'a,
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
    async fn user_unfollow_artists<'a>(
        &self,
        artist_ids: impl IntoIterator<Item = &'a ArtistId> + 'a,
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
    async fn user_artist_check_follow<'a>(
        &self,
        artist_ids: impl IntoIterator<Item = &'a ArtistId> + 'a,
    ) -> ClientResult<Vec<bool>> {
        let url = format!(
            "me/following/contains?type=artist&ids={}",
            join_ids(artist_ids)
        );
        let result = self.endpoint_get(&url, &Query::new()).await?;
        convert_result(&result)
    }

    /// Follow one or more users.
    ///
    /// Parameters:
    /// - user_ids - a list of artist IDs
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-follow-artists-users)
    async fn user_follow_users<'a>(
        &self,
        user_ids: impl IntoIterator<Item = &'a UserId> + 'a,
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
    async fn user_unfollow_users<'a>(
        &self,
        user_ids: impl IntoIterator<Item = &'a UserId> + 'a,
    ) -> ClientResult<()> {
        let url = format!("me/following?type=user&ids={}", join_ids(user_ids));
        self.endpoint_delete(&url, &json!({})).await?;

        Ok(())
    }

    /// Get a User’s Available Devices
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-a-users-available-devices)
    async fn device(&self) -> ClientResult<Vec<Device>> {
        let result = self
            .endpoint_get("me/player/devices", &Query::new())
            .await?;
        convert_result::<DevicePayload>(&result).map(|x| x.devices)
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
    async fn current_playback<'a>(
        &self,
        country: Option<&Market>,
        additional_types: Option<impl IntoIterator<Item = &'a AdditionalType> + 'a>,
    ) -> ClientResult<Option<CurrentPlaybackContext>> {
        let additional_types = additional_types.map(|x| {
            x.into_iter()
                .map(|x| x.as_ref())
                .collect::<Vec<_>>()
                .join(",")
        });
        let params = build_map! {
            optional "country": country.map(|x| x.as_ref()),
            optional "additional_types": additional_types.as_deref(),
        };

        let result = self.endpoint_get("me/player", &params).await?;
        if result.is_empty() {
            Ok(None)
        } else {
            convert_result(&result)
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
    async fn current_playing<'a>(
        &self,
        market: Option<&'a Market>,
        additional_types: Option<impl IntoIterator<Item = &'a AdditionalType> + 'a>,
    ) -> ClientResult<Option<CurrentlyPlayingContext>> {
        let additional_types = additional_types.map(|x| {
            x.into_iter()
                .map(|x| x.as_ref())
                .collect::<Vec<_>>()
                .join(",")
        });
        let params = build_map! {
            optional "market": market.map(|x| x.as_ref()),
            optional "additional_types": additional_types.as_ref(),
        };

        let result = self
            .endpoint_get("me/player/currently-playing", &params)
            .await?;
        if result.is_empty() {
            Ok(None)
        } else {
            convert_result(&result)
        }
    }

    /// Transfer a User’s Playback.
    ///
    /// Note: Although an array is accepted, only a single device_id is
    /// currently supported. Supplying more than one will return 400 Bad Request
    ///
    /// Parameters:
    /// - device_id - transfer playback to this device
    /// - force_play - true: after transfer, play. false: keep current state.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-transfer-a-users-playback)
    async fn transfer_playback(&self, device_id: &str, play: Option<bool>) -> ClientResult<()> {
        let params = build_json! {
            "device_ids": [device_id],
            optional "play": play,
        };

        self.endpoint_put("me/player", &params).await?;
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
    async fn start_context_playback<U: PlayContextIdType, O: PlayableIdType>(
        &self,
        context_uri: &Id<U>,
        device_id: Option<&str>,
        offset: Option<Offset<O>>,
        position_ms: Option<time::Duration>,
    ) -> ClientResult<()> {
        let params = build_json! {
            "context_uri": context_uri.uri(),
            optional "offset": offset.map(|x| match x {
                Offset::Position(position) => json!({ "position": position }),
                Offset::Uri(uri) => json!({ "uri": uri.uri() }),
            }),
            optional "position_ms": position_ms,

        };

        let url = append_device_id("me/player/play", device_id);
        self.put(&url, None, &params).await?;

        Ok(())
    }

    async fn start_uris_playback<'a, T: PlayableIdType + 'a>(
        &self,
        uris: impl IntoIterator<Item = &'a Id<T>> + 'a,
        device_id: Option<&str>,
        offset: Option<crate::model::Offset<T>>,
        position_ms: Option<u32>,
    ) -> ClientResult<()> {
        let params = build_json! {
            "uris": uris.into_iter().map(|id| id.uri()).collect::<Vec<_>>(),
            optional "position_ms": position_ms,
            optional "offset": offset.map(|x| match x {
                Offset::Position(position) => json!({ "position": position }),
                Offset::Uri(uri) => json!({ "uri": uri.uri() }),
            }),
        };

        let url = append_device_id("me/player/play", device_id);
        self.endpoint_put(&url, &params).await?;

        Ok(())
    }

    /// Pause a User’s Playback.
    ///
    /// Parameters:
    /// - device_id - device target for playback
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-pause-a-users-playback)
    async fn pause_playback(&self, device_id: Option<&str>) -> ClientResult<()> {
        let url = append_device_id("me/player/pause", device_id);
        self.endpoint_put(&url, &json!({})).await?;

        Ok(())
    }

    /// Skip User’s Playback To Next Track.
    ///
    /// Parameters:
    /// - device_id - device target for playback
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-skip-users-playback-to-next-track)
    async fn next_track(&self, device_id: Option<&str>) -> ClientResult<()> {
        let url = append_device_id("me/player/next", device_id);
        self.endpoint_post(&url, &json!({})).await?;

        Ok(())
    }

    /// Skip User’s Playback To Previous Track.
    ///
    /// Parameters:
    /// - device_id - device target for playback
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-skip-users-playback-to-previous-track)
    async fn previous_track(&self, device_id: Option<&str>) -> ClientResult<()> {
        let url = append_device_id("me/player/previous", device_id);
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
    async fn seek_track(&self, position_ms: u32, device_id: Option<&str>) -> ClientResult<()> {
        let url = append_device_id(
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
    async fn repeat(&self, state: &RepeatState, device_id: Option<&str>) -> ClientResult<()> {
        let url = append_device_id(
            &format!("me/player/repeat?state={}", state.as_ref()),
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
    async fn volume(&self, volume_percent: u8, device_id: Option<&str>) -> ClientResult<()> {
        if volume_percent > 100u8 {
            error!("volume must be between 0 and 100, inclusive");
        }
        let url = append_device_id(
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
    async fn shuffle(&self, state: bool, device_id: Option<&str>) -> ClientResult<()> {
        let url = append_device_id(&format!("me/player/shuffle?state={}", state), device_id);
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
    async fn add_item_to_queue<Item: PlayableIdType>(
        &self,
        item: &Id<Item>,
        device_id: Option<&str>,
    ) -> ClientResult<()> {
        let url = append_device_id(&format!("me/player/queue?uri={}", item), device_id);
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
    async fn save_shows<'a>(
        &self,
        show_ids: impl IntoIterator<Item = &'a ShowId> + 'a,
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
    /// See [`Self::get_saved_show_manual`] for a manually paginated version of
    /// this.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-users-saved-shows)
    fn get_saved_show(&self) -> Paginator<'_, ClientResult<Show>> {
        paginate(
            move |limit, offset| self.get_saved_show_manual(Some(limit), Some(offset)),
            self.get_config().pagination_chunks,
        )
    }

    /// The manually paginated version of [`Self::get_saved_show`].
    async fn get_saved_show_manual(
        &self,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> ClientResult<Page<Show>> {
        let limit = limit.map(|x| x.to_string());
        let offset = offset.map(|x| x.to_string());
        let params = build_map! {
            optional "limit": limit.as_ref(),
            optional "offset": offset.as_ref(),
        };

        let result = self.endpoint_get("me/shows", &params).await?;
        convert_result(&result)
    }

    /// Check if one or more shows is already saved in the current Spotify user’s library.
    ///
    /// Query Parameters
    /// - ids: Required. A comma-separated list of the Spotify IDs for the shows. Maximum: 50 IDs.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-check-users-saved-shows)
    async fn check_users_saved_shows<'a>(
        &self,
        ids: impl IntoIterator<Item = &'a ShowId> + 'a,
    ) -> ClientResult<Vec<bool>> {
        let ids = join_ids(ids);
        let params = build_map! {
            "ids": &ids,
        };
        let result = self.endpoint_get("me/shows/contains", &params).await?;
        convert_result(&result)
    }

    /// Delete one or more shows from current Spotify user's library.
    /// Changes to a user's saved shows may not be visible in other Spotify applications immediately.
    ///
    /// Query Parameters
    /// - ids: Required. A comma-separated list of Spotify IDs for the shows to be deleted from the user’s library.
    /// - market: Optional. An ISO 3166-1 alpha-2 country code or the string from_token.
    ///
    /// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-remove-shows-user)
    async fn remove_users_saved_shows<'a>(
        &self,
        show_ids: impl IntoIterator<Item = &'a ShowId> + 'a,
        country: Option<&Market>,
    ) -> ClientResult<()> {
        let url = format!("me/shows?ids={}", join_ids(show_ids));
        let params = build_json! {
            optional "country": country.map(|x| x.as_ref())
        };
        self.endpoint_delete(&url, &params).await?;

        Ok(())
    }
}
