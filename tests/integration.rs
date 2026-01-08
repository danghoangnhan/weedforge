//! Integration tests for weedforge.
//!
//! These tests require a running `SeaweedFS` instance.
//! Set `SEAWEEDFS_MASTER` environment variable to run.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use weedforge::{FileId, WeedClient};

/// Test `FileId` parsing.
#[test]
fn test_file_id_parse() {
    let fid = FileId::parse("3,0000016300007037");
    assert!(fid.is_ok());

    let fid = fid.unwrap();
    assert_eq!(fid.volume_id(), 3);
}

/// Test `FileId` roundtrip.
#[test]
fn test_file_id_roundtrip() {
    let original = FileId::new(42, 0x1234, 0xABCD_EF00);
    let rendered = original.render();
    let parsed = FileId::parse(&rendered);

    assert!(parsed.is_ok());
    assert_eq!(parsed.unwrap(), original);
}

/// Test client builder validation.
#[test]
fn test_client_builder_no_urls() {
    let result = WeedClient::builder().build();
    assert!(result.is_err());
}

/// Test client builder with URL.
#[test]
fn test_client_builder_with_url() {
    let result = WeedClient::builder()
        .master_url("http://localhost:9333")
        .build();
    assert!(result.is_ok());
}

/// Integration test with real `SeaweedFS` (skipped if not available).
#[tokio::test]
#[ignore = "Requires running SeaweedFS instance"]
async fn test_write_read_delete() {
    let master_url =
        std::env::var("SEAWEEDFS_MASTER").unwrap_or_else(|_| "http://localhost:9333".to_string());

    let client = WeedClient::builder()
        .master_url(&master_url)
        .build()
        .expect("Failed to create client");

    // Write
    let data = b"Hello, SeaweedFS!".to_vec();
    let file_id = client
        .write(data.clone(), Some("test.txt"))
        .await
        .expect("Failed to write");

    println!("Uploaded file: {file_id}");

    // Read
    let downloaded = client.read(&file_id).await.expect("Failed to read");
    assert_eq!(downloaded, data);

    // Public URL
    let url = client
        .public_url(&file_id)
        .await
        .expect("Failed to get URL");
    println!("Public URL: {url}");

    // Delete
    client.delete(&file_id).await.expect("Failed to delete");
}
