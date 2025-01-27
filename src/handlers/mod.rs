pub mod discord;
pub mod music_artwork;
pub mod music_player;

// Re-exports for convenient access
pub use discord::update_discord_activity;
pub use music_artwork::{get_artwork_itunes, get_artwork_musicbrainz};
pub use music_player::get_music_props;
