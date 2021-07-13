//! This example proves that you can use any HTTP client you like, as long as
//! it's available via its "feature".

use rspotify::{
    http::{ReqwestClient, UreqClient},
    model::Id,
    prelude::*,
    ClientCredsSpotify, Credentials,
};

#[tokio::main]
async fn main() {
    // You can use any logger for debugging.
    env_logger::init();

    let creds = Credentials::from_env().unwrap();

    // Spotify's clients are generic over the HTTP client; you have to specify
    // which one you want to use. Since this example enables both
    // `client-reqwest` and `client-ureq`, we can use either of them, even at
    // the same time:
    let mut spotify_reqwest = ClientCredsSpotify::<ReqwestClient>::new(creds);
    let mut spotify_ureq = ClientCredsSpotify::<UreqClient>::new(creds);

    spotify_reqwest.request_token().await.unwrap();
    spotify_ureq.request_token().unwrap();

    // Running the requests
    let birdy_uri = Id::from_uri("spotify:album:0sNOF9WDwhWunNAHPD3Baj").unwrap();
    let albums = spotify_reqwest.album(birdy_uri).await;
    println!("Response with reqwest: {:#?}", albums);

    let albums = spotify_ureq.album(birdy_uri).await;
    println!("Response with ureq: {:#?}", albums);
}
