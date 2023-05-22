use std::collections::HashMap;

use tokio::sync::{oneshot, mpsc};

use crate::{seventv::{get_twitch_user_emote_set, get_most_popular_emote, SevenUserEmote}, utils::now_secs};

struct TwitchUserEmoteMap {
    last_updated: u64, // seconds
    map: HashMap<
        String, // keyword
        SevenUserEmote,
    >,
}

struct EmoteManager {
    receiver: mpsc::Receiver<EmoteManagerMessage>,
    twitch_id_emotes_map: HashMap<String, TwitchUserEmoteMap>,
    popular_emote_map: HashMap<String, SevenUserEmote>,
}

enum EmoteManagerMessage {
    GetUserEmoteByKeyword {
        sender_cb: oneshot::Sender<Result<SevenUserEmote, String>>,
        twitch_id: String,
        emote_keyword: String,
    },
    GetPopularEmoteByKeyword {
        sender_cb: oneshot::Sender<Result<SevenUserEmote, String>>,
        emote_keyword: String,
    },
}

impl EmoteManager {
    // in seconds
    const USER_EMOTE_RELOAD_COOLDOWN: u64 = 10 * 60;
                                                     
    fn new(receiver: mpsc::Receiver<EmoteManagerMessage>) -> Self {
        Self {
            receiver,
            twitch_id_emotes_map: HashMap::new(),
            popular_emote_map: HashMap::new(),
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
        let emote_map = TwitchUserEmoteMap {
            last_updated: now_secs(),
            map: HashMap::from_iter(
                emote_set
                .emotes
                .into_iter()
                .map(|x| (x.name.to_owned(), x))
            ),
        };

        self.twitch_id_emotes_map.insert(twitch_id.to_owned(), emote_map);

        Ok(true)
    }

    async fn get_user_emote(
        &mut self,
        twitch_id: &str,
        emote_keyword: &str,
    ) -> Result<SevenUserEmote, String> {
        self.load_user_emotes(twitch_id, false).await?;

        let map = match self.twitch_id_emotes_map.get(twitch_id) {
            Some(TwitchUserEmoteMap { map, last_updated }) => map,
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
            Some(TwitchUserEmoteMap { map, last_updated }) => map,
            None => return Err("7tv: User not found".to_owned()),
        };

        if let Some(emote) = map.get(emote_keyword) {
            return Ok(emote.clone());
        }

        Err("Emote not found".to_owned())
    }

    // Gets most popular emote by keyword
    async fn get_emote(&mut self, emote_keyword: &str) -> Result<SevenUserEmote, String> {
        let emote_id = match self.popular_emote_map.get(emote_keyword) {
            Some(emote) => emote.clone(),
            None => {
                let emote = get_most_popular_emote(emote_keyword).await?;
                self.popular_emote_map.insert(emote_keyword.to_owned(), emote.clone());

                emote
            },
        };

        Ok(emote_id)
    }

    async fn handle_message(&mut self, msg: EmoteManagerMessage) {
        match msg {
            EmoteManagerMessage::GetUserEmoteByKeyword {
                sender_cb,
                twitch_id,
                emote_keyword,
            } => {
                let emote = self.get_user_emote(&twitch_id, &emote_keyword).await;
                sender_cb.send(emote).expect("Should send response");
            },
            EmoteManagerMessage::GetPopularEmoteByKeyword {
                sender_cb,
                emote_keyword,
            } => {
                let emote = self.get_emote(&emote_keyword).await;
                sender_cb.send(emote).expect("Should send response");
            },
        }
    }
}


async fn run_emote_manager(mut em: EmoteManager) {
    while let Some(msg) = em.receiver.recv().await {
        em.handle_message(msg).await;
    }
}

#[derive(Clone)]
pub struct EmoteManagerHandle {
    sender: mpsc::Sender<EmoteManagerMessage>,
}

impl EmoteManagerHandle {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(50);
        let actor = EmoteManager::new(rx);
        tokio::spawn(run_emote_manager(actor));

        Self { sender: tx }
    }

    pub async fn get_user_emote(
        &self,
        twitch_id: &str,
        emote_keyword: &str,
    ) -> Result<SevenUserEmote, String> {
        let (tx, rx) = oneshot::channel();

        let msg = EmoteManagerMessage::GetUserEmoteByKeyword {
            sender_cb: tx,
            twitch_id: twitch_id.to_owned(),
            emote_keyword: emote_keyword.to_owned(),
        };

        let _ = self.sender.send(msg).await;
        rx.await.expect("Task has been killed")
    }

    pub async fn get_popular_emote(
        &self,
        emote_keyword: &str,
    ) -> Result<SevenUserEmote, String> {
        let (tx, rx) = oneshot::channel();

        let msg = EmoteManagerMessage::GetPopularEmoteByKeyword {
            sender_cb: tx,
            emote_keyword: emote_keyword.to_owned(),
        };

        let _ = self.sender.send(msg).await;
        rx.await.expect("Task has been killed")
    }
}
