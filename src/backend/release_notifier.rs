use std::error::Error;
use std::time::Duration;

use http::header::ACCEPT;
use http::{HeaderMap, HeaderValue, StatusCode};
use reqwest::{Client, Url};
use serde_json::Value;

use crate::global::APP_USER_AGENT;
use crate::i18n::tr;
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
                "could not retrieve latest manga-tui version, more details about the api response : \n {response:#?} "
            )
            .into());
        }

        let response: Value = response.json().await?;

        // `name` may be null or empty when a release only has a tag, so `tag_name` is used as a fallback
        let version = ["name", "tag_name"]
            .iter()
            .filter_map(|key| response.get(key).and_then(Value::as_str))
            .map(str::trim)
            .find(|value| !value.is_empty())
            .ok_or("latest release has neither name nor tag_name")?;

        Ok(version.to_string())
    }

    /// Parses a version like `v1.2.3` into `(1, 2, 3)`
    fn parse_version(version: &str) -> Option<(u64, u64, u64)> {
        let mut parts = version
            .trim()
            .trim_start_matches(['v', 'V'])
            .split('.')
            .map(|part| part.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse::<u64>().ok());

        let major = parts.next()??;
        let minor = parts.next().flatten().unwrap_or(0);
        let patch = parts.next().flatten().unwrap_or(0);
        Some((major, minor, patch))
    }

    /// returns `true` if there is a new version
    fn new_version(&self, latest: &str, current: &str) -> bool {
        match (Self::parse_version(latest), Self::parse_version(current)) {
            (Some(latest), Some(current)) => latest > current,
            _ => latest != current,
        }
    }

    pub async fn check_new_releases(self, logger: &impl ILogger) -> Result<(), Box<dyn Error>> {
        logger.inform(tr("Buscando actualizaciones", "Checking for updates"));

        let latest_release = self.get_latest_release().await?;
        let current_version = format!("v{}", env!("CARGO_PKG_VERSION"));

        if self.new_version(&latest_release, &current_version) {
            let github_url = format!("https://github.com/josueBarretogit/manga-tui/releases/tag/{latest_release}");
            logger.inform(format!(
                "{} : {latest_release} → {github_url} ",
                tr("Hay una nueva versión disponible", "There is a new version available")
            ));
        } else {
            logger.inform(tr("Actualizado", "Up to date"));
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
    #[ignore]
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

        // Older releases must not be reported as updates
        assert!(!notifier.new_version("v0.9.0", "v0.10.0"));
        assert!(notifier.new_version("v0.10.1", "v0.10.0"));

        Ok(())
    }

    #[test]
    fn version_is_parsed_leniently() {
        assert_eq!(Some((0, 10, 0)), ReleaseNotifier::parse_version("v0.10.0"));
        assert_eq!(Some((1, 2, 0)), ReleaseNotifier::parse_version("1.2"));
        assert_eq!(Some((1, 2, 3)), ReleaseNotifier::parse_version("v1.2.3-beta"));
        assert_eq!(None, ReleaseNotifier::parse_version("latest"));
    }
}
