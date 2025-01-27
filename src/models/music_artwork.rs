use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ArtworkITunesSearchResponse {
    #[serde(rename = "resultCount")]
    pub result_count: i32,
    pub results: Vec<ArtworkITunesSearchResult>,
}

#[derive(Debug, Deserialize)]
pub struct ArtworkITunesSearchResult {
    #[serde(rename = "trackName")]
    pub track_name: String,
    #[serde(rename = "collectionName")]
    pub collection_name: String,
    #[serde(rename = "artworkUrl100")]
    pub artwork_url100: String,
    #[serde(rename = "trackViewUrl")]
    pub track_view_url: String,
}

#[derive(Debug, Deserialize)]
pub struct ArtworkMusicBrainzResponse {
    pub releases: Vec<ArtworkMusicBrainzRelease>,
}

#[derive(Debug, Deserialize)]
pub struct ArtworkMusicBrainzRelease {
    pub id: String,
}
