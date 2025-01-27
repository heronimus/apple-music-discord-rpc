#[derive(Debug, Clone)]
pub struct MusicProps {
    pub name: String,
    pub artist: String,
    pub album: String,
    pub duration: f64,
    pub player_position: f64,
}
