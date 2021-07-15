use crate::Type;
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::marker::PhantomData;
use std::ops::Deref;
use strum::Display;
use thiserror::Error;

// This is a sealed trait pattern implementation, it stops external code from
// implementing the `IdType` trait. The `Sealed` trait must be in a private mod,
// so external code can not see and implement it.
//
// We use the `sealed_types` macro for an easier implementation here.
//
// See also: https://rust-lang.github.io/api-guidelines/future-proofing.html
mod private {
    pub trait Sealed {}
}

pub trait IdType: private::Sealed {
    const TYPE: Type;
}
pub trait PlayableIdType: IdType {}
pub trait PlayContextIdType: IdType {}

macro_rules! sealed_types {
    ($($name:ident),+) => {
        $(
            #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
            pub enum $name {}
            impl private::Sealed for $name {}
            impl IdType for $name {
                const TYPE: Type = Type::$name;
            }
        )+
    }
}

sealed_types!(Artist, Album, Track, Playlist, User, Show, Episode);

impl PlayContextIdType for Artist {}
impl PlayContextIdType for Album {}
impl PlayableIdType for Track {}
impl PlayContextIdType for Playlist {}
impl PlayContextIdType for Show {}
impl PlayableIdType for Episode {}

pub type ArtistId = Id<Artist>;
pub type AlbumId = Id<Album>;
pub type TrackId = Id<Track>;
pub type PlaylistId = Id<Playlist>;
pub type UserId = Id<User>;
pub type ShowId = Id<Show>;
pub type EpisodeId = Id<Episode>;

pub type ArtistIdBuf = IdBuf<Artist>;
pub type AlbumIdBuf = IdBuf<Album>;
pub type TrackIdBuf = IdBuf<Track>;
pub type PlaylistIdBuf = IdBuf<Playlist>;
pub type UserIdBuf = IdBuf<User>;
pub type ShowIdBuf = IdBuf<Show>;
pub type EpisodeIdBuf = IdBuf<Episode>;

/// A Spotify object id of given [type](crate::enums::types::Type).
///
/// This is a not-owning type, it stores a `&str` only. See
/// [IdBuf](crate::idtypes::IdBuf) for owned version of the type.
#[derive(Debug, PartialEq, Eq, Serialize, Hash)]
pub struct Id<T> {
    #[serde(default)]
    _type: PhantomData<T>,
    #[serde(flatten)]
    id: str,
}

/// A Spotify object id of given [type](crate::enums::types::Type)
///
/// This is an owning type, it stores a `String`. See [Id](crate::idtypes::Id)
/// for light-weight non-owning type.
///
/// Use `Id::from_id(val).to_owned()`, `Id::from_uri(val).to_owned()` or
/// `Id::from_id_or_uri(val).to_owned()` to construct an instance of this type.
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, Hash)]
pub struct IdBuf<T> {
    #[serde(default)]
    _type: PhantomData<T>,
    #[serde(flatten)]
    id: String,
}

impl<T> AsRef<Id<T>> for IdBuf<T> {
    fn as_ref(&self) -> &Id<T> {
        // Safe, b/c of the same T between types, IdBuf can't be constructed
        // from invalid id, and Id is just a wrapped str with ZST type tag
        unsafe { &*(&*self.id as *const str as *const Id<T>) }
    }
}

impl<T> Borrow<Id<T>> for IdBuf<T> {
    fn borrow(&self) -> &Id<T> {
        self.as_ref()
    }
}

impl<T> Deref for IdBuf<T> {
    type Target = Id<T>;

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl<T: IdType> IdBuf<T> {
    /// Get a [`Type`](crate::enums::types::Type) of the id
    pub fn _type(&self) -> Type {
        T::TYPE
    }

    /// Spotify object id (guaranteed to be a string of alphanumeric characters)
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Spotify object URI in a well-known format: spotify:type:id
    pub fn uri(&self) -> String {
        self.as_ref().uri()
    }

    /// Full Spotify object URL, can be opened in a browser
    pub fn url(&self) -> String {
        self.as_ref().url()
    }
}

/// Spotify id or URI parsing error
///
/// See also [`Id`](crate::idtypes::Id) for details.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Display, Error)]
pub enum IdError {
    /// Spotify URI prefix is not `spotify:` or `spotify/`
    InvalidPrefix,
    /// Spotify URI can't be split into type and id parts
    /// (e.g. it has invalid separator)
    InvalidFormat,
    /// Spotify URI has invalid type name, or id has invalid type in a given
    /// context (e.g. a method expects a track id, but artist id is provided)
    InvalidType,
    /// Spotify id is invalid (empty or contains non-alphanumeric characters)
    InvalidId,
}

impl<T: IdType> std::fmt::Display for &Id<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "spotify:{}:{}", T::TYPE, &self.id)
    }
}

impl<T> AsRef<str> for &Id<T> {
    fn as_ref(&self) -> &str {
        &self.id
    }
}

impl<T> Borrow<str> for &Id<T> {
    fn borrow(&self) -> &str {
        &self.id
    }
}

impl<T: IdType> std::str::FromStr for IdBuf<T> {
    type Err = IdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Id::from_id_or_uri(s).map(|id| id.to_owned())
    }
}

impl<T: IdType> Id<T> {
    /// Owned version of the id [`IdBuf`](crate::idtypes::IdBuf).
    pub fn to_owned(&self) -> IdBuf<T> {
        IdBuf {
            _type: PhantomData,
            id: (&self.id).to_owned(),
        }
    }

    /// Spotify object type
    pub fn _type(&self) -> Type {
        T::TYPE
    }

    /// Spotify object id (guaranteed to be a string of alphanumeric characters)
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Spotify object URI in a well-known format: spotify:type:id
    ///
    /// Examples: `spotify:album:6IcGNaXFRf5Y1jc7QsE9O2`,
    /// `spotify:track:4y4VO05kYgUTo2bzbox1an`.
    pub fn uri(&self) -> String {
        format!("spotify:{}:{}", T::TYPE, &self.id)
    }

    /// Full Spotify object URL, can be opened in a browser
    ///
    /// Examples: `https://open.spotify.com/track/4y4VO05kYgUTo2bzbox1an`,
    /// `https://open.spotify.com/artist/2QI8e2Vwgg9KXOz2zjcrkI`.
    pub fn url(&self) -> String {
        format!("https://open.spotify.com/{}/{}", T::TYPE, &self.id)
    }

    /// Parse Spotify id or URI from string slice
    ///
    /// Spotify URI must be in one of the following formats:
    /// `spotify:{type}:{id}` or `spotify/{type}/{id}`.
    /// Where `{type}` is one of `artist`, `album`, `track`, `playlist`,
    /// `user`, `show`, or `episode`, and `{id}` is a non-empty
    /// alphanumeric string.
    /// The URI must be of given `T`ype, otherwise `IdError::InvalidType`
    /// error is returned.
    ///
    /// Examples: `spotify:album:6IcGNaXFRf5Y1jc7QsE9O2`,
    /// `spotify/track/4y4VO05kYgUTo2bzbox1an`.
    ///
    /// If input string is not a valid Spotify URI (it's not started with
    /// `spotify:` or `spotify/`), it must be a valid Spotify object id,
    /// i.e. a non-empty alphanumeric string.
    ///
    /// # Errors:
    ///
    /// - `IdError::InvalidType` - if `id_or_uri` is an URI, and it's type part
    ///    is not equal to `T`,
    /// - `IdError::InvalidId` - either if `id_or_uri` is an URI with invalid id
    ///    part, or it's an invalid id (id is invalid if it contains
    ///    non-alphanumeric characters),
    /// - `IdError::InvalidFormat` - if `id_or_uri` is an URI, and it can't be
    ///    split into type and id parts.
    pub fn from_id_or_uri<'a, 'b: 'a>(id_or_uri: &'b str) -> Result<&'a Id<T>, IdError> {
        match Id::<T>::from_uri(id_or_uri) {
            Ok(id) => Ok(id),
            Err(IdError::InvalidPrefix) => Id::<T>::from_id(id_or_uri),
            Err(error) => Err(error),
        }
    }

    /// Parse Spotify id from string slice
    ///
    /// A valid Spotify object id must be a non-empty alphanumeric string.
    ///
    /// # Errors:
    ///
    /// - `IdError::InvalidId` - if `id` contains non-alphanumeric characters.
    pub fn from_id<'a, 'b: 'a>(id: &'b str) -> Result<&'a Id<T>, IdError> {
        if id.chars().all(|ch| ch.is_ascii_alphanumeric()) {
            // Safe, b/c Id is just a str with ZST type tag, and id is proved
            // to be a valid id at this point
            Ok(unsafe { &*(id as *const str as *const Id<T>) })
        } else {
            Err(IdError::InvalidId)
        }
    }

    /// Parse Spotify URI from string slice
    ///
    /// Spotify URI must be in one of the following formats:
    /// `spotify:{type}:{id}` or `spotify/{type}/{id}`.
    /// Where `{type}` is one of `artist`, `album`, `track`, `playlist`, `user`,
    /// `show`, or `episode`, and `{id}` is a non-empty alphanumeric string.
    ///
    /// Examples: `spotify:album:6IcGNaXFRf5Y1jc7QsE9O2`,
    /// `spotify/track/4y4VO05kYgUTo2bzbox1an`.
    ///
    /// # Errors:
    ///
    /// - `IdError::InvalidPrefix` - if `uri` is not started with `spotify:`
    ///    or `spotify/`,
    /// - `IdError::InvalidType` - if type part of an `uri` is not a valid
    ///    Spotify type `T`,
    /// - `IdError::InvalidId` - if id part of an `uri` is not a valid id,
    /// - `IdError::InvalidFormat` - if it can't be splitted into type and
    ///    id parts.
    pub fn from_uri<'a, 'b: 'a>(uri: &'b str) -> Result<&'a Id<T>, IdError> {
        let mut chars = uri
            .strip_prefix("spotify")
            .ok_or(IdError::InvalidPrefix)?
            .chars();
        let sep = match chars.next() {
            Some(ch) if ch == '/' || ch == ':' => ch,
            _ => return Err(IdError::InvalidPrefix),
        };
        let rest = chars.as_str();

        let (tpe, id) = rest
            .rfind(sep)
            .map(|mid| rest.split_at(mid))
            .ok_or(IdError::InvalidFormat)?;

        match tpe.parse::<Type>() {
            Ok(tpe) if tpe == T::TYPE => Id::<T>::from_id(&id[1..]),
            _ => Err(IdError::InvalidType),
        }
    }
}
