//! The HTTP client may vary depending on which one the user configures. This
//! module contains the required logic to use different clients interchangeably.

// Disable all modules when both client features are enabled or when none are.
// This way only the compile error below gets shown instead of a whole list of
// confusing errors..

#[cfg(feature = "client-reqwest")]
mod reqwest;

#[cfg(feature = "client-ureq")]
mod ureq;

#[cfg(any(feature = "client-reqwest", feature = "client-ureq"))]
mod common;

#[cfg(feature = "client-reqwest")]
pub use self::reqwest::ReqwestClient;

#[cfg(feature = "client-ureq")]
pub use self::ureq::UreqClient;

pub use common::{BaseHttpClient, Form, Headers, HttpError, HttpResult, Query};
