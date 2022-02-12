use reqwest::header::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Error, ErrorKind};

use std::net::TcpStream;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{connect, Message};
use url::Url;

#[derive(Deserialize, Debug)]
struct AppsConnectionsOpenResponse {
    ok: bool,
    url: Option<String>,
    error: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
pub enum SocketEvent {
    #[serde(rename = "events_api")]
    EventsApi {
        payload: EventCallback,
        envelope_id: String,
        accepts_response_payload: bool,
    },
    #[serde(rename = "slash_commands")]
    SlashCommands {
        payload: SlashCommand,
        envelope_id: String,
        accepts_response_payload: bool,
    },
}
#[derive(Deserialize, Debug)]
pub struct AppMention {
    #[serde(rename = "type")]
    event_type: String,
    user: String,
    text: String,
    ts: String,
    channel: String,
    event_ts: String,
}
#[derive(Deserialize, Debug)]
pub struct EventCallback {
    token: String,
    team_id: String,
    event: AppMention,
    event_id: String,
}
pub struct Client {
    //Make option and handle if no token is provided
    token: String,
    wss_url: Option<Url>,
    socket: Option<tungstenite::WebSocket<MaybeTlsStream<TcpStream>>>,
}

impl Client {
    pub fn new(token: String) -> Self {
        Client {
            token,
            wss_url: None,
            socket: None,
        }
    }
    pub async fn get_wss_url(&mut self) -> Result<(), reqwest::Error> {
        let base_url = "https://slack.com/api/".to_owned();
        let client = reqwest::Client::new();
        let connection_response = client
            .post(base_url + "apps.connections.open")
            .bearer_auth(&self.token)
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .send()
            .await?
            .json::<AppsConnectionsOpenResponse>()
            .await?;

        let wss_url = connection_response.url.unwrap();
        let url = Url::parse(&wss_url).unwrap();
        self.wss_url = Some(url);
        Ok(())
    }

    pub async fn connect(&mut self) -> Result<(), Error> {
        self.get_wss_url().await;
        if let Some(url) = &self.wss_url {
            let (mut socket, response) = connect(url).expect("Can't connect");
            let msg = socket.read_message().expect("Error reading message");
            println!("recevied hello: {:?}", msg);
            self.socket = Some(socket);
            Ok(())
        } else {
            Err(Error::new(ErrorKind::Other, "oh no!"))
        }
    }
    pub fn get_token_from_file(filename: &str) -> Result<String, std::io::Error> {
        let t = fs::read_to_string(filename)?;
        let token = t.trim().to_string();
        Ok(token)
    }
    pub async fn receive_message(&mut self) -> Result<Message, Error> {
        Ok(self
            .socket
            .as_mut()
            .unwrap()
            .read_message()
            .expect("Error reading message"))
    }
    pub fn send_message(&mut self, msg: &str) {
    println!("send message {}", msg);
    self.socket
        .as_mut()
        .unwrap()
        .write_message(Message::Text(msg.into()))
        .unwrap();
    }
}

#[derive(Deserialize, Debug)]
pub struct SlashCommand {
    token: String,
    team_id: String,
    team_domain: String,
    channel_id: String,
    channel_name: String,
    user_id: String,
    user_name: String,
    command: String,
    text: String,
    api_app_id: String,
    is_enterprise_install: String,
    response_url: String,
    trigger_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Response {
    envelope_id: String,
    payload: BlockPayload,
}

#[derive(Serialize, Deserialize, Debug)]
struct BlockPayload {
    blocks: Vec<Block>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Block {
    #[serde(rename = "type")]
    block_type: String,
    text: TextBlock,
}
impl Block {
    fn new_section(text: TextBlock) -> Self {
        Block {
            block_type: "section".to_owned(),
            text,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct TextBlock {
    #[serde(rename = "type")]
    text_type: String,
    text: String,
}
impl TextBlock {
    fn new_mrkdwn(text: String) -> TextBlock {
        TextBlock {
            text_type: "mrkdwn".to_owned(),
            text,
        }
    }
}

pub async fn handle_slash_command(
    socket: &mut tungstenite::WebSocket<MaybeTlsStream<TcpStream>>,
    payload: SlashCommand,
    envelope_id: String,
) {
    let block_type = "section";
    let text_type = "mrkdwn";
    let block1 = Block::new_section(TextBlock::new_mrkdwn("This is a test".to_owned()));
    let block2 = Block::new_section(TextBlock::new_mrkdwn("This is another test".to_owned()));
    let blocks = vec![block1, block2];
    let payload = BlockPayload { blocks };

    // send block back as resposne
    let response = Response {
        envelope_id,
        payload,
    };
    let response_json = serde_json::to_string(&response).unwrap();
    println!("send message {}", &response_json);
    socket
        .write_message(Message::Text(response_json.into()))
        .unwrap();
}

pub fn parse_message(s: &str) -> SocketEvent {
    let socket_event: SocketEvent = serde_json::from_str(s).unwrap();
    socket_event
}
