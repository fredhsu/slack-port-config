use reqwest::header::*;
use serde::{Deserialize, Serialize};
use slack::*;
use tungstenite::{connect, Message};
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


#[derive(Serialize, Deserialize, Debug)]
struct Response {
    envelope_id: String,
    payload: BlockPayload,
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
    cv.get_token_from_file("tokens/token.txt".to_string())
        .unwrap();
    // let inventory = cv.get_all_devices().await?;
    // println!("Getting Inventory");
    // println!("{}", inventory);
    // let device = cv.get_device("F799ECF9B7DA78B0BC849B972D16E373").await?;
    // println!("device: {:?}", device);
    let tags = cv.get_tags().await?;
    println!("Tags: {}", tags);

    let slack_token = slack::Client::get_token_from_file("tokens/slack.token").unwrap();
    let mut slack = slack::Client::new(slack_token);
    let wss_url = slack.get_wss_url().await.unwrap();

    slack.connect().await.unwrap();
    loop {
        let msg = slack.receive_message().await.unwrap();
        match msg {
            Message::Text(t) => handle_text(&t, &mut slack),
            Message::Binary(b) => println!("binary"),
            Message::Ping(p) => println!("{:?}", p),
            Message::Pong(p) => println!("{:?}", p),
            Message::Close(_) => println!("Close"),
        }

        // let msg = socket.read_message().expect("Error reading message");
        //let hello: Hello = serde_json::from_str(&msg).unwrap();
        // println!("recevied hello: {:?}", msg);
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
        */
    }
    Ok(())
}

fn handle_text(t: &str, slack: &mut slack::Client) {
    let socket_event = slack::parse_message(t);
    println!("{:?}", &socket_event);
    match socket_event {
        slack::SocketEvent::EventsApi {
            payload,
            envelope_id,
            accepts_response_payload,
        } => {
            println!("{:?}", payload);
        }
        slack::SocketEvent::SlashCommands {
            payload,
            envelope_id,
            accepts_response_payload,
        } => {
            println!("Received slash command: {:?}", payload);
            handle_slash_command(slack, payload, envelope_id);
        }
        _ => {}
    }
}

// TODO different commands, /portinfo walljack #
// /portassign walljack - trigger static select
// sends notification to channel with @ of admins
// /portdown walljack
// /portup walljack
fn handle_slash_command(
    slack: &mut slack::Client,
    payload: slack::SlashCommand,
    envelope_id: String,
) {
    let first = format!("Getting status for port {}", payload.text);
    let placeholder = TextBlock::new_plain("placeholder".to_string());
    let option1 = OptionObject::new(TextBlock::new_plain("this is plain".to_string()), "value-0".to_string());
    let accessory = StaticSelect::new(placeholder, "action123".to_string(), vec![option1]);
    let mut block1 = Block::new_section(TextBlock::new_mrkdwn(first));
    block1.add_accessory(accessory);
    let block2 = Block::new_section(TextBlock::new_mrkdwn("This is another test".to_owned()));
    let blocks = vec![block1, block2];
    let payload = BlockPayload::new(blocks);

    // send block back as resposne
    let response = Response {
        envelope_id,
        payload,
    };
    let response_json = serde_json::to_string(&response).unwrap();
    slack.send_message(&response_json);
    println!("slack wrote message: {}", response_json);
}
