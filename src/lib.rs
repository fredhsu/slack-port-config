pub mod cvp {
    pub struct Host {
        hostname: String,
        port: u32,
        username: String,
        password: String,
    }

    impl Host {
        pub fn new(hostname: &str, port: u32, username: &str, password: &str) -> Self {
            Host {
                hostname: hostname.to_string(),
                port,
                username: username.to_string(),
                password: password.to_string(),
            }
        }
        pub async fn get_token(&self) -> Result<String, reqwest::Error> {
            let path = "/cvpservice/login/authenticate.do";
            let url = format!("https://{}:{}{}", self.hostname, self.port, path);
            let client = reqwest::Client::new();
            let response = client
                .post(url)
                .basic_auth(&self.username, Some(&self.password))
                .send()
                .await?
                .text()
                .await?;
            Ok(response)
        }
    }
}
