use chrono::prelude::*;
use reqwest::header::*;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs};
use uuid::Uuid;

pub struct Host {
    hostname: String,
    port: u32,
    username: String,
    password: String,
    token: Option<String>,
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
#[derive(Serialize, Deserialize, Debug)]
pub struct TagKey {
    pub workspace_id: String,
    pub element_type: Option<String>,
    //TODO make elementtype enum
    pub label: Option<String>,
    pub value: Option<String>,
}
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ElementType {
    ElementTypeUnspecified,
    ElementTypeDevice,
    ElementTypeInterface,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InterfaceQueryResponse {
    pub value: Vec<InterfaceResponse>,
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
    pub fn new(hostname: &str, port: u32, username: &str, password: &str) -> Self {
        Host {
            hostname: hostname.to_string(),
            port,
            username: username.to_string(),
            password: password.to_string(),
            token: None,
        }
    }

    pub fn get_token_from_file(&mut self, filename: String) -> Result<(), std::io::Error> {
        let t = fs::read_to_string(filename)?;
        let token = t.trim().to_string();
        self.token = Some(token);
        Ok(())
    }

    // get_token is only useful for on prem CVP
    pub async fn get_token_from_auth(&mut self) -> Result<(), reqwest::Error> {
        let path = "/cvpservice/login/authenticate.do";
        let url = format!("https://{}:{}{}", self.hostname, self.port, path);
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()?;
        let response = client
            .post(url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await?
            .json::<TokenResponse>()
            .await?;
        self.token = Some(response.cookie.value);
        println!("token is {:?}", &self.token);

        Ok(())
    }

    async fn get(&self, path: &str) -> Result<String, reqwest::Error> {
        if let Some(token) = &self.token {
            let url = format!("https://{}{}", self.hostname, path);
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
            // TODO: use an error to indicate no token
            Ok("".to_string())
        }
    }
    async fn post(&self, path: &str, body: String) -> Result<String, reqwest::Error> {
        if let Some(token) = &self.token {
            let url = format!("https://{}{}", self.hostname, path);
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
            Ok(response)
        } else {
            // TODO: use an error to indicate no token
            Ok("".to_string())
        }
    }
    pub async fn get_tags(&self) -> Result<String, reqwest::Error> {
        let path = "/api/resources/tag/v2/Tag/all";
        // TODO: replace this with the url above when cvaas is fixed
        // let path = "/api/v3/services/arista.tag.v2.Tag/GetAll";
        let workspace_key = TagKey {
            workspace_id: "".to_string(),
            element_type: None,
            label: None,
            value: None,
        };
        let filter = Tag { key: workspace_key };
        let data = PartialEqFilter {
            partial_eq_filter: vec![filter],
        };
        let json_data = serde_json::to_string(&data).unwrap();
        self.post(path, json_data).await
    }
    pub async fn get_tag_assignment(
        &self,
        partial_eq_filter: PartialEqFilter,
    ) -> Result<String, reqwest::Error> {
        // let path = "/api/resources/tag/v2/TagAssignment/all";
        // TODO: replace this with the url above when cvaas is fixed
        let path = "/api/v3/services/arista.tag.v2.TagAssignmentService/GetAll";
        let json_data = serde_json::to_string(&partial_eq_filter).unwrap();
        println!("Filter is : {}", json_data);
        self.post(path, json_data).await
    }

    pub async fn get_all_devices(&self) -> Result<String, reqwest::Error> {
        let path = "/api/resources/inventory/v1/Device/all";
        self.get(path).await
    }
    pub async fn get_device(&self, device_id: &str) -> Result<String, reqwest::Error> {
        let path = format!(
            "/api/resources/inventory/v1/Device?key.deviceId={}",
            device_id
        );
        self.get(&path).await
    }
    pub async fn post_change_control(&self, change: String) -> Result<String, reqwest::Error> {
        let path = "/api/v3/services/ccapi.ChangeControl/Update".to_string();
        self.post(&path, change).await
    }

    pub async fn approve_change_control(
        &self,
        approval: Approval,
    ) -> Result<String, reqwest::Error> {
        let approval_json = serde_json::to_string(&approval).unwrap();
        let path = "/api/v3/services/ccapi.ChangeControl/AddApproval".to_string();
        println!("Approving: {}", &approval_json);
        self.post(&path, approval_json).await
    }
    pub async fn execute_change_control(
        &self,
        start: StartChange,
    ) -> Result<String, reqwest::Error> {
        let start_json = serde_json::to_string(&start).unwrap();
        println!("Starting: {}", &start_json);
        let path = "/api/v3/services/ccapi.ChangeControl/Start".to_string();
        self.post(&path, start_json).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_get_token_from_file() {
        let mut cv = Host::new("foo", 443, "user", "pass");
        cv.get_token_from_file("tokens/token.txt".to_string())
            .unwrap();
        if let Some(token) = cv.token {
            assert!(token.starts_with("ey"));
        } else {
            panic!("did not read file");
        }
    }
}
