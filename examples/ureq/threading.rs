//! This example showcases how the Rspotify client can be used to perform
//! multithreaded requests as well.

use rspotify::{model::Id, prelude::*, ClientCredsSpotify, Credentials};
use std::{
    sync::{mpsc::channel, Arc},
    thread,
};

fn main() {
    let creds = Credentials::from_env().unwrap();

    let mut spotify = ClientCredsSpotify::new(creds);
    let ids = [
        Id::from_uri("spotify:album:0sNOF9WDwhWunNAHPD3Baj").unwrap(),
        Id::from_uri("spotify:album:5EBb7SSkPgxO9Lmt8NjAPT").unwrap(),
        Id::from_uri("spotify:album:3jtOeny1Xh5fp6aSOHahe2").unwrap(),
    ];
    let mut handles = Vec::new();
    let (wr, rd) = channel();

    spotify.request_token().unwrap();

    // Performing the requests concurrently
    let spotify = Arc::new(spotify);
    for id in ids {
        let spotify = Arc::clone(&spotify);
        let wr = wr.clone();
        let handle = thread::spawn(move || {
            let albums = spotify.album(id).unwrap();
            wr.send(albums).unwrap();
        });

        handles.push(handle);
    }

    for _ in ids {
        let album = rd.recv().unwrap();
        println!("Album: {}", album.name);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}
