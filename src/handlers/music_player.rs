use crate::error::AppError;
use crate::models::MusicProps;

use objc2_media_player::MPMusicPlayerController;

pub unsafe fn get_music_props(player: &MPMusicPlayerController) -> Result<MusicProps, AppError> {
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

    Ok(props)
}
