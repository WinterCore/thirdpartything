use std::{path::PathBuf, env, io::Cursor};

use serde::{Deserialize, Serialize};
use reqwest::{self, Client};
use tokio::{fs, io};

#[derive(Debug, Serialize, Deserialize)]
struct DataResponse<T> {
    data: T,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SevenUserData {
    pub id: String,
    pub emote_set: SevenUserEmoteSet,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SevenUserEmoteSet {
    pub id: String,
    pub name: String,
    pub emotes: Vec<SevenUserEmote>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SevenUserEmote {
    pub id: String,
    pub name: String,
    pub animated: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SevenEmotesData {
    items: Vec<SevenUserEmote>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SevenEmotesDataWrapper {
    emotes: SevenEmotesData,
}

pub async fn get_twitch_user_emote_set(
    twitch_id: &str,
) -> Result<SevenUserEmoteSet, String> {
    let resp = reqwest::get(format!("https://7tv.io/v3/users/twitch/{twitch_id}"))
        .await
        .map_err(|x| x.to_string())?;

    let json = resp
        .json::<SevenUserData>()
        .await
        .map_err(|x| x.to_string())?;

    Ok(json.emote_set)
}

pub async fn get_most_popular_emote(
    emote_keyword: &str,
) -> Result<SevenUserEmote, String> {
    let client = Client::new();
    let resp = client.post("https://7tv.io/v3/gql")
        .header("Content-Type", "application/json")
        .body(format!(r#"{{
            "query": "query SearchEmotes($query: String!) {{ emotes(query: $query filter: {{ case_sensitive: true, exact_match: true }} sort: {{ value: \"popularity\", order: DESCENDING }}) {{ items {{ id name animated }} }} }}",
            "variables": {{
                "query": "{emote_keyword}"
            }}
        }}"#))
        .send()
        .await
        .map_err(|x| x.to_string())?;

    let json = resp
        .json::<DataResponse<SevenEmotesDataWrapper>>()
        .await
        .map_err(|x| x.to_string())?;

    let emote = json.data.emotes.items.into_iter().nth(0);

    match emote {
        Some(emote) => Ok(emote),
        None => Err("Emote not found".to_owned()),
    }
}

pub async fn download_emote(emote_id: &str) -> Result<PathBuf, String> {
    // TODO: size is hardcoded for now
    let url = format!("https://cdn.7tv.app/emote/{emote_id}/4x.webp");
    let response = reqwest::get(url)
        .await
        .map_err(|x| x.to_string())?;
    let bytes = response.bytes()
        .await
        .map_err(|x| x.to_string())?;

    let path = {
        let mut dir = env::temp_dir();
        dir.push(format!("{emote_id}.webp"));
        dir
    };
    let mut file = fs::File::create(path.as_path())
        .await
        .map_err(|x| x.to_string())?;
    
    let mut content = Cursor::new(bytes);

    io::copy(&mut content, &mut file)
        .await
        .map_err(|x| x.to_string())?;
    
    Ok(path)
}
