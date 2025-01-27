use crate::error::AppError;
use crate::models::{ArtworkITunesSearchResponse, ArtworkMusicBrainzResponse, MusicProps};
use crate::utils::{lucene_escape, remove_parentheses_content};
use reqwest::blocking::Client as HttpClient;
use url::form_urlencoded;

pub fn get_artwork_itunes(
    http_client: &HttpClient,
    props: &MusicProps,
) -> Result<Option<String>, AppError> {
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

    let responses: ArtworkITunesSearchResponse = response.json()?;

    if responses.result_count == 1 {
        println!(
            "DEBUG: TrackViewURL: {}",
            responses.results[0].track_view_url
        );
        Ok(Some(responses.results[0].artwork_url100.clone()))
    } else if responses.result_count > 1 {
        // If there are multiple results, find the right album
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

pub fn get_artwork_musicbrainz(
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

    let responses: ArtworkMusicBrainzResponse = response.json()?;

    for release in responses.releases {
        let cover_art_url = format!("https://coverartarchive.org/release/{}/front", release.id);
        let response = http_client.head(&cover_art_url).send()?;
        if response.status().is_success() {
            return Ok(Some(cover_art_url));
        }
    }

    Ok(None)
}
