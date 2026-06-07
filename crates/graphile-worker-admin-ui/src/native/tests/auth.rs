use super::*;

#[test]
fn generated_secret_is_hex_and_long_enough() {
    let secret = generate_secret();
    assert_eq!(secret.len(), 48);
    assert!(secret.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn basic_auth_accepts_correct_credentials() {
    let credentials = STANDARD.encode("admin:secret");
    let request = Request::builder()
        .header(AUTHORIZATION, format!("Basic {credentials}"))
        .body(Body::empty())
        .unwrap();

    assert!(authorize_basic(request.headers(), "admin", "secret"));
    assert!(!authorize_basic(request.headers(), "admin", "wrong"));
}

#[test]
fn bearer_and_header_auth_accept_expected_tokens() {
    let bearer = Request::builder()
        .header(AUTHORIZATION, "Bearer admin-token")
        .body(Body::empty())
        .unwrap();
    assert!(AdminAuthConfig::bearer("admin-token", false).is_authorized(bearer.headers()));
    assert!(!AdminAuthConfig::bearer("other-token", false).is_authorized(bearer.headers()));

    let header = Request::builder()
        .header("x-admin-token", "header-token")
        .body(Body::empty())
        .unwrap();
    let auth = AdminAuthConfig::header("x-admin-token", "header-token", false).unwrap();
    assert!(auth.is_authorized(header.headers()));
}

#[tokio::test]
async fn unauthorized_basic_response_prompts_for_basic_auth() {
    let response = unauthorized_response(&AdminAuthConfig::basic("admin", "secret"));
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(response.headers().contains_key(WWW_AUTHENTICATE));

    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(body.contains("unauthorized"));
}
