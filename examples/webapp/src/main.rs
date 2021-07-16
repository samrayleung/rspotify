//! In this example, the token is saved into a cache file. If you are building a
//! real-world web app, you should store it in a database instead. In that case
//! you can disable `token_cached` in the `Config` struct passed to the client
//! when initializing it to avoid using cache files.
#[macro_use]
extern crate rocket;
use getrandom::getrandom;
use rocket::http::{Cookie, CookieJar, SameSite};
use rocket::serde::json::json;
use rocket::{launch, Build};
use rocket::{routes, Rocket};
use rocket_dyn_templates::handlebars::JsonValue;

use rocket::response::Redirect;
use rocket_dyn_templates::Template;
use rspotify::{prelude::*, scopes, AuthCodeSpotify, Config, Credentials, OAuth, Token};

use std::cell::RefCell;
use std::fs;
use std::{collections::HashMap, env, path::PathBuf};

#[derive(Debug, Responder)]
pub enum AppResponse {
    Template(Template),
    Redirect(Redirect),
    Json(JsonValue),
}

const CACHE_PATH: &str = ".spotify_cache/";

/// Generate `length` random chars
fn generate_random_uuid(length: usize) -> String {
    let alphanum: &[u8] =
        "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".as_bytes();
    let mut buf = vec![0u8; length];
    getrandom(&mut buf).unwrap();
    let range = alphanum.len();

    buf.iter()
        .map(|byte| alphanum[*byte as usize % range] as char)
        .collect()
}

fn get_cache_path(cookies: &CookieJar<'_>) -> PathBuf {
    let project_dir_path = env::current_dir().unwrap();
    let mut cache_path = project_dir_path;
    cache_path.push(CACHE_PATH);
    cache_path.push(cookies.get_pending("uuid").unwrap().value());

    cache_path
}

fn create_cache_path_if_absent(cookies: &CookieJar<'_>) -> PathBuf {
    let cache_path = get_cache_path(cookies);
    if !cache_path.exists() {
        let mut path = cache_path.clone();
        path.pop();
        fs::create_dir_all(path).unwrap();
    }
    cache_path
}

fn remove_cache_path(cookies: &CookieJar<'_>) {
    let cache_path = get_cache_path(&cookies);
    if cache_path.exists() {
        fs::remove_file(cache_path).unwrap()
    }
    cookies.remove(Cookie::named("uuid"))
}

fn check_cache_path_exists(cookies: &CookieJar<'_>) -> bool {
    let cache_path = get_cache_path(cookies);
    cache_path.exists()
}

fn init_spotify(cookies: &CookieJar<'_>) -> AuthCodeSpotify {
    let config = Config {
        token_cached: true,
        cache_path: create_cache_path_if_absent(cookies),
        ..Default::default()
    };

    // Please notice that protocol of redirect_uri, make sure it's http
    // (or https). It will fail if you mix them up.
    let oauth = OAuth {
        scopes: scopes!("user-read-currently-playing", "playlist-modify-private"),
        redirect_uri: "http://localhost:8000/callback".to_owned(),
        ..Default::default()
    };

    // Replacing client_id and client_secret with yours.
    let creds = Credentials::new(
        "e1dce60f1e274e20861ce5d96142a4d3",
        "0e4e03b9be8d465d87fc32857a4b5aa3",
    );

    AuthCodeSpotify::with_config(creds, oauth, config)
}

#[get("/callback?<code>")]
fn callback(cookies: &CookieJar, code: String) -> AppResponse {
    let spotify = init_spotify(&cookies);

    match spotify.request_token(&code) {
        Ok(_) => {
            println!("Request user token successful");
            AppResponse::Redirect(Redirect::to("/"))
        }
        Err(err) => {
            println!("Failed to get user token {:?}", err);
            let mut context = HashMap::new();
            context.insert("err_msg", "Failed to get token!");
            AppResponse::Template(Template::render("error", context))
        }
    }
}

#[get("/")]
fn index(cookies: &CookieJar<'_>) -> AppResponse {
    let mut context = HashMap::new();

    // The user is authenticated if their cookie is set and a cache exists for
    // them.
    let authenticated = cookies.get_pending("uuid").is_some() && check_cache_path_exists(&cookies);
    if !authenticated {
        let cookie = Cookie::build("uuid", generate_random_uuid(64))
            .path("/")
            .secure(false)
            .same_site(SameSite::None)
            .finish();
        cookies.add(cookie);
        let spotify = init_spotify(&cookies);
        let auth_url = spotify.get_authorize_url(true).unwrap();
        context.insert("auth_url", auth_url);
        return AppResponse::Template(Template::render("authorize", context));
    }

    let cache_path = get_cache_path(&cookies);
    let token = Token::from_cache(cache_path).unwrap();
    let spotify = AuthCodeSpotify::from_token(token);
    match spotify.me() {
        Ok(user_info) => {
            context.insert(
                "display_name",
                user_info
                    .display_name
                    .unwrap_or_else(|| String::from("Dear")),
            );
            AppResponse::Template(Template::render("index", context.clone()))
        }
        Err(err) => {
            context.insert("err_msg", format!("Failed for {}!", err));
            AppResponse::Template(Template::render("error", context))
        }
    }
}

#[get("/sign_out")]
fn sign_out(cookies: &CookieJar<'_>) -> AppResponse {
    remove_cache_path(cookies);
    AppResponse::Redirect(Redirect::to("/"))
}

#[get("/playlists")]
fn playlist(cookies: &CookieJar<'_>) -> AppResponse {
    let mut spotify = init_spotify(&cookies);
    if !spotify.config.cache_path.exists() {
        return AppResponse::Redirect(Redirect::to("/"));
    }

    let token = spotify.read_token_cache().unwrap();
    spotify.token = RefCell::new(Some(token));
    let playlists = spotify
        .current_user_playlists()
        .take(50)
        .filter_map(Result::ok)
        .collect::<Vec<_>>();

    if playlists.is_empty() {
        return AppResponse::Redirect(Redirect::to("/"));
    }

    AppResponse::Json(json!(playlists))
}

#[get("/me")]
fn me(cookies: &CookieJar<'_>) -> AppResponse {
    let mut spotify = init_spotify(&cookies);
    if !spotify.config.cache_path.exists() {
        return AppResponse::Redirect(Redirect::to("/"));
    }

    spotify.token = RefCell::new(Some(spotify.read_token_cache().unwrap()));
    match spotify.me() {
        Ok(user_info) => AppResponse::Json(json!(user_info)),
        Err(_) => AppResponse::Redirect(Redirect::to("/")),
    }
}

#[launch]
fn rocket() -> Rocket<Build> {
    rocket::build()
        .mount("/", routes![index, callback, sign_out, me, playlist])
        .attach(Template::fairing())
}
