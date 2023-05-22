use std::{collections::HashMap, sync::Arc, unreachable, path::{PathBuf, Path}, unimplemented};
use std::str;

use tokio::{sync::{Semaphore, oneshot, mpsc}, process::Command, fs};

use crate::seventv::{download_emote, SevenUserEmote};

enum EmoteStatus {
    Pending(Arc<Semaphore>), // downloading/converting
    Ready,
}


enum EmotePullerMessage {
    PullEmote {
        emote: SevenUserEmote,
        sender_cb: oneshot::Sender<Result<(), String>>,
    }
}

pub struct EmotePuller {
    emote_map: HashMap<String, EmoteStatus>,
    receiver: mpsc::Receiver<EmotePullerMessage>,
}

impl EmotePuller {
    fn new(receiver: mpsc::Receiver<EmotePullerMessage>) -> Self {
        Self {
            receiver,
            emote_map: HashMap::new(),
        }
    }

    fn get_emote_filename(emote: &SevenUserEmote) -> String {
        let id = emote.id.as_str();
        if emote.animated {
            format!("{id}.gif")
        } else {
            format!("{id}.webp")
        }
    }

    pub fn get_pulled_emote_path(emote: &SevenUserEmote) -> String {
        let mut path = PathBuf::from("./emotes");
        path.push(Self::get_emote_filename(emote));
        path.to_string_lossy().to_string()
    }

    async fn emote_file_exists(emote: &SevenUserEmote) -> bool {
        fs::metadata(Self::get_pulled_emote_path(emote))
            .await
            .is_ok()
    }

    async fn process_emote(emote: &SevenUserEmote, temp_path: &Path) -> Result<(), String> {
        println!("Processing emote: {}", emote.name);

        let from = {
            if emote.animated {
                println!("Converting animated emote {}", emote.name);
                let output = Command::new("magick")
                    .arg("mogrify")
                    .arg("-format")
                    .arg("gif")
                    .arg(temp_path.to_path_buf().into_os_string())
                    .output()
                    .await
                    .map_err(|x| x.to_string())?;
                println!("Done converting animated emote {} {:?} {:?}", emote.name, str::from_utf8(&output.stdout), str::from_utf8(&output.stderr));

                let mut path = temp_path.parent().expect("Emote file should have parent directory").to_path_buf();
                path.push(Self::get_emote_filename(emote));
                path
            } else {
                temp_path.to_path_buf()
            }
        };

        let to = Self::get_pulled_emote_path(emote);
        
        println!("Moving emote {} {:?} {:?}", emote.name, from, to);
        fs::rename(from, to)
            .await
            .map_err(|x| x.to_string())?;
        println!("Done processing emote {}", emote.name);
        
        Ok(())
    }

    async fn load_emote(&mut self, emote: &SevenUserEmote) -> Result<(), String> {
        let emote_status = self.emote_map.get(&emote.id);

        if let None = emote_status {
            if Self::emote_file_exists(emote).await {
                self.emote_map.insert(emote.id.to_owned(), EmoteStatus::Ready);
                return Ok(());
            }

            let semaphore = Arc::new(Semaphore::new(1));
            self.emote_map.insert(emote.id.to_owned(), EmoteStatus::Pending(semaphore.clone()));
            let _ = semaphore.acquire()
                .await
                .map_err(|x| x.to_string())?;
            semaphore.close();

            // Actually load emote

            let temp_path = download_emote(&emote.id).await?;
            Self::process_emote(emote, &temp_path).await?;

            semaphore.close();
            self.emote_map.insert(emote.id.to_owned(), EmoteStatus::Ready);
            
            return Ok(());
        }

        if let Some(status) = emote_status {
            if let EmoteStatus::Ready = status {
                return Ok(());
            }

            if let EmoteStatus::Pending(semaphore) = status {
                match semaphore.acquire().await {
                    Ok(_) => panic!("Should never be able to acquire semaphore ticket"),
                    Err(_) => {
                        return Ok(())
                    },
                }
            }
        }

        unreachable!();
    }

    async fn handle_message(&mut self, msg: EmotePullerMessage) {
        match msg {
            EmotePullerMessage::PullEmote { sender_cb, emote } => {
                let emote = self.load_emote(&emote).await;
                sender_cb.send(emote).expect("Should send response");
            },
        }
    }
}

async fn run_emote_puller(mut ep: EmotePuller) {
    while let Some(msg) = ep.receiver.recv().await {
        ep.handle_message(msg).await;
    }
}

pub struct EmotePullerHandle {
    sender: mpsc::Sender<EmotePullerMessage>,
}

impl EmotePullerHandle {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(50);
        let actor = EmotePuller::new(rx);
        tokio::spawn(run_emote_puller(actor));

        Self { sender: tx }
    }

    pub async fn pull_emote(
        &self,
        emote: SevenUserEmote,
    ) -> Result<(), String> {
        let (tx, rx) = oneshot::channel();

        let msg = EmotePullerMessage::PullEmote {
            sender_cb: tx,
            emote,
        };

        let _ = self.sender.send(msg).await;
        rx.await.expect("Task has been killed")
    }
}
