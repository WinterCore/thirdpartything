use std::{time::{self, UNIX_EPOCH}, collections::HashMap, cell::RefCell};
use tokio::sync::RwLock;
use reqwest::{self, Client};
use serde::{Deserialize, Serialize};

use crate::utils::now_secs;


#[derive(Debug, Serialize, Deserialize)]
struct TwitchAuthDataResponse {
    access_token: String,
    expires_in: u64,
    token_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ResponseDataWrapper<T> {
    data: T,
}

#[derive(Debug, Serialize, Deserialize)]
struct TwitchUserData {
    id: String,
    login: String,
    display_name: String,

    /*
    profile_image_url: String,
    created_at: String,
    */
}

type TwitchUserDataResponse = ResponseDataWrapper<Vec<TwitchUserData>>;

#[derive(Debug)]
pub struct TwitchClient {
    client_id: String,
    client_secret: String,
    auth_token: RwLock<Option<(String, u64)>>,
    twitch_username_id_map: RwLock<HashMap<String, String>>,
}

impl TwitchClient {
    pub fn new(client_id: String, client_secret: String) -> Self {
        Self {
            client_id,
            client_secret,
            auth_token: RwLock::new(None),
            twitch_username_id_map: RwLock::new(HashMap::new()),
        }
    }

    async fn get_auth_token(&self) -> Result<String, String> {
        let now = time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();

        let auth_token = self.auth_token.read().await;

        if let Some((token, expires_at)) = auth_token.as_ref() {
            if now < *expires_at {
                return Ok(token.clone());
            }
        }
        drop(auth_token);

        let token = self.update_auth_token().await?;
        
        Ok(token)
    }

    async fn update_auth_token(&self) -> Result<String, String> {
        let client = Client::new();
        let query = vec![
            ("client_id", self.client_id.as_str()),
            ("client_secret", self.client_secret.as_str()),
            ("grant_type", "client_credentials"),
        ];

        let response = client.post("https://id.twitch.tv/oauth2/token")
            .query(&query)
            .send()
            .await
            .map_err(|x| x.to_string())?;
        
        let json = response
            .json::<TwitchAuthDataResponse>()
            .await
            .map_err(|x| x.to_string())?;

        let token = json.access_token;
        let expires_in = json.expires_in;

        let now = now_secs();
        let expires_at = now + expires_in;

        let mut auth_token = self.auth_token.write().await;
        *auth_token = Some((token.clone(), expires_at));
        println!("Fetched new auth token");

        Ok(token.clone())
    }

    pub async fn get_id_for_username(&self, username: &str) -> Result<String, String> {
        let username_map = self.twitch_username_id_map.read().await;

        if let Some(id) = username_map.get(username) {
            return Ok(id.clone());
        }
        drop(username_map);

        let auth_token = self.get_auth_token().await?;

        let client = Client::new();
        let response = client.get("https://api.twitch.tv/helix/users")
            .header("Client-Id", &self.client_id)
            .header("Authorization", format!("Bearer {auth_token}"))
            .query(&[("login", &username)])
            .send()
            .await
            .map_err(|x| x.to_string())?;

        let json = response
            .json::<TwitchUserDataResponse>()
            .await
            .map_err(|x| x.to_string())?;

        let data = json.data
            .get(0)
            .ok_or("Empty user data")?;

        let id = data.id.clone();

        let mut username_map = self.twitch_username_id_map.write().await;
        username_map.insert(username.to_owned(), id.clone());
        drop(username_map);

        Ok(id.clone())
    }
}
