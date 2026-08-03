//! Cross-language `FileId` conformance.
//!
//! `clients/README.md` says "parity is enforced, not assumed" — but the shared
//! vectors in `clients/conformance/fileid_vectors.json` were only ever loaded by
//! the Go and Java tests. The implementation those two are supposed to be in
//! parity *with* was the one nothing checked against them.
//!
//! This closes that. All three languages now run the same file, so a change to
//! the wire format fails here rather than silently diverging.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde::Deserialize;
use weedforge::FileId;

// Compiled in, so a moved or deleted vectors file is a build error rather than a
// test that quietly passes over nothing.
const VECTORS: &str = include_str!("../clients/conformance/fileid_vectors.json");

#[derive(Debug, Deserialize)]
struct Vectors {
    valid: Vec<ValidVector>,
    invalid: Vec<InvalidVector>,
}

#[derive(Debug, Deserialize)]
struct ValidVector {
    input: String,
    volume_id: u32,
    /// A string, not a number: `u64::MAX` does not survive a JSON number in
    /// every language that reads this file.
    file_key: String,
    cookie: u32,
    render: String,
    note: String,
}

#[derive(Debug, Deserialize)]
struct InvalidVector {
    input: String,
    reason: String,
}

fn vectors() -> Vectors {
    serde_json::from_str(VECTORS).expect("conformance vectors are valid JSON")
}

#[test]
fn valid_vectors_parse_and_render() {
    let vectors = vectors();
    assert!(!vectors.valid.is_empty(), "no valid vectors loaded");

    for v in &vectors.valid {
        let fid = FileId::parse(&v.input)
            .unwrap_or_else(|e| panic!("{} should parse ({}): {e}", v.input, v.note));

        let expected_key: u64 = v
            .file_key
            .parse()
            .unwrap_or_else(|e| panic!("{} has an unparseable file_key: {e}", v.input));

        assert_eq!(fid.volume_id(), v.volume_id, "volume_id of {}", v.input);
        assert_eq!(fid.file_key(), expected_key, "file_key of {}", v.input);
        assert_eq!(fid.cookie(), v.cookie, "cookie of {}", v.input);
        assert_eq!(fid.render(), v.render, "render of {}", v.input);

        // render() must be a fixed point: the canonical form has to parse back
        // to the same value, or a round trip through storage would not survive.
        let reparsed = FileId::parse(&fid.render()).expect("canonical form reparses");
        assert_eq!(reparsed, fid, "render/parse round trip of {}", v.input);
    }
}

#[test]
fn invalid_vectors_are_rejected() {
    let vectors = vectors();
    assert!(!vectors.invalid.is_empty(), "no invalid vectors loaded");

    for v in &vectors.invalid {
        assert!(
            FileId::parse(&v.input).is_err(),
            "{} should have been rejected: {}",
            v.input,
            v.reason
        );
    }
}
