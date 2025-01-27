use crate::error::AppError;
use crate::handlers::{get_artwork_itunes, get_artwork_musicbrainz, get_music_props};
use crate::models::MusicProps;
use crate::utils::truncate_string;

use discord_presence::models::rich_presence::ActivityType;
use discord_presence::Client;
use objc2_media_player::MPMusicPlayerController;
use reqwest::blocking::Client as HttpClient;

pub unsafe fn update_discord_activity(
    player: &MPMusicPlayerController,
    discord_client: &mut Client,
    http_client: &HttpClient,
) -> Result<(), AppError> {
    match get_music_props(player) {
        Ok(props) => {
            let artwork_url = match get_artwork_itunes(http_client, &props) {
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
