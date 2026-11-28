//! Published AWS SigV4 test vectors and `rusty-s3` 0.10 cross-checks.
//!
//! Note on path-normalization cases in `aws-sig-v4-test-suite`:
//! The generic AWS SigV4 test suite includes `normalize-path` / `*-normalized`
//! vectors (`get-relative`, `get-slashes`, `get-slash-dot-slash`, etc.) that
//! collapse `//`, `.`, and `..` segments. Amazon S3 does **not** normalize
//! URI paths before signing, and `dyson-sigv4` intentionally preserves `//`,
//! `.`, and `..` as sent on the wire. Those path-normalization vectors
//! therefore do not apply here.

use std::collections::BTreeMap;
use std::time::Duration;

use chrono::{DateTime, Utc};
use dyson_sigv4::{
    Credentials, S3_SERVICE, UNSIGNED_PAYLOAD, authorization, payload_sha256, presign_query,
    presign_query_with_headers, presign_url,
};

fn utc(rfc3339: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(rfc3339)
        .unwrap()
        .with_timezone(&Utc)
}

fn s3_doc_creds() -> Credentials {
    Credentials::new(
        "AKIAIOSFODNN7EXAMPLE",
        "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
        "us-east-1",
    )
}

fn testsuite_creds() -> Credentials {
    Credentials::new(
        "AKIDEXAMPLE",
        "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
        "us-east-1",
    )
}

// ============================================================================
// AWS S3 Documentation: "Authenticating Requests: Using the Authorization
// Header (AWS Signature Version 4)"
// Source: https://docs.aws.amazon.com/AmazonS3/latest/API/sig-v4-header-based-auth.html
// (archived snapshot fetched from:
//  https://web.archive.org/web/20240101/https://docs.aws.amazon.com/AmazonS3/latest/API/sig-v4-header-based-auth.html)
// ============================================================================

#[test]
fn s3_doc_get_object_with_range() {
    let now = utc("2013-05-24T00:00:00Z");
    let empty = payload_sha256(b"");
    assert_eq!(
        empty,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );

    let auth = authorization(
        "GET",
        "/test.txt",
        "",
        &[
            ("Host", "examplebucket.s3.amazonaws.com"),
            ("Range", "bytes=0-9"),
            ("x-amz-content-sha256", &empty),
            ("x-amz-date", "20130524T000000Z"),
        ],
        &empty,
        &s3_doc_creds(),
        S3_SERVICE,
        now,
    )
    .unwrap();

    // Expected Signature from S3 doc "Example: GET Object":
    // f0e8bdb87c964420e857bd35b5d6ed310bd44f0170aba48dd91039c6036bdb41
    assert_eq!(
        auth,
        "AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20130524/us-east-1/s3/aws4_request, \
         SignedHeaders=host;range;x-amz-content-sha256;x-amz-date, \
         Signature=f0e8bdb87c964420e857bd35b5d6ed310bd44f0170aba48dd91039c6036bdb41"
    );
}

#[test]
fn s3_doc_put_object() {
    let now = utc("2013-05-24T00:00:00Z");
    let body_hash = payload_sha256(b"Welcome to Amazon S3.");
    assert_eq!(
        body_hash,
        "44ce7dd67c959e0d3524ffac1771dfbba87d2b6b4b4e99e42034a8b803f8b072"
    );

    let headers = [
        ("Host", "examplebucket.s3.amazonaws.com"),
        ("Date", "Fri, 24 May 2013 00:00:00 GMT"),
        ("x-amz-date", "20130524T000000Z"),
        ("x-amz-storage-class", "REDUCED_REDUNDANCY"),
        ("x-amz-content-sha256", body_hash.as_str()),
    ];

    // Expected Signature from S3 doc "Example: PUT Object":
    // 98ad721746da40c64f1a55b78f14c238d841ea1380cd77a1b5971af0ece108bd
    let expected = "AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20130524/us-east-1/s3/aws4_request, \
         SignedHeaders=date;host;x-amz-content-sha256;x-amz-date;x-amz-storage-class, \
         Signature=98ad721746da40c64f1a55b78f14c238d841ea1380cd77a1b5971af0ece108bd";

    // Both unencoded `/test$file.text` and wire-encoded `/test%24file.text`
    // (as well as relative `test$file.text` as written in the doc) yield the
    // exact same canonical URI `/test%24file.text` and signature.
    for path in ["/test$file.text", "/test%24file.text", "test$file.text"] {
        let auth = authorization(
            "PUT",
            path,
            "",
            &headers,
            &body_hash,
            &s3_doc_creds(),
            S3_SERVICE,
            now,
        )
        .unwrap();
        assert_eq!(auth, expected, "path={path}");
    }
}

#[test]
fn s3_doc_get_bucket_lifecycle() {
    let now = utc("2013-05-24T00:00:00Z");
    let empty = payload_sha256(b"");

    let auth = authorization(
        "GET",
        "/",
        "lifecycle",
        &[
            ("Host", "examplebucket.s3.amazonaws.com"),
            ("x-amz-date", "20130524T000000Z"),
            ("x-amz-content-sha256", &empty),
        ],
        &empty,
        &s3_doc_creds(),
        S3_SERVICE,
        now,
    )
    .unwrap();

    // Expected Signature from S3 doc "Example: GET Bucket Lifecycle":
    // fea454ca298b7da1c68078a5d1bdbfbbe0d65c699e0f91ac7a200a0136783543
    assert_eq!(
        auth,
        "AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20130524/us-east-1/s3/aws4_request, \
         SignedHeaders=host;x-amz-content-sha256;x-amz-date, \
         Signature=fea454ca298b7da1c68078a5d1bdbfbbe0d65c699e0f91ac7a200a0136783543"
    );
}

#[test]
fn s3_doc_get_bucket_list_objects() {
    let now = utc("2013-05-24T00:00:00Z");
    let empty = payload_sha256(b"");

    let auth = authorization(
        "GET",
        "/",
        "max-keys=2&prefix=J",
        &[
            ("Host", "examplebucket.s3.amazonaws.com"),
            ("x-amz-date", "20130524T000000Z"),
            ("x-amz-content-sha256", &empty),
        ],
        &empty,
        &s3_doc_creds(),
        S3_SERVICE,
        now,
    )
    .unwrap();

    // Expected Signature from S3 doc "Example: Get Bucket (List Objects)":
    // 34b48302e7b5fa45bde8084f4b7868a86f0a534bc59db6670ed5711ef69dc6f7
    assert_eq!(
        auth,
        "AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20130524/us-east-1/s3/aws4_request, \
         SignedHeaders=host;x-amz-content-sha256;x-amz-date, \
         Signature=34b48302e7b5fa45bde8084f4b7868a86f0a534bc59db6670ed5711ef69dc6f7"
    );
}

// ============================================================================
// AWS S3 Documentation: "Authenticating Requests: Using Query Parameters
// (AWS Signature Version 4)"
// Source: https://docs.aws.amazon.com/AmazonS3/latest/API/sigv4-query-string-auth.html
// (archived snapshot fetched from:
//  https://web.archive.org/web/20240101/https://docs.aws.amazon.com/AmazonS3/latest/API/sigv4-query-string-auth.html)
// ============================================================================

#[test]
fn s3_doc_presigned_get_object() {
    let now = utc("2013-05-24T00:00:00Z");
    let expected_query = concat!(
        "X-Amz-Algorithm=AWS4-HMAC-SHA256",
        "&X-Amz-Credential=AKIAIOSFODNN7EXAMPLE%2F20130524%2Fus-east-1%2Fs3%2Faws4_request",
        "&X-Amz-Date=20130524T000000Z",
        "&X-Amz-Expires=86400",
        "&X-Amz-SignedHeaders=host",
        "&X-Amz-Signature=aeeed9bbccd4d02ee5c0109b86d86835f995330da4c265957d157751f604d404",
    );

    let query = presign_query(
        "GET",
        "examplebucket.s3.amazonaws.com",
        "/test.txt",
        &[],
        86400,
        &s3_doc_creds(),
        S3_SERVICE,
        now,
    )
    .unwrap();
    assert_eq!(query, expected_query);

    let url = presign_url(
        "https",
        "GET",
        "examplebucket.s3.amazonaws.com",
        "/test.txt",
        &[],
        86400,
        &s3_doc_creds(),
        S3_SERVICE,
        now,
    )
    .unwrap();
    assert_eq!(
        url,
        format!("https://examplebucket.s3.amazonaws.com/test.txt?{expected_query}")
    );
}

// ============================================================================
// AWS SigV4 Test Suite (`aws-sig-v4-test-suite`)
// Verified identically against both official AWS repositories:
// - https://github.com/awslabs/aws-c-auth/tree/main/tests/aws-signing-test-suite/v4
// - https://github.com/boto/botocore/tree/develop/tests/unit/auth/aws4_testsuite
//
// All cases below use:
//   service = "service", region = "us-east-1", date = 2015-08-30T12:36:00Z,
//   access_key_id = "AKIDEXAMPLE",
//   secret_access_key = "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY".
// ============================================================================

#[test]
fn aws_testsuite_get_vanilla() {
    let now = utc("2015-08-30T12:36:00Z");
    let empty = payload_sha256(b"");
    let auth = authorization(
        "GET",
        "/",
        "",
        &[
            ("Host", "example.amazonaws.com"),
            ("X-Amz-Date", "20150830T123600Z"),
        ],
        &empty,
        &testsuite_creds(),
        "service",
        now,
    )
    .unwrap();
    // Source: get-vanilla/header-signature.txt & get-vanilla.authz
    assert_eq!(
        auth,
        "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request, \
         SignedHeaders=host;x-amz-date, \
         Signature=5fa00fa31553b73ebf1942676e86291e8372ff2a2260956d9b8aae1d763fbf31"
    );
}

#[test]
fn aws_testsuite_get_vanilla_query_order_key_case() {
    let now = utc("2015-08-30T12:36:00Z");
    let empty = payload_sha256(b"");
    let auth = authorization(
        "GET",
        "/",
        "Param2=value2&Param1=value1",
        &[
            ("Host", "example.amazonaws.com"),
            ("X-Amz-Date", "20150830T123600Z"),
        ],
        &empty,
        &testsuite_creds(),
        "service",
        now,
    )
    .unwrap();
    // Source: get-vanilla-query-order-key-case/header-signature.txt & .authz
    assert_eq!(
        auth,
        "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request, \
         SignedHeaders=host;x-amz-date, \
         Signature=b97d918cfa904a5beff61c982a1b6f458b799221646efd99d3219ec94cdf2500"
    );
}

#[test]
fn aws_testsuite_get_unreserved() {
    let now = utc("2015-08-30T12:36:00Z");
    let empty = payload_sha256(b"");
    let auth = authorization(
        "GET",
        "/-._~0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz",
        "",
        &[
            ("Host", "example.amazonaws.com"),
            ("X-Amz-Date", "20150830T123600Z"),
        ],
        &empty,
        &testsuite_creds(),
        "service",
        now,
    )
    .unwrap();
    // Source: get-unreserved/header-signature.txt & .authz
    assert_eq!(
        auth,
        "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request, \
         SignedHeaders=host;x-amz-date, \
         Signature=07ef7494c76fa4850883e2b006601f940f8a34d404d0cfa977f52a65bbf5f24f"
    );
}

#[test]
fn aws_testsuite_get_utf8() {
    let now = utc("2015-08-30T12:36:00Z");
    let empty = payload_sha256(b"");
    let auth = authorization(
        "GET",
        "/ሴ",
        "",
        &[
            ("Host", "example.amazonaws.com"),
            ("X-Amz-Date", "20150830T123600Z"),
        ],
        &empty,
        &testsuite_creds(),
        "service",
        now,
    )
    .unwrap();
    // Source: get-utf8/header-signature.txt & .authz
    assert_eq!(
        auth,
        "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request, \
         SignedHeaders=host;x-amz-date, \
         Signature=8318018e0b0f223aa2bbf98705b62bb787dc9c0e678f255a891fd03141be5d85"
    );
}

#[test]
fn aws_testsuite_get_header_value_trim() {
    let now = utc("2015-08-30T12:36:00Z");
    let empty = payload_sha256(b"");
    let auth = authorization(
        "GET",
        "/",
        "",
        &[
            ("Host", "example.amazonaws.com"),
            ("My-Header1", " value1"),
            ("My-Header2", " \"a   b   c\""),
            ("X-Amz-Date", "20150830T123600Z"),
        ],
        &empty,
        &testsuite_creds(),
        "service",
        now,
    )
    .unwrap();
    // Source: get-header-value-trim/header-signature.txt & .authz
    assert_eq!(
        auth,
        "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request, \
         SignedHeaders=host;my-header1;my-header2;x-amz-date, \
         Signature=acc3ed3afb60bb290fc8d2dd0098b9911fcaa05412b367055dee359757a9c736"
    );
}

#[test]
fn aws_testsuite_get_header_key_duplicate() {
    let now = utc("2015-08-30T12:36:00Z");
    let empty = payload_sha256(b"");
    let auth = authorization(
        "GET",
        "/",
        "",
        &[
            ("Host", "example.amazonaws.com"),
            ("My-Header1", "value2"),
            ("My-Header1", "value2"),
            ("My-Header1", "value1"),
            ("X-Amz-Date", "20150830T123600Z"),
        ],
        &empty,
        &testsuite_creds(),
        "service",
        now,
    )
    .unwrap();
    // Source: get-header-key-duplicate/header-signature.txt & .authz
    assert_eq!(
        auth,
        "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request, \
         SignedHeaders=host;my-header1;x-amz-date, \
         Signature=c9d5ea9f3f72853aea855b47ea873832890dbdd183b4468f858259531a5138ea"
    );
}

#[test]
fn aws_testsuite_post_vanilla() {
    let now = utc("2015-08-30T12:36:00Z");
    let empty = payload_sha256(b"");
    let auth = authorization(
        "POST",
        "/",
        "",
        &[
            ("Host", "example.amazonaws.com"),
            ("X-Amz-Date", "20150830T123600Z"),
        ],
        &empty,
        &testsuite_creds(),
        "service",
        now,
    )
    .unwrap();
    // Source: post-vanilla/header-signature.txt & .authz
    assert_eq!(
        auth,
        "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request, \
         SignedHeaders=host;x-amz-date, \
         Signature=5da7c1a2acd57cee7505fc6676e4e544621c30862966e37dddb68e92efbe5d6b"
    );
}

#[test]
fn aws_testsuite_additional_query_and_header_vectors() {
    let now = utc("2015-08-30T12:36:00Z");
    let empty = payload_sha256(b"");
    let creds = testsuite_creds();
    let base_headers = [
        ("Host", "example.amazonaws.com"),
        ("X-Amz-Date", "20150830T123600Z"),
    ];

    // Source: boto/botocore aws4_testsuite/get-vanilla-query-order-value/*.authz
    assert_eq!(
        authorization(
            "GET",
            "/",
            "Param1=value2&Param1=value1",
            &base_headers,
            &empty,
            &creds,
            "service",
            now
        )
        .unwrap(),
        "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request, \
         SignedHeaders=host;x-amz-date, \
         Signature=5772eed61e12b33fae39ee5e7012498b51d56abc0abb7c60486157bd471c4694"
    );

    // Source: boto/botocore aws4_testsuite/get-vanilla-query-order-key/*.authz
    assert_eq!(
        authorization(
            "GET",
            "/",
            "Param1=value2&Param1=Value1",
            &base_headers,
            &empty,
            &creds,
            "service",
            now
        )
        .unwrap(),
        "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request, \
         SignedHeaders=host;x-amz-date, \
         Signature=eedbc4e291e521cf13422ffca22be7d2eb8146eecf653089df300a15b2382bd1"
    );

    // Source: awslabs/aws-c-auth & boto/botocore get-vanilla-query-order-encoded
    assert_eq!(
        authorization(
            "GET",
            "/",
            "Param-3=Value3&Param=Value2&%E1%88%B4=Value1",
            &base_headers,
            &empty,
            &creds,
            "service",
            now
        )
        .unwrap(),
        "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request, \
         SignedHeaders=host;x-amz-date, \
         Signature=371d3713e185cc334048618a97f809c9ffe339c62934c032af5a0e595648fcac"
    );

    // Source: awslabs/aws-c-auth & boto/botocore get-vanilla-query-unreserved
    assert_eq!(
        authorization(
            "GET",
            "/",
            "-._~0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz=-._~0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz",
            &base_headers,
            &empty,
            &creds,
            "service",
            now,
        )
        .unwrap(),
        "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request, \
         SignedHeaders=host;x-amz-date, \
         Signature=9c3e54bfcdf0b19771a7f523ee5669cdf59bc7cc0884027167c21bb143a40197"
    );

    // Source: awslabs/aws-c-auth & boto/botocore get-vanilla-utf8-query
    assert_eq!(
        authorization(
            "GET",
            "/",
            "ሴ=bar",
            &base_headers,
            &empty,
            &creds,
            "service",
            now
        )
        .unwrap(),
        "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request, \
         SignedHeaders=host;x-amz-date, \
         Signature=2cdec8eed098649ff3a119c94853b13c643bcf08f8b0a1d91e12c9027818dd04"
    );

    // Source: boto/botocore aws4_testsuite/get-header-value-order/*.authz
    assert_eq!(
        authorization(
            "GET",
            "/",
            "",
            &[
                ("Host", "example.amazonaws.com"),
                ("My-Header1", "value4"),
                ("My-Header1", "value1"),
                ("My-Header1", "value3"),
                ("My-Header1", "value2"),
                ("X-Amz-Date", "20150830T123600Z"),
            ],
            &empty,
            &creds,
            "service",
            now,
        )
        .unwrap(),
        "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request, \
         SignedHeaders=host;my-header1;x-amz-date, \
         Signature=08c7e5a9acfcfeb3ab6b2185e75ce8b1deb5e634ec47601a50643f830c755c01"
    );

    // Source: boto/botocore aws4_testsuite/post-header-key-sort/*.authz
    assert_eq!(
        authorization(
            "POST",
            "/",
            "",
            &[
                ("X-Amz-Date", "20150830T123600Z"),
                ("My-Header1", "value1"),
                ("Host", "example.amazonaws.com"),
            ],
            &empty,
            &creds,
            "service",
            now,
        )
        .unwrap(),
        "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request, \
         SignedHeaders=host;my-header1;x-amz-date, \
         Signature=c5410059b04c1ee005303aed430f6e6645f61f4dc9e1461ec8f8916fdf18852c"
    );
}

// ============================================================================
// Broker presigning scenarios & `rusty-s3` 0.10 cross-checks
// ============================================================================

fn parse_query_map(raw_query: &str) -> BTreeMap<String, String> {
    raw_query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (k.to_owned(), v.to_owned())
        })
        .collect()
}

fn jiff_ts(dt: DateTime<Utc>) -> jiff::Timestamp {
    jiff::Timestamp::from_second(dt.timestamp()).unwrap()
}

#[test]
fn rusty_s3_crosscheck_presigned_get_with_version_and_response_overrides() {
    use rusty_s3::{Bucket, S3Action as _, UrlStyle};

    let now = utc("2026-10-15T12:34:56Z");
    let expires = 900;
    let key_id = "AKIAIOSFODNN7EXAMPLE";
    let secret = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY";
    let region = "eu-west-1";
    let bucket_name = "my-snapshots";
    let object_key = "team/2026/report final+v2.pdf";
    let disposition = "attachment; filename=\"my report; v2.pdf\"";

    let rs3_bucket = Bucket::new(
        "https://s3.eu-west-1.amazonaws.com".parse().unwrap(),
        UrlStyle::VirtualHost,
        bucket_name,
        region,
    )
    .unwrap();
    let rs3_creds = rusty_s3::Credentials::new(key_id, secret);
    let mut action = rs3_bucket.get_object(Some(&rs3_creds), object_key);
    action.query_mut().insert("versionId", "ver/123+abc=def");
    action
        .query_mut()
        .insert("response-content-type", "application/pdf");
    action
        .query_mut()
        .insert("response-content-disposition", disposition);
    let rs3_url = action.sign_with_time(Duration::from_secs(expires), &jiff_ts(now));

    let our_creds = Credentials::new(key_id, secret, region);
    let our_url = presign_url(
        "https",
        "GET",
        "my-snapshots.s3.eu-west-1.amazonaws.com",
        &format!("/{object_key}"),
        &[
            ("versionId", "ver/123+abc=def"),
            ("response-content-type", "application/pdf"),
            ("response-content-disposition", disposition),
        ],
        expires,
        &our_creds,
        S3_SERVICE,
        now,
    )
    .unwrap();

    let rs3_parsed = url::Url::parse(rs3_url.as_str()).unwrap();
    let our_parsed = url::Url::parse(&our_url).unwrap();
    assert_eq!(our_parsed.host_str(), rs3_parsed.host_str());
    assert_eq!(our_parsed.path(), rs3_parsed.path());

    let rs3_q = parse_query_map(rs3_parsed.query().unwrap());
    let our_q = parse_query_map(our_parsed.query().unwrap());
    assert_eq!(
        our_q.get("X-Amz-Signature"),
        rs3_q.get("X-Amz-Signature"),
        "dyson-sigv4 vs rusty-s3 0.10 signature mismatch:\nours: {our_url}\nrs3:  {rs3_url}"
    );
    assert_eq!(our_q, rs3_q);
    assert_eq!(
        our_q
            .get("response-content-disposition")
            .map(String::as_str),
        Some("attachment%3B%20filename%3D%22my%20report%3B%20v2.pdf%22")
    );
}

#[test]
fn rusty_s3_crosscheck_presigned_put_path_style_with_port() {
    use rusty_s3::{Bucket, S3Action as _, UrlStyle};

    let now = utc("2026-10-15T08:00:00Z");
    let expires = 3600;
    let key_id = "AKIDTEST123";
    let secret = "secret/key+with=chars";
    let region = "us-east-1";

    let rs3_bucket = Bucket::new(
        "https://minio.internal:9000".parse().unwrap(),
        UrlStyle::Path,
        "backups",
        region,
    )
    .unwrap();
    let rs3_creds = rusty_s3::Credentials::new(key_id, secret);
    let action = rs3_bucket.put_object(Some(&rs3_creds), "dir/snap café+1.tar.zst");
    let rs3_url = action.sign_with_time(Duration::from_secs(expires), &jiff_ts(now));

    let our_creds = Credentials::new(key_id, secret, region);
    let our_query = presign_query(
        "PUT",
        "minio.internal:9000",
        "/backups/dir/snap café+1.tar.zst",
        &[],
        expires,
        &our_creds,
        S3_SERVICE,
        now,
    )
    .unwrap();

    let rs3_q = parse_query_map(rs3_url.query().unwrap());
    let our_q = parse_query_map(&our_query);
    assert_eq!(our_q.get("X-Amz-Signature"), rs3_q.get("X-Amz-Signature"));
    assert_eq!(our_q, rs3_q);
}

#[test]
fn rusty_s3_crosscheck_presigned_multipart_upload_part() {
    use rusty_s3::{Bucket, S3Action as _, UrlStyle};

    let now = utc("2026-10-15T18:45:00Z");
    let expires = 1800;
    let key_id = "AKIDMULTIPART";
    let secret = "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY";
    let region = "us-west-2";
    let upload_id = "upload/id+with=special~chars_1";

    let rs3_bucket = Bucket::new(
        "https://s3.us-west-2.amazonaws.com".parse().unwrap(),
        UrlStyle::VirtualHost,
        "artifacts",
        region,
    )
    .unwrap();
    let rs3_creds = rusty_s3::Credentials::new(key_id, secret);
    let action = rs3_bucket.upload_part(Some(&rs3_creds), "builds/app.bin", 2, upload_id);
    let rs3_url = action.sign_with_time(Duration::from_secs(expires), &jiff_ts(now));

    let our_creds = Credentials::new(key_id, secret, region);
    let our_query = presign_query(
        "PUT",
        "artifacts.s3.us-west-2.amazonaws.com",
        "/builds/app.bin",
        &[("partNumber", "2"), ("uploadId", upload_id)],
        expires,
        &our_creds,
        S3_SERVICE,
        now,
    )
    .unwrap();

    let rs3_q = parse_query_map(rs3_url.query().unwrap());
    let our_q = parse_query_map(&our_query);
    assert_eq!(our_q.get("X-Amz-Signature"), rs3_q.get("X-Amz-Signature"));
    assert_eq!(our_q, rs3_q);
}

#[test]
fn rusty_s3_crosscheck_presigned_put_with_extra_signed_header() {
    use rusty_s3::{Bucket, S3Action as _, UrlStyle};

    let now = utc("2026-10-15T18:45:00Z");
    let expires = 600;
    let key_id = "AKIDEXTRA";
    let secret = "secret-value";
    let region = "us-east-1";

    let rs3_bucket = Bucket::new(
        "https://s3.amazonaws.com".parse().unwrap(),
        UrlStyle::VirtualHost,
        "uploads",
        region,
    )
    .unwrap();
    let rs3_creds = rusty_s3::Credentials::new(key_id, secret);
    let mut action = rs3_bucket.put_object(Some(&rs3_creds), "data.json");
    action
        .headers_mut()
        .insert("content-type", "application/json");
    let rs3_url = action.sign_with_time(Duration::from_secs(expires), &jiff_ts(now));

    let our_creds = Credentials::new(key_id, secret, region);
    let our_query = presign_query_with_headers(
        "PUT",
        "/data.json",
        &[],
        &[
            ("Host", "uploads.s3.amazonaws.com"),
            ("Content-Type", "application/json"),
        ],
        expires,
        &our_creds,
        S3_SERVICE,
        now,
    )
    .unwrap();

    let rs3_q = parse_query_map(rs3_url.query().unwrap());
    let our_q = parse_query_map(&our_query);
    assert_eq!(our_q.get("X-Amz-Signature"), rs3_q.get("X-Amz-Signature"));
    assert_eq!(our_q, rs3_q);
}

#[test]
fn egress_proxy_unsigned_payload_path_and_virtual_hosted_signing() {
    let now = utc("2026-10-15T12:00:00Z");
    let creds = Credentials::new("AKIDPROXY", "proxy-secret", "auto");

    // Path-style with custom service and query string
    let auth_path = authorization(
        "PUT",
        "/bucket/a+b/file%2Fname.txt",
        "partNumber=2&uploadId=abc%2Fdef",
        &[
            ("Host", "account.r2.cloudflarestorage.com"),
            ("X-Amz-Content-Sha256", UNSIGNED_PAYLOAD),
            ("X-Amz-Date", "20261015T120000Z"),
            ("Content-Type", "  application/octet-stream  "),
        ],
        UNSIGNED_PAYLOAD,
        &creds,
        S3_SERVICE,
        now,
    )
    .unwrap();
    assert!(auth_path.contains("Credential=AKIDPROXY/20261015/auto/s3/aws4_request"));
    assert!(auth_path.contains("SignedHeaders=content-type;host;x-amz-content-sha256;x-amz-date"));

    // Virtual-hosted-style with non-s3 service name
    let auth_sts = authorization(
        "POST",
        "/",
        "Action=GetCallerIdentity&Version=2011-06-15",
        &[
            ("Host", "sts.us-east-1.amazonaws.com"),
            ("X-Amz-Date", "20261015T120000Z"),
        ],
        UNSIGNED_PAYLOAD,
        &creds,
        "sts",
        now,
    )
    .unwrap();
    assert!(auth_sts.contains("Credential=AKIDPROXY/20261015/auto/sts/aws4_request"));
    assert!(auth_sts.contains("SignedHeaders=host;x-amz-date"));
}
