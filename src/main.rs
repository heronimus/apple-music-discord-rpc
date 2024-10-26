// use backoff::{backoff::Backoff, ExponentialBackoff};
use discord_presence::models::rich_presence::ActivityType;
use discord_presence::Client;
use reqwest::blocking::{Client as HttpClient, ClientBuilder};
use serde::Deserialize;
use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::{thread, time::Duration};
use thiserror::Error;
use url::form_urlencoded;

use objc2::rc::{autoreleasepool, Retained};
use objc2_foundation::{NSDate, NSRunLoop};
use objc2_media_player::{MPMediaPlayback, MPMusicPlayerController};

#[derive(Debug, Clone)]
struct MusicProps {
    name: String,
    artist: String,
    album: String,
    duration: f64,
    player_position: f64,
}

#[derive(Debug, Deserialize)]
struct Release {
    id: String,
}

#[derive(Debug, Deserialize)]
struct ITunesSearchResponse {
    #[serde(rename = "resultCount")]
    result_count: i32,
    results: Vec<ITunesSearchResult>,
}

#[derive(Debug, Deserialize)]
struct ITunesSearchResult {
    #[serde(rename = "trackName")]
    track_name: String,
    #[serde(rename = "collectionName")]
    collection_name: String,
    #[serde(rename = "artworkUrl100")]
    artwork_url100: String,
    #[serde(rename = "trackViewUrl")]
    track_view_url: String,
}

#[derive(Debug, Deserialize)]
struct MusicBrainzResponse {
    releases: Vec<Release>,
}

#[derive(Error, Debug)]
enum AppError {
    #[error("Music app is not playing any song")]
    NoSongPlaying,
    #[error("Failed to get music property: {0}")]
    MusicPropertyError(String),
    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),
    #[error("Discord RPC error: {0}")]
    DiscordError(#[from] discord_presence::error::DiscordError),
    #[error("Other error: {0}")]
    Other(String),
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut client = Client::new(773825528921849856);
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    let http_client = ClientBuilder::new()
        .timeout(Duration::from_secs(10))
        .build()?;

    client
        .on_ready(|_ctx| {
            println!("Discord RPC connected!");
        })
        .persist();

    client.start();

    ctrlc::set_handler(move || {
        println!("Shutting down...");
        r.store(false, Ordering::SeqCst);
    })?;

    unsafe {
        let run_loop = NSRunLoop::currentRunLoop();
        let mut date_loop = NSDate::now();

        while running.load(Ordering::SeqCst) {
            date_loop = date_loop.dateByAddingTimeInterval(15.0);
            run_loop.runUntilDate(&date_loop);

            if let Err(e) = update_loop(&mut client, &http_client) {
                eprintln!("Error in update loop: {}", e);
                thread::sleep(Duration::from_secs(30)); // Wait longer on error
            } else {
                thread::sleep(Duration::from_secs(15));
            }
        }
    }

    client.clear_activity()?;
    println!("Disconnected from Discord RPC.");

    Ok(())
}

unsafe fn update_loop(client: &mut Client, http_client: &HttpClient) -> Result<(), AppError> {
    match get_music_props() {
        Ok(props) => {
            println!("DEBUG: Current music props: {:?}", props);
            // let artwork_url = retry_with_backoff(|| get_artwork_musicbrainz(http_client, &props))?;
            // let artwork_url = itunes_search(http_client, &props)
            //     .unwrap_or_else(|_| None)
            //     .or_else(|| {
            //         retry_with_backoff(|| get_artwork_musicbrainz(http_client, &props))
            //             .ok()
            //             .flatten()
            //     });
            let artwork_url = match itunes_search(http_client, &props) {
                Ok(Some(url)) => Some(url),
                _ => match get_artwork_musicbrainz(http_client, &props) {
                    Ok(Some(url)) => Some(url),
                    _ => None,
                },
            };

            update_presence(client, &props, artwork_url)?;
        }
        Err(AppError::NoSongPlaying) => {
            println!("DEBUG: No song playing");
            client.clear_activity()?;
        }
        Err(e) => return Err(e),
    }
    Ok(())
}

unsafe fn get_music_props() -> Result<MusicProps, AppError> {
    autoreleasepool(|pool| {
        // Create a new player instance each time
        let player = Retained::autorelease(MPMusicPlayerController::systemMusicPlayer(), pool);

        println!("DEBUG: Player state: {:?}", player.playbackState());
        println!("DEBUG: Now playing item: {:?}", player.nowPlayingItem());

        let props = match player.nowPlayingItem() {
            Some(item) => {
                let name = item
                    .title()
                    .map(|s| s.to_string())
                    .ok_or_else(|| AppError::MusicPropertyError("title".to_string()))?;
                let artist = item
                    .artist()
                    .map(|s| s.to_string())
                    .ok_or_else(|| AppError::MusicPropertyError("artist".to_string()))?;
                let album = item
                    .albumTitle()
                    .map(|s| s.to_string())
                    .ok_or_else(|| AppError::MusicPropertyError("album".to_string()))?;
                let duration = item.playbackDuration();
                let player_position = player.currentPlaybackTime();

                MusicProps {
                    name,
                    artist,
                    album,
                    duration,
                    player_position,
                }
            }
            None => return Err(AppError::NoSongPlaying),
        };

        println!("DEBUG: get_music_props returning: {:?}", props);
        Ok(props)
    })
}

// fn retry_with_backoff<T, E, F>(f: F) -> Result<T, AppError>
// where
//     F: Fn() -> Result<T, E>,
//     E: std::fmt::Display,
// {
//     let mut backoff = ExponentialBackoff::default();
//     loop {
//         match f() {
//             Ok(value) => return Ok(value),
//             Err(err) => {
//                 if let Some(duration) = backoff.next_backoff() {
//                     eprintln!("Operation failed, retrying in {:?}: {}", duration, err);
//                     thread::sleep(duration);
//                 } else {
//                     return Err(AppError::Other(format!(
//                         "Operation failed after retries: {}",
//                         err
//                     )));
//                 }
//             }
//         }
//     }
// }

fn itunes_search(http_client: &HttpClient, props: &MusicProps) -> Result<Option<String>, AppError> {
    let query = format!("{} {} {}", props.name, props.artist, props.album).replace("*", "");
    let params = form_urlencoded::Serializer::new(String::new())
        .append_pair("media", "music")
        .append_pair("entity", "song")
        .append_pair("term", &query)
        .finish();

    let url = format!("https://itunes.apple.com/search?{}", params);

    let response = http_client
        .get(&url)
        .header("User-Agent", "discord-rich-presence")
        .header("Accept", "application/json")
        .send()?;

    // println!("DEBUG iTunes Response: {:#?}", response);
    // println!("DEBUG iTunes URL: {:#?}", url);

    let responses: ITunesSearchResponse = response.json()?;

    if responses.result_count == 1 {
        println!(
            "DEBUG: TrackViewURL: {}",
            responses.results[0].track_view_url
        );
        Ok(Some(responses.results[0].artwork_url100.clone()))
    } else if responses.result_count > 1 {
        // If there are multiple results, find the right album
        //
        Ok(responses
            .results
            .iter()
            .find(|r| {
                r.collection_name
                    .to_lowercase()
                    .contains(&props.album.to_lowercase())
                    && r.track_name
                        .to_lowercase()
                        .contains(&props.name.to_lowercase())
            })
            .map(|r| r.artwork_url100.clone()))
    } else {
        Ok(None)
    }
}

fn get_artwork_musicbrainz(
    http_client: &HttpClient,
    props: &MusicProps,
) -> Result<Option<String>, AppError> {
    const MB_EXCLUDED_NAMES: [&str; 2] = ["Various Artist", "Single"];

    let query_terms: Vec<String> = vec![
        if !MB_EXCLUDED_NAMES
            .iter()
            .any(|&name| props.artist.contains(name))
        {
            format!(
                "artist:\"{}\"",
                lucene_escape(&remove_parentheses_content(&props.artist))
            )
        } else {
            String::new()
        },
        if !MB_EXCLUDED_NAMES
            .iter()
            .any(|&name| props.album.contains(name))
        {
            format!("release:\"{}\"", lucene_escape(&props.album))
        } else {
            format!("recording:\"{}\"", lucene_escape(&props.name))
        },
    ]
    .into_iter()
    .filter(|s| !s.is_empty())
    .collect();

    let query = query_terms.join(" ");
    println!("query: {:#?}", query);

    let params = form_urlencoded::Serializer::new(String::new())
        .append_pair("fmt", "json")
        .append_pair("limit", "10")
        .append_pair("query", &query)
        .finish();

    let url = format!("https://musicbrainz.org/ws/2/release?{}", params);
    println!("url: {:#?}", url);

    let response = http_client
        .get(&url)
        .header("User-Agent", "rust/apple-music-discord-rs")
        .header("Accept", "application/json")
        .send()?;

    let responses: MusicBrainzResponse = response.json()?;

    for release in responses.releases {
        let cover_art_url = format!("https://coverartarchive.org/release/{}/front", release.id);
        let response = http_client.head(&cover_art_url).send()?;
        if response.status().is_success() {
            return Ok(Some(cover_art_url));
        }
    }

    Ok(None)
}

fn lucene_escape(term: &str) -> String {
    let special_chars = [
        '+', '-', '&', '|', '!', '(', ')', '{', '}', '[', ']', '^', '"', '~', '*', '?', ':', '\\',
    ];
    let mut result = String::with_capacity(term.len() * 2);
    for c in term.chars() {
        if special_chars.contains(&c) {
            result.push('\\');
        }
        result.push(c);
    }
    result
}

fn remove_parentheses_content(term: &str) -> String {
    term.chars()
        .scan(0, |depth, c| {
            match c {
                '(' => *depth += 1,
                ')' if *depth > 0 => *depth -= 1,
                _ if *depth == 0 => return Some(Some(c)),
                _ => {}
            }
            Some(None)
        })
        .flatten()
        .collect::<String>()
        .trim()
        .to_string()
}

fn truncate_string(value: &str) -> String {
    let max_length: usize = 128;
    if value.len() <= max_length {
        value.to_string()
    } else {
        let mut truncated = value.to_string();
        truncated.truncate(max_length - 3);
        truncated.push_str("...");
        truncated
    }
}

fn update_presence(
    client: &mut Client,
    props: &MusicProps,
    artwork_url: Option<String>,
) -> Result<(), AppError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| AppError::Other(e.to_string()))?
        .as_secs();

    let start_time = now.saturating_sub(props.player_position as u64);
    let end_time = start_time + props.duration as u64;
    println!("start_time {}, end_time {}", start_time, end_time);
    client.set_activity(|act| {
        act._type(ActivityType::Listening)
            .state(truncate_string(&props.artist))
            .details(truncate_string(&props.name))
            .assets(|assets| {
                assets
                    .large_text(&props.album)
                    .large_image(artwork_url.as_deref().unwrap_or("appicon"))
            })
            .timestamps(|timestamps| timestamps.start(start_time).end(end_time))
            .append_buttons(|b| b.label("Open Apple Music").url("https://music.apple.com"))
    })?;

    Ok(())
}
