use reqwest::header::*;
use serde::{Deserialize, Serialize};
use slack::*;
use tungstenite::Message;
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
    // let mut cv = cvp::Host::new(
    //     "www.cv-staging.corp.arista.io",
    //     443,
    //     "fredlhsu@arista.com",
    //     "arista",
    // );
    let mut cv = cvp::Host::new(
        "10.90.226.175",
        443,
        "cvpadmin",
        "arista123!",
    );
    // cv.get_token_from_file("tokens/token.txt".to_string())
    //     .unwrap();
    cv.get_token_from_auth().await.unwrap();
    let inventory = cv.get_all_devices().await?;
    println!("Getting Inventory");
    println!("{}", inventory);
    // let device = cv.get_device("F799ECF9B7DA78B0BC849B972D16E373").await?;
    // println!("device: {:?}", device);
    let tags = cv.get_tags().await?;
    println!("Tags: {}", tags);

    let slack_token = slack::Client::get_token_from_file("tokens/slack.token").unwrap();
    let mut slack = slack::Client::new(slack_token);
    // let wss_url = slack.get_wss_url().await.unwrap();

    slack.connect().await.unwrap();
    loop {
        let msg = slack.receive_message().await.unwrap();
        match msg {
            Message::Text(t) => handle_text(&t, &mut slack).await,
            Message::Binary(b) => println!("binary"),
            Message::Ping(p) => println!("{:?}", p),
            Message::Pong(p) => println!("{:?}", p),
            Message::Close(_) => break,
        }
    }
    Ok(())
}

async fn handle_text(t: &str, slack: &mut slack::Client) {
    println!("*** Incoming text *** \n {:?}", t);
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
        slack::SocketEvent::Interactive {
            payload,
            envelope_id,
            accepts_response_payload,
        } => {
            println!("Received interactive: {:?}", payload);
            handle_interactive(slack, payload).await;
            println!("response sent");
        }
        _ => {}
    }
}

async fn handle_interactive(
    slack: &mut slack::Client,
    payload: slack::Interactive,
) {
    println!("Received interactive with actions {:?}", payload.actions);
    let text = format!(
        "Updated with segement ID {}",
        payload.actions.first().unwrap().selected_option.text.text
    );
    let message = slack::MessagePayload {
        text,
        blocks: None,
        thread_ts: None,
        mrkdwn: false,
    };
    // Resposne to an interactive action is via response_url which is specific to the action and will tie into the block that sent the action
    let response_json = serde_json::to_string(&message).unwrap();
    println!("responding to : {}", &payload.response_url);
    let client = reqwest::Client::new();
    client
        .post(&payload.response_url)
        .body(response_json)
        .send()
        .await
        .unwrap();
        //TODO remove semicolon and make this return value
}

// logic for different slash commands
// /portinfo walljack #
// /portassign walljack - trigger static select
// sends notification to channel with @ of admins
// /portdown walljack
// /portup walljack
fn handle_slash_command(
    slack: &mut slack::Client,
    payload: slack::SlashCommand,
    envelope_id: String,
) {
    let command = &payload.get_command();
    match command.as_str() {
        "portcheck" => portcheck(&payload.text, &envelope_id, slack),
        "portassign" => port_assign(&payload.text, &envelope_id, slack),
        "portdown" => println!("Shutting down port {}", &payload.text),
        "portup" => println!("Bringing port {} up", &payload.text),
        _ => println!("Unknown command"),
    }
    // let first = format!("Getting status for port {}", payload.text);
    // let placeholder = TextBlock::new_plain("placeholder".to_string());
    // let option1 = OptionObject::new(
    //     TextBlock::new_plain("this is plain".to_string()),
    //     "value-0".to_string(),
    // );
    // let accessory = StaticSelect::new(placeholder, "action123".to_string(), vec![option1]);
    // let mut block1 = Block::new_section(TextBlock::new_mrkdwn(first));
    // block1.add_accessory(accessory);
    // let block2 = Block::new_section(TextBlock::new_mrkdwn("This is another test".to_owned()));
    // let blocks = vec![block1, block2];
    // let payload = BlockPayload::new(blocks);

    // send block back as resposne
    // let response = Response {
    //     envelope_id,
    //     payload,
    // };
    // let response_json = serde_json::to_string(&response).unwrap();
    // slack.send_message(&response_json);
    // println!("slack wrote message: {}", response_json);
}

fn portcheck(text: &str, envelope_id: &str, slack: &mut slack::Client) {
    let walljack = text;
    let switchport = "Eth 1/1";
    let switch_id = "JPE123";
    let segment_id = "USER:VLAN100";
    let resp_text = format!(
        "Wall jack: {} is connected to port {} on switch {} and is in segement ID {}",
        walljack, switchport, switch_id, segment_id
    );
    let block2 = Block::new_section(TextBlock::new_mrkdwn(resp_text));
    let blocks = vec![block2];
    let payload = BlockPayload::new(blocks);
    slack.send_response(envelope_id, payload);
}

fn port_assign(text: &str, envelope_id: &str, slack: &mut slack::Client) {
    let placeholder = TextBlock::new_plain("segment".to_string());
    let option1 = OptionObject::new(
        TextBlock::new_plain("USERS:VLAN 100".to_string()),
        "vlan100".to_string(),
    );
    let accessory = StaticSelect::new(placeholder, "action123".to_string(), vec![option1]);
    let first = format!("Choose a segment for walljack: {}", text);
    let mut block1 = Block::new_section(TextBlock::new_mrkdwn(first));
    block1.add_accessory(accessory);
    let blocks = vec![block1];
    let payload = BlockPayload::new(blocks);
    slack.send_response(envelope_id, payload);
}
