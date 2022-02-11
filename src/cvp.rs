use reqwest::header::*;
use serde::{Deserialize, Serialize};
use std::fs;

pub struct Host {
    hostname: String,
    port: u32,
    username: String,
    password: String,
    token: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TokenResponse {
    token: String,
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
        let client = reqwest::Client::new();
        let response = client
            .post(url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await?
            .json::<TokenResponse>()
            .await?;
        self.token = Some(response.token);
        Ok(())
    }

    async fn get(&self, path: &str) -> Result<String, reqwest::Error> {
        if let Some(token) = &self.token {
            let url = format!("https://{}{}", self.hostname, path);
            let client = reqwest::Client::new();
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
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_get_token_from_file() {
        let mut cv = Host::new("foo", 443, "user", "pass");
        cv.get_token_from_file("token.txt".to_string());
        if let Some(token) = cv.token {
            assert!(token.starts_with("ey"));
        } else {
            panic!("did not read file");
        }
    }
}
