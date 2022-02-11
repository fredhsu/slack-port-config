use reqwest::header::*;
use serde::{Deserialize, Serialize};
//use serde_json::Result;
use std::net::TcpStream;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{connect, Message};
use url::Url;
mod cvp;
mod slack;

#[derive(Deserialize, Debug)]
struct AppsConnectionsOpenResponse {
    ok: bool,
    url: Option<String>,
    error: Option<String>,
}

#[derive(Deserialize, Debug)]
struct ChannelsResponse {
    ok: bool,
    channels: Vec<Channel>,
}

#[derive(Deserialize, Debug)]
struct Channel {
    id: String,
    name: String,
    is_channel: bool,
    created: u32,
    creator: String,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
enum SocketEvent {
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

#[derive(Serialize, Deserialize, Debug)]
struct TextBlock {
    #[serde(rename = "type")]
    text_type: String,
    text: String,
}

// TODO: create enum for event payloads and event_types
#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
enum EventPayload {
    #[serde(rename = "event_callback")]
    EventCallback(EventCallback),
}

#[derive(Deserialize, Debug)]
struct EventCallback {
    token: String,
    team_id: String,
    event: AppMention,
    event_id: String,
}

#[derive(Deserialize, Debug)]
struct Hello {
    num_connections: u32,
    debug_info: DebugInfo,
    connection_info: ConnectionInfo,
}
#[derive(Deserialize, Debug)]
struct DebugInfo {
    host: String,
    build_number: u32,
    approximate_connection_time: u32,
}
#[derive(Deserialize, Debug)]
struct ConnectionInfo {
    app_id: String,
}

#[derive(Deserialize, Debug)]
struct AppMention {
    #[serde(rename = "type")]
    event_type: String,
    user: String,
    text: String,
    ts: String,
    channel: String,
    event_ts: String,
}

#[derive(Deserialize, Debug)]
enum EventType {
    #[serde(rename = "slash_command")]
    SlashCommand,
    #[serde(rename = "events_api")]
    EventsAPI,
    #[serde(rename = "hello")]
    Hello,
    #[serde(rename = "event_callback")]
    EventCallback,
}

async fn get_channels(base_url: String, oauth_token: &str) -> Result<(), reqwest::Error> {
    let client = reqwest::Client::new();
    let conversations_url = format!("{}{}", base_url, "conversations.list");

    let response = client
        .get(conversations_url)
        .header(AUTHORIZATION, "Bearer ".to_owned() + oauth_token)
        .send()
        .await?
        .json::<ChannelsResponse>()
        .await?;
    println!("body = {:?}", response);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    let mut cv = cvp::Host::new(
        "www.cv-staging.corp.arista.io",
        443,
        "fredlhsu@arista.com",
        "arista",
    );
    cv.get_token_from_file("token.txt".to_string());
    let inventory = cv.get_all_devices().await?;
    println!("{}", inventory);
    let device = cv.get_device("F799ECF9B7DA78B0BC849B972D16E373").await?;
    println!("device: {:?}", device);

    let mut slack = slack::Client::new();
    let wss_url = slack.get_wss_url().await.unwrap();

    let mut socket = slack.connect().await;
    if let Some(socket) = socket {
    let msg = socket.read_message().expect("Error reading message");
    //let hello: Hello = serde_json::from_str(&msg).unwrap();
    println!("recevied hello: {:?}", msg);
    }
    /*
    loop {
        let msg = socket.read_message().expect("Error reading message");
        if let tungstenite::Message::Text(msg) = msg {
            println!("Received message: {}", msg);
            let socket_event: SocketEvent = serde_json::from_str(&msg).unwrap();
            println!("Received: {:?}", socket_event);
            match socket_event {
                SocketEvent::EventsApi {
                    payload,
                    envelope_id,
                    accepts_response_payload,
                } => {
                    println!("{:?}", payload);
                    if payload.event.text.ends_with("quit") {
                        break;
                    }
                }
                SocketEvent::SlashCommands {
                    payload,
                    envelope_id,
                    accepts_response_payload,
                } => {
                    println!("{} is : {:?}", payload.command, payload.text);
                    handle_slash_command(&mut socket, payload, envelope_id);
                }
                _ => {}
            }
        }
        // send ack back to slack with envelope_id
    }
    */
    Ok(())
}

fn handle_slash_command(
    socket: &mut tungstenite::WebSocket<MaybeTlsStream<TcpStream>>,
    payload: SlashCommand,
    envelope_id: String,
) {
    let block_type = "section";
    let text_type = "mrkdwn";
    let block1 = Block {
        block_type: block_type.to_string(),
        text: TextBlock {
            text_type: text_type.to_string(),
            text: "#switch 1".to_owned(),
        },
    };
    let block2 = Block {
        block_type: block_type.to_string(),
        text: TextBlock {
            text_type: text_type.to_string(),
            text: "port 1".to_owned(),
        },
    };
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
