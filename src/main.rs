mod http;
mod twitch;
mod emote;
mod seventv;
mod utils;
mod emote_puller;

use std::{io::{self, BufRead}, env, sync::Arc, unimplemented, time::Duration};
use emote_puller::{EmotePullerHandle, EmotePuller};
use tokio::{net::{TcpListener, TcpStream}, io::{AsyncWriteExt, BufWriter, ReadBuf, BufReader, AsyncReadExt}, fs};
use tokio::time::sleep;
use dotenv::dotenv;
use twitch::TwitchClient;
use emote::EmoteManagerHandle;
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

    let emote_manager = Arc::new(EmoteManagerHandle::new());

    let emote_puller = Arc::new(EmotePullerHandle::new());

    loop {
        let (mut socket, _) = listener.accept().await?;
        let ip = match socket.peer_addr() {
            Err(_) => {
                println!("[ERROR]: Failed to get client IP");
                continue;
            },
            Ok(ip) => ip,
        };

        println!("[INFO]: Received request from {}", ip);

        let emote_manager_instance = emote_manager.clone();
        let twitch_client_instance = twitch_client.clone();
        let emote_puller_instance = emote_puller.clone();

        tokio::spawn(async move {
            match serve_request(
                emote_manager_instance,
                emote_puller_instance,
                twitch_client_instance,
                &mut socket,
            ).await {
                Ok(_) => {
                    println!("[INFO]: Sent response");
                },
                Err(err) => {
                    println!("[ERROR]: Failed to generate response {:?}", err);
                },
            }
        });
    }
}

struct ParsedRequest {
    emotes: Vec<String>,
    twitch_username: Option<String>,
}

fn parse_request(pathname: &str) -> Option<ParsedRequest> {
    let parts: Vec<String> = pathname.trim_matches('/').split('/').map(|x| x.to_owned()).collect();

    if parts.len() == 1 {
        let emote = parts.get(0)?.to_owned().trim_end_matches(".gif").to_owned();

        return Some(ParsedRequest {
            emotes: vec![emote],
            twitch_username: None,
        });
    }

    if parts.len() == 2 {
        let username = parts.get(0)?.to_owned();
        let emote = parts.get(0)?.to_owned().trim_end_matches(".gif").to_owned();

        return Some(ParsedRequest {
            emotes: vec![emote],
            twitch_username: Some(username),
        });
    }

    None
}

async fn serve_request(
    emote_manager: Arc<EmoteManagerHandle>,
    emote_puller: Arc<EmotePullerHandle>,
    twitch_client: Arc<TwitchClient>,
    stream: &mut TcpStream,
) -> Result<(), String> {
    let (raw_reader, raw_writer) = stream.split();
    let mut reader = BufReader::new(raw_reader);
    let mut buffer: Vec<u8> = vec![0; 1000];
    reader.read(&mut buffer)
        .await
        .map_err(|_| "Failed to read request data".to_owned())?;

    let mut writer = BufWriter::new(raw_writer);


    // sleep(Duration::from_secs(2)).await;

    let http_request = HttpRequest::parse(&buffer)?;

    let ParsedRequest { emotes, twitch_username } = parse_request(&http_request.pathname)
        .ok_or("[ERROR]: Couldn't parse request pathname".to_owned())?;

    println!("{:?}", emotes);

    let emote_keyword = emotes.into_iter().nth(0).unwrap();

    let emote = async {
        if let Some(username) = twitch_username {
            let twitch_id = match twitch_client.get_id_for_username(&username).await {
                Ok(twitch_id) => twitch_id,
                Err(err) => {
                    println!("[ERROR]: Something happened while getting twitch id for username {username} {err}");

                    return Err("Failed to get twitch ID".to_owned());
                },
            };

            let emote = emote_manager.get_user_emote(&twitch_id, &emote_keyword).await?;
            Ok(emote)
        } else {
            let emote = emote_manager.get_popular_emote(&emote_keyword).await?;
            Ok(emote)
        }
    }.await?;

    emote_puller.pull_emote(emote.clone()).await?;

    let mime = {
        if emote.animated {
            "image/gif"
        } else {
            "image/webp"
        }
    };

    println!("Reading emote file");
    println!("{:?}", EmotePuller::get_pulled_emote_path(&emote) );
    let emote_file = fs::read(EmotePuller::get_pulled_emote_path(&emote))
        .await
        .map_err(|x| x.to_string())?;
    let emote_size = emote_file.len();
    println!("File size {emote_size}");

    // Refactor into HttpResponse struct
    let response_head = format!("HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {emote_size}\r\n\r\n");
    writer.write(response_head.as_bytes())
        .await
        .map_err(|x| x.to_string())?;
    writer.write_all(&emote_file)
        .await
        .map_err(|x| x.to_string())?;

    writer.shutdown()
        .await
        .map_err(|x| x.to_string())?;

    Ok(())
}
