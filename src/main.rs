use std::path::{Path, PathBuf};
use std::{collections::HashMap, fs};

use chrono::prelude::*;
use cvp::{Action, Approval, Change, ChangeConfig, CloudVisionError, RootStage, Stage, StageRow};
use serde::Deserialize;
use slack::*;
use tungstenite::Message;

use crate::cvp::StartChange;
//use serde_derive::Deserialize;

pub mod cvp;
mod slack;

use clap::Parser;

async fn get_tag_assignment(
    cv: &cvp::Host,
    label: String,
    value: String,
) -> Result<Vec<cvp::InterfaceResponse>, CloudVisionError> {
    let workspace_key = cvp::TagKey {
        workspace_id: None,
        element_type: Some("ELEMENT_TYPE_INTERFACE".to_string()),
        label: Some(label.to_string()),
        value: Some(value.to_string()),
    };
    let filter = cvp::Tag { key: workspace_key };
    let data = cvp::PartialEqFilter {
        partial_eq_filter: vec![filter],
    };
    let device_json = cv.get_tag_assignment_config(data).await?;
    println!("device: {}", &device_json);
    // TODO: use stream deserializer here for the json stream response
    // https://docs.serde.rs/serde_json/struct.StreamDeserializer.html
    // TODO: Better error handling here, should we return an error if there is no assignment?
    let result: cvp::TagAssignmentConfigResponse = serde_json::from_str(&device_json).unwrap();
    Ok(vec![result.result])
}

async fn _get_inventory(cv: &cvp::Host) -> Result<(), CloudVisionError> {
    let inventory = cv.get_all_devices().await?;
    println!("Getting Inventory");
    println!("{}", inventory);
    Ok(())
}

/// Command line arguments
#[derive(Parser, Debug, PartialEq)]
#[clap(author, version, about)]
struct Cli {
    #[clap(long)]
    cvp_host: Option<String>,
    #[clap(long)]
    cvp_port: Option<u32>,
    #[clap(long)]
    cvp_token: Option<String>,
    #[clap(long)]
    slack_token: Option<String>,
    #[clap(short, long, parse(from_os_str), value_name = "FILE")]
    config_file: Option<PathBuf>,
}

#[derive(PartialEq, Debug, Deserialize)]
struct Config {
    cloudvision: CloudVisionConfig,
    slack: SlackConfig,
}

#[derive(PartialEq, Debug, Deserialize)]
struct SlackConfig {
    token: String,
}

#[derive(PartialEq, Debug, Deserialize)]
struct CloudVisionConfig {
    hostname: String,
    port: u32,
    token: String,
}

impl Config {
    fn new_from_toml(toml_str: &str) -> Self {
        toml::from_str(toml_str).unwrap()
    }
    fn new_from_cli(cli: Cli) -> Self {
        let cloudvision = CloudVisionConfig {
            hostname: cli.cvp_host.unwrap_or_default(),
            port: cli.cvp_port.unwrap_or_default(),
            token: cli.cvp_token.unwrap_or_default(),
        };
        let slack = SlackConfig {
            token: cli.slack_token.unwrap_or_default(),
        };
        Config { cloudvision, slack }
    }
}

fn read_config_file(filename: &Path) -> Config {
    let toml_str = fs::read_to_string(filename).expect("Error reading config file");
    Config::new_from_toml(&toml_str)
}

#[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    // Options should be command line, config file, or env var
    let cli = Cli::parse();
    println!("{:?}", cli);
    let config = if let Some(config_file) = cli.config_file.as_deref() {
        read_config_file(config_file)
    } else {
        Config::new_from_cli(cli)
    };
    println!("{:?}", config);

    let mut cv = cvp::Host::new("www.cv-staging.corp.arista.io", 443);

    cv.get_token_from_file("tokens/paul-token.txt".to_string())
        .unwrap();

    let slack_token = slack::Client::get_token_from_file("tokens/slack.token").unwrap();
    let mut slack = slack::Client::new(slack_token);

    slack.connect().await.unwrap();
    loop {
        let msg = slack.receive_message().await.unwrap();
        match msg {
            Message::Text(t) => handle_text(&cv, &t, &mut slack).await,
            Message::Binary(_) => println!("binary"),
            Message::Ping(_p) => {}
            Message::Pong(_p) => {}
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
            envelope_id: _,
            accepts_response_payload: _,
        } => {
            println!("{:?}", payload);
        }
        slack::SocketEvent::SlashCommands {
            payload,
            envelope_id,
            accepts_response_payload: _,
        } => {
            handle_slash_command(cv, slack, payload, envelope_id).await;
        }
        slack::SocketEvent::Interactive {
            payload,
            envelope_id: _,
            accepts_response_payload: _,
        } => {
            println!("Received interactive: {:?}", payload);
            handle_interactive(payload).await;
            println!("response sent");
        }
    }
}

async fn handle_interactive(payload: slack::Interactive) {
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

// Matches possible slash commands
// TODO: use an enum for commands
async fn handle_slash_command(
    cv: &cvp::Host,
    slack: &mut slack::Client,
    payload: slack::SlashCommand,
    envelope_id: String,
) {
    let command = &payload.get_command();
    match command.as_str() {
        "portcheck" => portcheck(cv, &payload.text, &envelope_id, slack).await,
        "portdown" => port_shut(cv, &payload.text, &envelope_id, slack).await,
        "portup" => port_no_shut(cv, &payload.text, &envelope_id, slack).await,
        "portassign" => println!("Assign port {} ", &payload.text),
        _ => println!("Unknown command {}", command),
    }
}

async fn portcheck(cv: &cvp::Host, walljack: &str, envelope_id: &str, slack: &mut slack::Client) {
    let device = get_tag_assignment(cv, "wall_jack".to_string(), walljack.to_string())
        .await
        .unwrap();
    let resp_text = if let Some(first_device) = device.first() {
        format!(
            "Wall jack: {} is connected to port {} on switch {}",
            walljack, &first_device.value.key.interface_id, &first_device.value.key.device_id
        )
    } else {
        "Wall jack number was not found".to_string()
    };
    let block2 = Block::new_section(TextBlock::new_mrkdwn(resp_text));
    let blocks = vec![block2];
    let payload = BlockPayload::new(blocks);
    slack.send_response(envelope_id, payload);
}

async fn port_shut(cv: &cvp::Host, walljack: &str, envelope_id: &str, slack: &mut slack::Client) {
    let resp_text;
    let device = get_tag_assignment(cv, "wall_jack".to_string(), walljack.to_string())
        .await
        .unwrap();
    if let Some(first_device) = device.first() {
        resp_text = format!("Wall jack: {} has been shut down", walljack);
        execute_shut_action(
            cv,
            &first_device.value.key.device_id,
            &first_device.value.key.interface_id,
        )
        .await;
    } else {
        resp_text = "Wall jack number was not found".to_string();
    }
    let block2 = Block::new_section(TextBlock::new_mrkdwn(resp_text));
    let blocks = vec![block2];
    let payload = BlockPayload::new(blocks);
    slack.send_response(envelope_id, payload);
}
async fn port_no_shut(
    cv: &cvp::Host,
    walljack: &str,
    envelope_id: &str,
    slack: &mut slack::Client,
) {
    // TODO: pass function such as execute_no_shut_action as a functino parameter to a
    // function that will generate response and execute action
    let resp_text;
    let device = get_tag_assignment(cv, "wall_jack".to_string(), walljack.to_string())
        .await
        .unwrap();
    if let Some(first_device) = device.first() {
        resp_text = format!("Wall jack: {} has been enabled", walljack);
        execute_no_shut_action(
            cv,
            &first_device.value.key.device_id,
            &first_device.value.key.interface_id,
        )
        .await;
    } else {
        resp_text = "Wall jack number was not found".to_string();
    }
    let block2 = Block::new_section(TextBlock::new_mrkdwn(resp_text));
    let blocks = vec![block2];
    let payload = BlockPayload::new(blocks);
    slack.send_response(envelope_id, payload);
}

// TODO: Not yet implemented
fn _port_assign(text: &str, envelope_id: &str, slack: &mut slack::Client) {
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

async fn execute_shut_action(cv: &cvp::Host, device: &str, interface: &str) {
    // Build the action
    let change = build_shut_action(device.to_string(), interface.to_string());
    let change_json = serde_json::to_string(&change).unwrap();
    let cc_res = cv.post_change_control(change_json).await.unwrap();
    println!("post_change_control result: {}", cc_res);

    // Approve the change
    let cc_timestamp = format!("{:?}", Utc::now());
    let cc_id = change.config.id;
    let approval = Approval {
        cc_id: cc_id.clone(),
        cc_timestamp,
    };
    let response = cv.approve_change_control(approval).await.unwrap();
    println!("approval response: {}", response);
    let start = StartChange {
        cc_id: cc_id.clone(),
    };
    // Execute the change
    cv.execute_change_control(start).await.unwrap();
}
async fn execute_no_shut_action(cv: &cvp::Host, device: &str, interface: &str) {
    // Build the action
    let change = build_no_shut_action(device.to_string(), interface.to_string());
    let change_json = serde_json::to_string(&change).unwrap();
    let cc_res = cv.post_change_control(change_json).await.unwrap();
    println!("post_change_control result: {}", cc_res);

    // Approve the change
    let cc_timestamp = format!("{:?}", Utc::now());
    let cc_id = change.config.id;
    let approval = Approval {
        cc_id: cc_id.clone(),
        cc_timestamp,
    };
    let response = cv.approve_change_control(approval).await.unwrap();
    println!("approval response: {}", response);
    let start = StartChange {
        cc_id: cc_id.clone(),
    };
    // Execute the change
    cv.execute_change_control(start).await.unwrap();
}

fn build_no_shut_action(device: String, interface: String) -> Change {
    let mut args = HashMap::new();
    args.insert("DeviceID".to_string(), device);
    args.insert("interface".to_string(), interface);
    let action_name = "rfzsJdsdQEU9EOlPeNeAL".to_string();
    let stage_name = "no_shut_interface".to_string();
    build_action_change(action_name, stage_name, args)
}

fn build_shut_action(device: String, interface: String) -> Change {
    let utc = Utc::now().format("%y-%m-%d-%H-%M-%S").to_string();
    let mut args = HashMap::new();
    args.insert("DeviceID".to_string(), device);
    args.insert("interface".to_string(), interface);
    // TODO: put action name and id into config struct
    let action = Action {
        name: "ps5pMVndlXpK6IsQJGr7U".to_string(),
        args,
    };
    let stage = Stage::new("shut_interface".to_string(), action);
    let stages = vec![stage];
    let stage_row = StageRow { stage: stages };
    let stage_rows = vec![stage_row];
    let root_stage = RootStage::new(format!("Change {} root", utc), stage_rows);
    let config = ChangeConfig::new(format!("Change {}", utc), root_stage);
    Change { config }
}

fn build_action_change(
    action_name: String,
    stage_name: String,
    args: HashMap<String, String>,
) -> Change {
    let utc = Utc::now().format("%y-%m-%d-%H-%M-%S").to_string();
    let action = Action {
        name: action_name,
        args,
    };
    let stage = Stage::new(stage_name, action);
    let stages = vec![stage];
    let stage_row = StageRow { stage: stages };
    let stage_rows = vec![stage_row];
    let root_stage = RootStage::new(format!("Change {} root", utc), stage_rows);
    let config = ChangeConfig::new(format!("Change {}", utc), root_stage);
    Change { config }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_new_from_toml() {
        let toml_str = r#"
        [cloudvision]
        hostname = "www.cv-staging.arista.io"
        port = 443
        token = "cvptoken"
        [slack]
        token = "slacktoken"
        "#;
        let config = Config::new_from_toml(toml_str);
        let cloudvision = CloudVisionConfig {
            hostname: "www.cv-staging.arista.io".to_string(),
            port: 443,
            token: "cvptoken".to_string(),
        };
        let slack = SlackConfig {
            token: "slacktoken".to_string(),
        };
        let base_config = Config { cloudvision, slack };
        assert_eq!(config, base_config);
    }
    #[test]
    fn test_new_from_cli() {
        let config_file = Some(PathBuf::from("config.toml"));

        let cli = Cli {
            cvp_host: Some("www.cv-staging.arista.io".to_string()),
            cvp_port: Some(443),
            cvp_token: Some("cvptoken".to_string()),
            slack_token: Some("slacktoken".to_string()),
            config_file,
        };
        let config = Config::new_from_cli(cli);
        let cloudvision = CloudVisionConfig {
            hostname: "www.cv-staging.arista.io".to_string(),
            port: 443,
            token: "cvptoken".to_string(),
        };
        let slack = SlackConfig {
            token: "slacktoken".to_string(),
        };
        let base_config = Config { cloudvision, slack };
        assert_eq!(config, base_config);
    }
    #[test]
    fn test_action_change() {
        let device = "JPE1999";
        let interface = "Ethernet1";
        let action_name = "ps5pMVndlXpK6IsQJGr7U".to_string();
        let stage_name = "shut_interface".to_string();
        let mut args = HashMap::new();
        args.insert("DeviceID".to_string(), device.to_string());
        args.insert("interface".to_string(), interface.to_string());

        let build_action = build_action_change(action_name, stage_name.clone(), args.clone());
        let stage = build_action
            .config
            .root_stage
            .stage_row
            .first()
            .unwrap()
            .stage
            .first()
            .unwrap();
        assert_eq!(&stage_name, &stage.name);
        assert_eq!(&args, &stage.action.args);
    }
}
