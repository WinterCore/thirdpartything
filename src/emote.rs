use std::collections::HashMap;

use tokio::sync::oneshot;

use crate::{seventv::get_twitch_user_emote_set, utils::now_secs};

struct EmoteMap {
    last_updated: u64, // seconds
    map: HashMap<
        String, // keyword
        String, // id
    >,
}

pub struct EmoteManager {
    twitch_id_emotes_map: HashMap<String, EmoteMap>,
}

pub enum EmoteManagerMessage {
    GetUserEmoteByKeyword {
        sender_cb: oneshot::Sender<Result<String, String>>,
        twitch_id: String,
        emote_keyword: String,
    },
}

impl EmoteManager {
    // in seconds
    const USER_EMOTE_RELOAD_COOLDOWN: u64 = 10 * 60;
                                                     
    pub fn new() -> Self {
        Self {
            twitch_id_emotes_map: HashMap::new(),
        }
    }
    
    async fn load_user_emotes(
        &mut self,
        twitch_id: &str,
        reload: bool,
    ) -> Result<bool, String> {
        let existing = self.twitch_id_emotes_map.get(twitch_id);

        if existing.is_some() && ! reload {
            return Ok(false);
        }
        
        if let Some(map) = existing {
            if now_secs() > map.last_updated + Self::USER_EMOTE_RELOAD_COOLDOWN {
                return Ok(false);
            }
        }

        let emote_set = get_twitch_user_emote_set(&twitch_id).await?;
        println!("Emote set {:?}", emote_set);
        let emote_map = EmoteMap {
            last_updated: now_secs(),
            map: HashMap::from_iter(
                emote_set
                .emotes
                .into_iter()
                .map(|x| (x.name, x.id))
            ),
        };

        self.twitch_id_emotes_map.insert(twitch_id.to_owned(), emote_map);

        Ok(true)
    }

    async fn get_user_emote(
        &mut self,
        twitch_id: &str,
        emote_keyword: &str,
    ) -> Result<String, String> {
        self.load_user_emotes(twitch_id, false).await?;

        let map = match self.twitch_id_emotes_map.get(twitch_id) {
            Some(EmoteMap { map, last_updated }) => map,
            None => return Err("7tv: User not found".to_owned()),
        };

        if ! map.contains_key(emote_keyword) {
            drop(map);
            if ! self.load_user_emotes(twitch_id, true).await? {
                return Err("Emote not found".to_owned());
            }
        }

        // TODO: Refactor following duplicate code
        let map = match self.twitch_id_emotes_map.get(twitch_id) {
            Some(EmoteMap { map, last_updated }) => map,
            None => return Err("7tv: User not found".to_owned()),
        };

        if let Some(emote_id) = map.get(emote_keyword) {
            return Ok(emote_id.clone());
        }

        Err("Emote not found".to_owned())
    }

    pub async fn handle_message(&mut self, msg: EmoteManagerMessage) {
        match msg {
            EmoteManagerMessage::GetUserEmoteByKeyword {
                sender_cb: cb,
                twitch_id,
                emote_keyword,
            } => {
                let emote = self.get_user_emote(&twitch_id, &emote_keyword).await;
                cb.send(emote).expect("Should send response");
            },
        }
    }
}
