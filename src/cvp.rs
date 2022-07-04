use reqwest::header::*;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs};
use url::Url;
use uuid::Uuid;

/// Wraps error types when working with CloudVision APIs or parsing
#[derive(Debug)]
pub enum CloudVisionError {
    NoToken,
    Request(reqwest::Error),
    JsonParse(serde_json::Error),
}

impl From<reqwest::Error> for CloudVisionError {
    fn from(err: reqwest::Error) -> Self {
        CloudVisionError::Request(err)
    }
}
impl From<serde_json::Error> for CloudVisionError {
    fn from(err: serde_json::Error) -> Self {
        CloudVisionError::JsonParse(err)
    }
}

pub struct Config {
    pub hostname: String,
    pub port: u16,
    pub token: String,
}

impl Config {
    pub fn new(hostname: String, port: u16, token: String) -> Self {
        Self {
            hostname,
            port,
            token,
        }
    }
    pub fn from_file(filename: String) -> Self {
        // readfile
        unimplemented!();
    }
    pub fn from_env() -> Self {
        // read env
        unimplemented!();
    }
}

// A CloudVision host
pub struct Host {
    hostname: String,
    port: u16,
    pub token: Option<String>,
    pub base_url: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TokenResponse {
    cookie: CookieResponse,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct CookieResponse {
    #[serde(rename = "Value")]
    value: String,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct PartialEqFilter {
    pub partial_eq_filter: Vec<Tag>,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct Tag {
    pub key: TagKey,
}

// TODO: reduce pub fields
#[derive(Serialize, Deserialize, Debug)]
pub struct TagKey {
    pub workspace_id: Option<String>,
    pub element_type: Option<String>,
    //TODO make elementtype enum
    pub label: Option<String>,
    pub value: Option<String>,
}
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ElementType {
    Unspecified,
    Device,
    Interface,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InterfaceQueryResponse {
    pub value: Vec<InterfaceResponse>,
}

// TODO: Generalize to handle different response types
#[derive(Serialize, Deserialize, Debug)]
pub struct TagAssignmentConfigResponse {
    pub result: InterfaceResponse,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct InterfaceResponse {
    pub value: Interface,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Interface {
    pub key: InterfaceKey,
}
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InterfaceKey {
    pub workspace_id: String,
    pub element_type: String,
    pub label: String,
    pub value: String,
    pub device_id: String,
    pub interface_id: String,
}
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub key: DeviceKey,
    pub software_version: String,
    pub model_name: String,
    pub hardware_revision: String,
    pub fqdn: String,
    pub hostname: String,
    pub domain_name: String,
    pub system_mac_address: String,
    pub boot_time: String,
    pub streaming_status: String,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct DeviceKey {
    #[serde(rename = "deviceId")]
    pub device_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ChangeAction {
    change: Change,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Change {
    pub config: ChangeConfig,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct ChangeConfig {
    pub id: String,
    pub name: String,
    pub root_stage: RootStage,
}
impl ChangeConfig {
    pub fn new(name: String, root_stage: RootStage) -> Self {
        let id = Uuid::new_v4().to_string();
        ChangeConfig {
            id,
            name,
            root_stage,
        }
    }
}
#[derive(Serialize, Deserialize, Debug)]
pub struct RootStage {
    pub id: String,
    pub name: String,
    pub stage_row: Vec<StageRow>,
}
impl RootStage {
    pub fn new(name: String, stage_row: Vec<StageRow>) -> Self {
        let id = Uuid::new_v4().to_string();
        RootStage {
            id,
            name,
            stage_row,
        }
    }
}
#[derive(Serialize, Deserialize, Debug)]
pub struct StageRow {
    pub stage: Vec<Stage>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Stage {
    pub id: String,
    pub name: String,
    pub action: Action,
}
impl Stage {
    pub fn new(name: String, action: Action) -> Self {
        let id = Uuid::new_v4().to_string();
        Stage { id, name, action }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Action {
    pub name: String,
    pub args: HashMap<String, String>,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct Approval {
    pub cc_id: String,
    pub cc_timestamp: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StartChange {
    pub cc_id: String,
}

impl Host {
    pub fn new(hostname: &str, port: u16) -> Self {
        Host {
            hostname: hostname.to_string(),
            port,
            token: None,
            //base_url: format!("https://{}:{}", hostname, port),
            base_url: format!("https://{}", hostname),
        }
    }
    pub fn build_url(&self, path: &str) -> String {
        let mut url = Url::parse(&self.base_url).unwrap();
        url.set_path(path);
        url.set_port(Some(self.port));
        url.as_str().to_string()
        //format!("{}{}", self.base_url, path)
    }

    pub fn get_token_from_file(&mut self, filename: String) -> Result<(), std::io::Error> {
        let t = fs::read_to_string(filename)?;
        let token = t.trim().to_string();
        self.token = Some(token);
        Ok(())
    }

    pub async fn get(&self, path: &str) -> Result<String, CloudVisionError> {
        if let Some(token) = &self.token {
            let url = self.build_url(path);
            let client = reqwest::Client::builder()
                .danger_accept_invalid_certs(true)
                .build()?;
            let response = client
                .get(url)
                .header(ACCEPT, "application/json")
                .bearer_auth(token.to_string())
                .send()
                .await?
                .text()
                .await?;
            Ok(response)
        } else {
            Err(CloudVisionError::NoToken)
        }
    }
    async fn post(&self, path: &str, body: String) -> Result<String, CloudVisionError> {
        if let Some(token) = &self.token {
            let url = self.build_url(path);
            let client = reqwest::Client::builder()
                .danger_accept_invalid_certs(true)
                .build()?;
            let response = client
                .post(url)
                .header(ACCEPT, "application/json")
                .bearer_auth(token.to_string())
                .body(body)
                .send()
                .await?
                .text()
                .await?;
            println!("POST response: {}", &response);
            Ok(response)
        } else {
            Err(CloudVisionError::NoToken)
        }
    }
    pub async fn get_tags(&self) -> Result<String, CloudVisionError> {
        let path = "/api/resources/tag/v2/Tag/all";
        // TODO: replace this with the url above when cvaas is fixed
        // let path = "/api/v3/services/arista.tag.v2.Tag/GetAll";
        let workspace_key = TagKey {
            workspace_id: None,
            element_type: None,
            label: None,
            value: None,
        };
        let filter = Tag { key: workspace_key };
        let data = PartialEqFilter {
            partial_eq_filter: vec![filter],
        };
        let json_data = serde_json::to_string(&data)?;
        self.post(path, json_data).await
    }

    // TODO rework these to return proper values, will need introspection on json deserialization
    pub async fn get_tag_assignment_config(
        &self,
        partial_eq_filter: PartialEqFilter,
    ) -> Result<String, CloudVisionError> {
        let path = "/api/resources/tag/v2/TagAssignmentConfig/all";
        let json_data = serde_json::to_string(&partial_eq_filter)?;
        self.post(path, json_data).await
    }

    pub async fn get_all_devices(&self) -> Result<String, CloudVisionError> {
        let path = "/api/resources/inventory/v1/Device/all";
        self.get(path).await
    }
    pub async fn get_device(&self, device_id: &str) -> Result<String, CloudVisionError> {
        let path = format!(
            "/api/resources/inventory/v1/Device?key.deviceId={}",
            device_id
        );
        self.get(&path).await
    }
    pub async fn post_change_control(&self, change: String) -> Result<String, CloudVisionError> {
        let path = "/api/v3/services/ccapi.ChangeControl/Update".to_string();
        self.post(&path, change).await
    }

    pub async fn approve_change_control(
        &self,
        approval: Approval,
    ) -> Result<String, CloudVisionError> {
        let approval_json = serde_json::to_string(&approval)?;
        let path = "/api/v3/services/ccapi.ChangeControl/AddApproval".to_string();
        println!("Approving: {}", &approval_json);
        self.post(&path, approval_json).await
    }
    pub async fn execute_change_control(
        &self,
        start: StartChange,
    ) -> Result<String, CloudVisionError> {
        let start_json = serde_json::to_string(&start)?;
        let path = "/api/v3/services/ccapi.ChangeControl/Start".to_string();
        self.post(&path, start_json).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_get_token_from_file() {
        let mut cv = Host::new("foo", 443);
        cv.get_token_from_file("tokens/token.txt".to_string())
            .unwrap();
        if let Some(token) = cv.token {
            assert!(token.starts_with("ey"));
        } else {
            panic!("did not read file");
        }
    }
    #[test]
    fn test_build_url() {
        let cv = Host::new("foo", 8000);
        let url = cv.build_url("/bar");
        assert_eq!(url, "https://foo:8000/bar");
    }
}
