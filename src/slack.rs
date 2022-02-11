use reqwest::header::*;
use serde::{Deserialize, Serialize};
use std::fs;

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

struct Client {
    //Make option and handle if no token is provided
    token: String,
}

impl Client {
    pub async fn get_wss_url(&self) -> Result<reqwest::Url, reqwest::Error> {
        // TODO: add check for token is available
        let base_url = "https://slack.com/api/".to_owned();
        let client = reqwest::Client::new();
        let connection_response = client
            .post(base_url + "apps.connections.open")
            .bearer_auth(self.token.to_string())
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .send()
            .await?
            .json::<AppsConnectionsOpenResponse>()
            .await?;

        let wss_url = connection_response.url.unwrap();
        let url = Url::parse(&wss_url).unwrap();
        Ok(url)
    }
    pub async fn connect(
        &self,
    ) -> Result<(tungstenite::WebSocket<MaybeTlsStream<TcpStream>>, Response), reqwest::Error> {
        let url = &self.get_wss_url().await?;
        let (mut socket, response) = connect(url).expect("Can't connect");
        let msg = socket.read_message().expect("Error reading message");
        //let hello: Hello = serde_json::from_str(&msg).unwrap();
        println!("recevied hello: {:?}", msg);
        Ok((socket, response))
    }
    pub fn get_token_from_file(&mut self, filename: String) -> Result<(), std::io::Error> {
        let t = fs::read_to_string(filename)?;
        let token = t.trim().to_string();
        self.token = Some(token);
        Ok(())
    }
}

#[derive(Deserialize, Debug)]
struct SlashCommand {
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

pub fn handle_slash_command(
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
