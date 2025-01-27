use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
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
