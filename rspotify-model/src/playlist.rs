//! All kinds of playlists objects

use chrono::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::image::Image;
use super::page::Page;
use super::user::PublicUser;
use crate::{Followers, PlayableItem, Type};

/// Playlist result object
///
/// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-add-tracks-to-playlist)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlaylistResult {
    pub snapshot_id: String,
}

/// Playlist Track Reference Object
///
/// [Reference](https://developer.spotify.com/documentation/web-api/reference/#object-playlisttracksrefobject)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlaylistTracksRef {
    pub href: String,
    pub total: u32,
}

/// Simplified playlist object
///
/// [Reference](https://developer.spotify.com/documentation/web-api/reference/#object-simplifiedplaylistobject)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimplifiedPlaylist {
    pub collaborative: bool,
    pub external_urls: HashMap<String, String>,
    pub href: String,
    pub id: String,
    pub images: Vec<Image>,
    pub name: String,
    pub owner: PublicUser,
    pub public: Option<bool>,
    pub snapshot_id: String,
    pub tracks: PlaylistTracksRef,
    #[serde(rename = "type")]
    pub _type: Type,
    pub uri: String,
}

/// Full playlist object
///
/// [Reference](https://developer.spotify.com/documentation/web-api/reference/#object-playlistobject)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FullPlaylist {
    pub collaborative: bool,
    pub description: Option<String>,
    pub external_urls: HashMap<String, String>,
    pub followers: Followers,
    pub href: String,
    pub id: String,
    pub images: Vec<Image>,
    pub name: String,
    pub owner: PublicUser,
    pub public: Option<bool>,
    pub snapshot_id: String,
    pub tracks: Page<PlaylistItem>,
    #[serde(rename = "type")]
    pub _type: Type,
    pub uri: String,
}

/// Playlist track object
///
/// [Reference](https://developer.spotify.com/documentation/web-api/reference/#object-playlisttrackobject)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlaylistItem {
    pub added_at: Option<DateTime<Utc>>,
    pub added_by: Option<PublicUser>,
    pub is_local: bool,
    pub track: Option<PlayableItem>,
}
/// Featured playlists object
/// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-featured-playlists)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FeaturedPlaylists {
    pub message: String,
    pub playlists: Page<SimplifiedPlaylist>,
}

/// Category playlists object wrapped by `Page`
///
/// [Reference](https://developer.spotify.com/documentation/web-api/reference/#endpoint-get-a-categories-playlists)
#[derive(Deserialize)]
pub struct CategoryPlaylists {
    pub playlists: Page<SimplifiedPlaylist>,
}
