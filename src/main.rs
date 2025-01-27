use discord_presence::models::rich_presence::ActivityType;
use discord_presence::Client;
use reqwest::blocking::{Client as HttpClient, ClientBuilder};
use serde::Deserialize;
use std::cell::RefCell;
use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use url::form_urlencoded;

use objc2::rc::{autoreleasepool, Retained};
use objc2::{define_class, msg_send, sel, AllocAnyThread, DeclaredClass};
use objc2_foundation::{
    ns_string, NSCopying, NSDate, NSDefaultRunLoopMode, NSObject, NSObjectProtocol, NSPort,
    NSRunLoop, NSString,
};
use objc2_foundation::{NSNotification, NSNotificationCenter};
use objc2_media_player::{
    MPMediaEntityPersistentID, MPMediaPlayback, MPMusicPlaybackState, MPMusicPlayerController,
};

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

struct MusicPlayerObserverIvars {
    object: Retained<MPMusicPlayerController>,
    // key_path: Retained<NSString>,
    playback_state_notification: Retained<NSString>,
    now_playing_item_notification: Retained<NSString>,
    http_client: HttpClient,
    discord_client: RefCell<Client>,
    previous_index: RefCell<MPMediaEntityPersistentID>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "MusicPlayerObserver"]
    #[ivars = MusicPlayerObserverIvars]
    struct MusicPlayerObserver;

    impl MusicPlayerObserver {

        #[unsafe(method(handlePlaybackStateChange:))]
        unsafe fn handle_playback_state_change(&self, notification: &NSNotification) {
            println!("<--->  Playback state changed");

            if let Some(object) = &notification.object() {
                // Cast the notification object to MPMusicPlayerController
                let player: &MPMusicPlayerController =  msg_send![object, self];

                let previous_media_id = *self.ivars().previous_index.borrow();

                match player.nowPlayingItem() {
                    Some(item) => {
                        if item.persistentID() != previous_media_id {
                            println!("<---> MediaItemPersistentID changed from {} to {}", previous_media_id, item.persistentID() );
                            // Store the new ID
                            *self.ivars().previous_index.borrow_mut() = item.persistentID();

                            //Update activity on changes
                            if let Err(e) = discord_update_activity(player, &mut self.ivars().discord_client.borrow_mut(), &self.ivars().http_client) {
                                eprintln!("DISCOR_RPC: error in discord_update_activity: {}", e);
                            }
                        }

                        println!("<--->    -- Playing Item Title: {:#?}",item.title().unwrap());
                        println!("<--->    -- Playing Item ID: {:#?}",item.persistentID());
                        println!("<--->    -- Playing Item Duration: {:#?}",item.playbackDuration());
                    }
                    None => {
                        println!("<--->    -- No Playing Item");
                    }
                }

                // Console Debug Section
                match player.playbackState() {
                        MPMusicPlaybackState::Playing => {
                            println!("<--->    -- player playbackState: playing {:#?}",player.playbackState().0);
                        }
                        MPMusicPlaybackState::Paused => {
                            println!("<--->    -- player playbackState: paused {:#?}",player.playbackState().0);
                        }
                        MPMusicPlaybackState::Stopped => {
                            println!("<--->    -- player playbackState: stopped {:#?}",player.playbackState().0);
                        }
                        _ => {
                            println!("<--->    -- player playbackState: unknown state {:#?}",player.playbackState().0);
                        }
                    }
                println!("<--->    -- indexOfNowPlayingItem {:#?}",player.indexOfNowPlayingItem());
                println!("<--->    -- currentPlaybackTime: {:#?}",player.currentPlaybackTime());


            }
        }

        #[unsafe(method(handleNowPlayingItemChange:))]
        unsafe fn handle_now_playing_item_change(&self, notification: &NSNotification) {
            println!("<---> Now playing item / playlist changed");

            if let Some(object) = notification.object() {
                // Cast the notification object to MPMusicPlayerController
                let player: &MPMusicPlayerController = msg_send![&object, self];

                match player.nowPlayingItem() {
                    Some(item) => {
                        println!("<--->  Playing Item Title{:#?}",item.title().unwrap());
                    }
                    None => {
                        println!("<--->  No Playing Item");
                    }
                }
            }
        }
    }

    unsafe impl NSObjectProtocol for MusicPlayerObserver {}
);

impl MusicPlayerObserver {
    unsafe fn new() -> Retained<Self> {
        let observer = Self::alloc().set_ivars(MusicPlayerObserverIvars {
            object: MPMusicPlayerController::systemMusicPlayer(),
            playback_state_notification: ns_string!(
                "MPMusicPlayerControllerPlaybackStateDidChangeNotification"
            )
            .copy(),
            now_playing_item_notification: ns_string!(
                "MPMusicPlayerControllerNowPlayingItemDidChangeNotification"
            )
            .copy(),
            discord_client: RefCell::new(Client::new(773825528921849856)),
            http_client: ClientBuilder::new()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap(),
            previous_index: RefCell::new(MPMediaEntityPersistentID::from_be(0)),
        });
        let observer: Retained<Self> = unsafe { msg_send![super(observer), init] };

        observer
            .ivars()
            .discord_client
            .borrow()
            .on_ready(|_ctx| {
                println!("Discord RPC connected!");
            })
            .persist();

        observer.ivars().discord_client.borrow_mut().start();

        // Add notification observers
        unsafe {
            let notification_center = NSNotificationCenter::defaultCenter();

            // Add observer for playback state changes
            let _ = notification_center.addObserver_selector_name_object(
                &*observer,
                sel!(handlePlaybackStateChange:),
                Some(&observer.ivars().playback_state_notification),
                Some(&*observer.ivars().object),
            );

            // Add observer for now playing item changes
            let _ = notification_center.addObserver_selector_name_object(
                &*observer,
                sel!(handleNowPlayingItemChange:),
                Some(&observer.ivars().now_playing_item_notification),
                Some(&*observer.ivars().object),
            );

            // Start generating notifications
            observer
                .ivars()
                .object
                .beginGeneratingPlaybackNotifications();
        }

        observer
    }
}

impl Drop for MusicPlayerObserver {
    fn drop(&mut self) {
        unsafe {
            //Clear Discord Activity
            if let Err(e) = self.ivars().discord_client.borrow_mut().clear_activity() {
                eprintln!("DEBUG: error in clear_activity: {}", e);
            };
            println!("Disconnected from Discord RPC.");

            // Remove notification observers
            let notification_center = NSNotificationCenter::defaultCenter();
            notification_center.removeObserver_name_object(
                self,
                Some(&self.ivars().playback_state_notification),
                Some(&*self.ivars().object),
            );
            notification_center.removeObserver_name_object(
                self,
                Some(&self.ivars().now_playing_item_notification),
                Some(&self.ivars().object),
            );

            // Stop generating notifications
            self.ivars().object.endGeneratingPlaybackNotifications();
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        println!("Shutting down client...");
        r.store(false, Ordering::SeqCst);
    })?;

    unsafe {
        println!("DEBUG: Registering Observer");
        let dummy_player = MPMusicPlayerController::systemMusicPlayer();
        let _observer = MusicPlayerObserver::new();

        let run_loop = NSRunLoop::currentRunLoop();

        // Add a port to the run loop to keep it active
        let port = NSPort::port(); // or create a new port
        run_loop.addPort_forMode(&port, NSDefaultRunLoopMode);

        while running.load(Ordering::SeqCst) {
            autoreleasepool(|_| {
                // Use a shorter interval to be more responsive
                let date = NSDate::dateWithTimeIntervalSinceNow(5.00);

                // Use runMode with a shorter timeout
                run_loop.runMode_beforeDate(NSDefaultRunLoopMode, &date);
                // println!("--- Loop {:#?}", obs_player.playbackState());

                _ = dummy_player.indexOfNowPlayingItem();
            });

            // Optional: Small sleep to prevent CPU spinning
            // std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    Ok(())
}

unsafe fn discord_update_activity(
    player: &MPMusicPlayerController,
    discord_client: &mut Client,
    http_client: &HttpClient,
) -> Result<(), AppError> {
    match music_player_get_props(player) {
        Ok(props) => {
            let artwork_url = match itunes_search(http_client, &props) {
                Ok(Some(url)) => Some(url),
                _ => match get_artwork_musicbrainz(http_client, &props) {
                    Ok(Some(url)) => Some(url),
                    _ => None,
                },
            };

            discord_update_presence(discord_client, &props, artwork_url)?;
        }
        Err(AppError::NoSongPlaying) => {
            println!("DEBUG: No song playing");
            discord_client.clear_activity()?;
        }
        Err(e) => return Err(e),
    }
    Ok(())
}

unsafe fn music_player_get_props(player: &MPMusicPlayerController) -> Result<MusicProps, AppError> {
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
            let player_position = 0.0;

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

    // println!("DEBUG: music_player_get_props returning: {:?}", props);
    Ok(props)
}

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

fn discord_update_presence(
    discord_client: &mut Client,
    props: &MusicProps,
    artwork_url: Option<String>,
) -> Result<(), AppError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| AppError::Other(e.to_string()))?
        .as_secs();

    let start_time = now.saturating_sub(props.player_position as u64);
    let end_time = start_time + props.duration as u64;
    println!(
        ">> DISCORD RPC: start_time {}, end_time {}",
        start_time, end_time
    );
    discord_client.set_activity(|act| {
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
