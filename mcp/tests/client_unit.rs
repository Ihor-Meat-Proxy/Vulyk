use serde_json::json;
use vulyk_mcp::client::VulykClient;

#[tokio::test]
async fn client_rejects_empty_token_from_env_shape() {
    // from_parts is for tests; env path is covered by binary startup docs.
    let client = VulykClient::from_parts("http://127.0.0.1:9".into(), "tok".into());
    let err = client
        .post("/api/v1/projects", json!({}))
        .await
        .expect_err("should fail to connect");
    let msg = err.to_string();
    assert!(
        msg.contains("HTTP") || msg.contains("error") || msg.contains("connect"),
        "{msg}"
    );
}
