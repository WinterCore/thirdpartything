mod http;
mod emote;

use std::{io, str, env, sync::Arc};
use tokio::net::{TcpListener, TcpStream};
use dotenv::dotenv;
use emote::TwitchClient;
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

        match serve_request(twitch_client.clone(), &mut socket).await {
            Ok(_) => {
            },
            Err(err) => {
                println!("[ERROR]: Failed to generate response {:?}", err);
            },
        }
    }
}

async fn serve_request(
    twitch_client: Arc<TwitchClient>,
    stream: &mut TcpStream,
) -> io::Result<()> {
    let (reader, writer) = stream.split();
    let mut buffer: Vec<u8> = vec![0; 1000];
    reader.try_read(&mut buffer)?;

    let request = HttpRequest::parse(&buffer);

    twitch_client
        .get_id_for_username("winterrcore")
        .await;

    Ok(())
}
