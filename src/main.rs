mod http;
mod twitch;
mod emote;
mod seventv;
mod utils;

use std::{io, env, sync::Arc};
use tokio::{net::{TcpListener, TcpStream}, sync::{mpsc::{self, Sender}, oneshot}};
use dotenv::dotenv;
use twitch::TwitchClient;
use emote::{EmoteManager, EmoteManagerMessage};
// use tokio::io::BufReader;

use http::HttpRequest;

#[tokio::main]
async fn main() -> io::Result<()> {
    dotenv().ok();

    let twitch_client_id = env::var("TWITCH_CLIENT_ID")
        .expect("TWITCH_CLIENT_ID env variable is present!");
    let twitch_client_secret = env::var("TWITCH_CLIENT_SECRET")
        .expect("TWITCH_CLIENT_SECRET env variable is present!");

    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    let twitch_client = Arc::new(TwitchClient::new(
        twitch_client_id,
        twitch_client_secret,
    ));

    let mut emote_manager = EmoteManager::new();
    

    let (tx, mut rx) = mpsc::channel::<EmoteManagerMessage>(4);

    tokio::spawn(async move {

        while let Some(msg) = rx.recv().await {
            emote_manager.handle_message(msg).await;
        }
    });

    loop {
        let (mut socket, _) = listener.accept().await?;
        let ip = match socket.peer_addr() {
            Err(e) => {
                println!("[ERROR]: Failed to get client IP");
                continue;
            },
            Ok(ip) => ip,
        };

        println!("[INFO]: Received request from {}", ip);

        match serve_request(
            tx.clone(),
            twitch_client.clone(),
            &mut socket
        ).await {
            Ok(_) => {
            },
            Err(err) => {
                println!("[ERROR]: Failed to generate response {:?}", err);
            },
        }
    }
}

async fn serve_request(
    emote_manager_tx: Sender<EmoteManagerMessage>,
    twitch_client: Arc<TwitchClient>,
    stream: &mut TcpStream,
) -> Result<(), String> {
    let (reader, writer) = stream.split();
    let mut buffer: Vec<u8> = vec![0; 1000];
    reader.try_read(&mut buffer)
        .map_err(|_| "Failed to read request data".to_owned())?;

    let request = HttpRequest::parse(&buffer);
    let username = "winterrcore";

    let twitch_id = match twitch_client.get_id_for_username("winterrcore").await {
        Ok(twitch_id) => twitch_id,
        Err(err) => {
            println!("[ERROR]: Something happened while getting twitch id for username {username} {err}");

            return Err("Failed to get twitch ID".to_owned());
        },
    };
    

    // TODO: Make this better
    let (send, recv) = oneshot::channel();
    let msg = EmoteManagerMessage::GetUserEmoteByKeyword {
        sender_cb: send,
        twitch_id,
        emote_keyword: "docnotL".to_owned(),
    };

    let _ = emote_manager_tx.send(msg).await;
    let emote_id = recv.await.expect("Should receive response")?;
    println!("Found emote {emote_id}");

    Ok(())
}
