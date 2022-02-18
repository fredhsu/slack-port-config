use std::{ops::Add, collections::HashMap};

use cvp::{RootStage, StageRow, Stage, Action, ChangeConfig, Change, Approval};
use reqwest::header::*;
use serde::{Deserialize, Serialize};
use slack::*;
use tungstenite::Message;
use chrono::prelude::*;

use crate::cvp::StartChange;
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

async fn get_tags(cv: &cvp::Host) -> Result<(), reqwest::Error>{
    let tags = cv.get_tags().await?;
    println!("Tags: {}", tags);
    Ok(())
}
async fn get_tag_assignment(cv: &cvp::Host, label: String, value: String) -> Result<Vec<cvp::InterfaceResponse>, reqwest::Error>{
    /*
    workspace_id: "",
    element_type: ELEMENT_TYPE_INTERFACE,
    label: "wall_jack",
    value: "sjc1-401-019-1",
    */
    /* Jack numbering
    Jack number format:  BBf-rrr-ppp-j
where   BB        = two letter building code
            f           = floor number
            rr        = room number
            pp      = plate number
            j           = number of jack on that plate
            */
        let workspace_key = cvp::TagKey { workspace_id: "".to_string(), element_type: Some("ELEMENT_TYPE_INTERFACE".to_string()), label: Some(label.to_string()), value: Some(value.to_string()) };
        let filter = cvp::Tag {
            key: workspace_key,
        };
        let data = cvp::PartialEqFilter {
            partial_eq_filter: vec![filter],
        };
    let device_json = cv.get_tag_assignment(data).await.unwrap();
    // let device : Vec<cvp::InterfaceResponse> = serde_json::from_str(&device_json).unwrap();
    Ok(serde_json::from_str(&device_json).unwrap())
}

async fn get_inventory(cv: &cvp::Host)-> Result<(), reqwest::Error> {
    let inventory = cv.get_all_devices().await?;
    println!("Getting Inventory");
    println!("{}", inventory);
    Ok(())
}

async fn get_device(cv: &cvp::Host)-> Result<(), reqwest::Error> {
    let device = cv.get_device("JPE12233288").await?;
    println!("device: {:?}", device);
    Ok(())
}

async fn shut_interface(cv: &cvp::Host) -> Result<(), reqwest::Error> {

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
    // let mut cv = cvp::Host::new(
    //     "10.90.226.175",
    //     443,
    //     "cvpadmin",
    //     "arista123!",
    // );
    cv.get_token_from_file("tokens/token.txt".to_string())
        .unwrap();

        let device = "SSJ17200818".to_string();
        let interface = "Ethernet1".to_string();
    execute_change_action(&cv, device, interface).await;
    let slack_token = slack::Client::get_token_from_file("tokens/slack.token").unwrap();
    let mut slack = slack::Client::new(slack_token);

    slack.connect().await.unwrap();
    loop {
        let msg = slack.receive_message().await.unwrap();
        match msg {
            Message::Text(t) => handle_text(&cv, &t, &mut slack).await,
            Message::Binary(_) => println!("binary"),
            Message::Ping(p) => {},
            Message::Pong(p) => {},
            Message::Close(_) => break,
        }
    }
    Ok(())
}

async fn handle_text(cv: &cvp::Host, t: &str, slack: &mut slack::Client) {
    let socket_event = slack::parse_message(t);
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
            handle_slash_command(cv, slack, payload, envelope_id).await;
        }
        slack::SocketEvent::Interactive {
            payload,
            envelope_id,
            accepts_response_payload,
        } => {
            println!("Received interactive: {:?}", payload);
            handle_interactive(payload).await;
            println!("response sent");
        }
    }
}

async fn handle_interactive(
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
async fn handle_slash_command(
    cv: &cvp::Host,
    slack: &mut slack::Client,
    payload: slack::SlashCommand,
    envelope_id: String,
) {
    let command = &payload.get_command();
    println!("Command is: {}", &command);
    match command.as_str() {
        "portcheck" => portcheck(cv, &payload.text, &envelope_id, slack).await,
        "portassign" => port_assign(&payload.text, &envelope_id, slack),
        "portdown" => println!("Shutting down port {}", &payload.text),
        "portup" => println!("Bringing port {} up", &payload.text),
        _ => println!("Unknown command"),
    }
}

async fn portcheck(cv: &cvp::Host, walljack: &str, envelope_id: &str, slack: &mut slack::Client) {
    let device = get_tag_assignment(&cv, "wall_jack".to_string(), walljack.to_string()).await.unwrap();
    // println!("devicejson: {}", device_json);
    let first_device = &device.first().unwrap().value.key;
    println!("portcheck device: {}", &first_device.device_id);
    // let resp_text = format!(
    //     "Wall jack: {} is connected to port {} on switch {} and is in segement ID {}",
    //     walljack, &first_device.interface_id, &first_device.device_id, segment_id
    // );
    let resp_text = format!(
        "Wall jack: {} is connected to port {} on switch {}",
        walljack, &first_device.interface_id, &first_device.device_id);
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

async fn execute_change_action(cv: &cvp::Host, device:String, interface: String) {
    // Build the action
    let change = build_change_action(device, interface);
    let change_json = serde_json::to_string(&change).unwrap();
    cv.post_change_control(change_json).await.unwrap();
    // Approve the change
    let cc_timestamp= format!("{:?}", Utc::now());
    let cc_id = change.config.id;
    let approval = Approval {
        cc_id: cc_id.clone(),
        cc_timestamp,
    };
    let response = cv.approve_change_control(approval).await.unwrap();
    println!("approval response: {}", response);
    let start= StartChange {
        cc_id: cc_id.clone(),
    };
    // Execute the change
    // cv.execute_change_control(start).await.unwrap();

}

fn build_change_action(device: String, interface: String) -> Change {
    let utc= Utc::now().format("%y-%m-%d-%H-%M-%S").to_string();
    let mut args = HashMap::new();
    args.insert("DeviceID".to_string(), device);
    args.insert("interface".to_string(), interface);
    let action = Action{
        name: "ps5pMVndlXpK6IsQJGr7U".to_string(),
        args,
    };
    let stage = Stage::new("shut_interface".to_string(), action);
    let stages = vec![stage];
    let stage_row = StageRow{
        stage: stages};
    let stage_rows = vec![stage_row];
    let root_stage = RootStage::new(format!("Change {} root", utc), stage_rows);
    let config = ChangeConfig::new(format!("Change {}", utc), root_stage);
    Change {config}
}