use std::error::Error;
use std::time::Duration;

use http::header::ACCEPT;
use http::{HeaderMap, HeaderValue, StatusCode};
use reqwest::{Client, Url};
use serde_json::Value;

use crate::global::APP_USER_AGENT;
use crate::logger::ILogger;

#[derive(Debug)]
pub struct ReleaseNotifier {
    github_url: Url,
    client: Client,
}

pub static GITHUB_URL: &str = "https://api.github.com/repos/josueBarretogit/manga-tui";

impl ReleaseNotifier {
    pub fn new(github_url: Url) -> Self {
        let mut default_headers = HeaderMap::new();

        default_headers.insert("X-GitHub-Api-Version", HeaderValue::from_static("2022-11-28"));
        default_headers.insert(ACCEPT, HeaderValue::from_static("application/vnd.github+json"));

        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .default_headers(default_headers)
            .user_agent(&*APP_USER_AGENT)
            .build()
            .unwrap();

        Self { github_url, client }
    }

    async fn get_latest_release(&self) -> Result<String, Box<dyn Error>> {
        let endpoint = format!("{}/releases/latest", self.github_url);

        let response = self.client.get(endpoint).send().await?;

        if response.status() != StatusCode::OK {
            return Err(format!(
                "could not retrieve latest manga-tui version, more details about the api response : \n {:#?} ",
                response
            )
            .into());
        }

        let response: Value = response.json().await?;

        let response = response.get("name").cloned().unwrap();

        Ok(response.as_str().unwrap().to_string())
    }

    /// returns `true` if there is a new version
    fn new_version(&self, latest: &str, current: &str) -> bool {
        latest != current
    }

    pub async fn check_new_releases(self, logger: &impl ILogger) -> Result<(), Box<dyn Error>> {
        logger.inform("Checking for updates");

        let latest_release = self.get_latest_release().await?;
        let current_version = format!("v{}", env!("CARGO_PKG_VERSION"));

        if self.new_version(&latest_release, &current_version) {
            let github_url = format!("https://github.com/josueBarretogit/manga-tui/releases/tag/{latest_release}");
            logger.inform(format!("There is a new version : {latest_release} to update go to the releases page: {github_url} "));
            tokio::time::sleep(Duration::from_secs(2)).await;
        } else {
            logger.inform("Up to date");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use httpmock::Method::GET;
    use httpmock::MockServer;
    use pretty_assertions::assert_str_eq;
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn it_get_latest_version_from_github() -> Result<(), Box<dyn Error>> {
        let server = MockServer::start_async().await;
        let notifier = ReleaseNotifier::new(server.base_url().parse()?);

        let release = "v0.4.0";

        let request = server
            .mock_async(|when, then| {
                when.method(GET)
                    .header("X-GitHub-Api-Version", "2022-11-28")
                    .header("Accept", "application/vnd.github+json")
                    .header("User-Agent", &*APP_USER_AGENT)
                    .path_contains("releases/latest");
                then.status(200).json_body(json!({ "name" : release }));
            })
            .await;

        let latest_release = notifier.get_latest_release().await?;

        request.assert_async().await;

        assert_str_eq!(release, latest_release);

        Ok(())
    }

    #[test]
    fn it_compares_latest_version_from_current_version() -> Result<(), Box<dyn Error>> {
        let notifier = ReleaseNotifier::new("http:/localhost".parse()?);

        let latest_version = "v0.5.0";
        let current = "v0.4.0";

        let new_version = notifier.new_version(latest_version, current);

        assert!(new_version);

        let latest_version = "v1.5.0";
        let current = "v0.4.2";

        let new_version = notifier.new_version(latest_version, current);

        assert!(new_version);

        let latest_version = "v1.5.0";
        let current = "v1.5.0";

        let new_version = notifier.new_version(latest_version, current);

        assert!(!new_version);

        Ok(())
    }
}
