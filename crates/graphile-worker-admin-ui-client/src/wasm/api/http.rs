use gloo_net::http::{Request, RequestBuilder};
use serde::de::DeserializeOwned;
use serde::Serialize;
use web_sys::RequestCredentials;

use super::super::types::{AdminClientConfig, AuthMode, ErrorResponse};

pub(super) async fn api_get<T>(
    path: &str,
    config: &AdminClientConfig,
    token: &str,
) -> Result<T, String>
where
    T: DeserializeOwned,
{
    let response = with_api_headers(Request::get(path), config, token, false)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    parse_response(response).await
}

pub(super) async fn api_post<B, T>(
    path: &str,
    body: &B,
    config: &AdminClientConfig,
    token: &str,
) -> Result<T, String>
where
    B: Serialize + ?Sized,
    T: DeserializeOwned,
{
    let request = with_api_headers(Request::post(path), config, token, true)
        .json(body)
        .map_err(|error| error.to_string())?;
    let response = request.send().await.map_err(|error| error.to_string())?;
    parse_response(response).await
}

fn with_api_headers(
    builder: RequestBuilder,
    config: &AdminClientConfig,
    token: &str,
    writes: bool,
) -> RequestBuilder {
    let mut builder = builder
        .credentials(RequestCredentials::SameOrigin)
        .header("Accept", "application/json");

    if writes {
        builder = builder.header(&config.csrf_header, &config.csrf);
    }
    if token.is_empty() {
        return builder;
    }

    match config.auth_mode {
        AuthMode::Bearer => {
            builder = builder.header("Authorization", &format!("Bearer {token}"));
        }
        AuthMode::Header if !config.auth_header.is_empty() => {
            builder = builder.header(&config.auth_header, token);
        }
        _ => {}
    }
    builder
}

async fn parse_response<T>(response: gloo_net::http::Response) -> Result<T, String>
where
    T: DeserializeOwned,
{
    let status = response.status();
    let text = response.text().await.map_err(|error| error.to_string())?;
    if !(200..300).contains(&status) {
        return Err(serde_json::from_str::<ErrorResponse>(&text)
            .map(|error| error.error)
            .unwrap_or_else(|_| format!("{status}: {text}")));
    }
    serde_json::from_str(&text).map_err(|error| error.to_string())
}
