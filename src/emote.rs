use std::{time::{self, UNIX_EPOCH}, collections::HashMap};
use tokio::sync::RwLock;
use reqwest::{self, Client};

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

        let auth_token_read_lock = self.auth_token.read().await;

        if let Some((token, expires_at)) = auth_token_read_lock. {
            if now < expires_at {
                return Ok(token.clone());
            }
        }

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

        let response_result = client.post("https://id.twitch.tv/oauth2/token")
            .query(&query)
            .send()
            .await;
        
        let response_result = match response_result {
            Ok(resp) => resp,
            Err(err) => return Err(err.to_string()),
        };

        let json = match response_result.json::<HashMap<String, String>>().await {
            Ok(json) => json,
            Err(err) => return Err(err.to_string()),
        };

        let token = json.get("access_token")
            .ok_or("Invalid response".to_owned())?;
        let expires_in = json.get("expires_in")
            .and_then(|x| x.parse::<u64>().ok())
            .ok_or("Invalid response".to_owned())?;

        let now = time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();
        let expires_at = now + expires_in;

        self.auth_token = Some(token.clone());
        self.auth_token_expires = expires_at;
        

        Ok(token.clone())
    }

    pub async fn get_id_for_username(&self, username: &str) -> Result<String, String> {
        let auth_token = self.get_auth_token().await?;

        let client = Client::new();
        let response = client.get("https://api.twitch.tv/helix/users")
            .header("Client-Id", &self.client_id)
            .header("Authorization", auth_token)
            .query(&[("login", &username)])
            .send()
            .await;

        match response {
            Ok(resp) => {
                println!("{:?} {:?}", resp.status(), resp.text().await.unwrap());
            },
            Err(err) => {
                println!("Error {:?}", err);
            },
        }

        // println!("Get User ID Response: {:?}", response);

        Ok("fjasdkf".to_owned())
    }
}



/*
pub fn get_emote_path(username: &str, emote: &str) -> PathBuf {
}
*/
