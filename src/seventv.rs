use serde::{Deserialize, Serialize};
use reqwest;

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

#[derive(Debug, Serialize, Deserialize)]
pub struct SevenUserEmote {
    pub id: String,
    pub name: String,
}

pub async fn get_twitch_user_emote_set(twitch_id: &str) -> Result<SevenUserEmoteSet, String> {
    let resp = reqwest::get(format!("https://7tv.io/v3/users/twitch/{twitch_id}"))
        .await
        .map_err(|x| x.to_string())?;

    let json = resp
        .json::<SevenUserData>()
        .await
        .map_err(|x| x.to_string())?;

    Ok(json.emote_set)
}
