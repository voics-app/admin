//! Cloudflare Access JWT verification against a stubbed certs endpoint, with real RS256 keys.

use admin_bff::application::ports::access_verifier::{AccessTokenError, AccessTokenVerifier};
use admin_bff::infrastructure::cloudflare::access_verifier::CloudflareAccessVerifier;
use admin_bff::infrastructure::http::internal_client::http_client;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use rsa::pkcs1::EncodeRsaPrivateKey;
use rsa::traits::PublicKeyParts;
use rsa::RsaPrivateKey;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const ISSUER: &str = "https://acme.cloudflareaccess.com";
const AUDIENCE: &str = "aud-tag-123";

struct SigningKey {
    kid: String,
    encoding: EncodingKey,
    jwk: serde_json::Value,
}

fn signing_key(kid: &str) -> SigningKey {
    let private = RsaPrivateKey::new(&mut rand_core(), 2048).expect("rsa key");
    let public = private.to_public_key();
    let pem = private.to_pkcs1_pem(rsa::pkcs8::LineEnding::LF).expect("pem");
    let jwk = json!({
        "kid": kid, "kty": "RSA", "alg": "RS256", "use": "sig",
        "n": URL_SAFE_NO_PAD.encode(public.n().to_bytes_be()),
        "e": URL_SAFE_NO_PAD.encode(public.e().to_bytes_be()),
    });
    SigningKey { kid: kid.into(), encoding: EncodingKey::from_rsa_pem(pem.as_bytes()).expect("encoding key"), jwk }
}

fn rand_core() -> rsa::rand_core::OsRng {
    rsa::rand_core::OsRng
}

fn token(key: &SigningKey, claims: serde_json::Value) -> String {
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(key.kid.clone());
    encode(&header, &claims, &key.encoding).expect("token")
}

fn claims(email: &str, aud: &str, iss: &str, exp_offset: i64) -> serde_json::Value {
    let now = chrono::Utc::now().timestamp();
    json!({ "email": email, "aud": [aud], "iss": iss, "iat": now, "exp": now + exp_offset, "sub": "abc" })
}

async fn certs_server(keys: &[&SigningKey]) -> MockServer {
    let server = MockServer::start().await;
    let jwks: Vec<_> = keys.iter().map(|k| k.jwk.clone()).collect();
    Mock::given(method("GET"))
        .and(path("/cdn-cgi/access/certs"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "keys": jwks, "public_cert": {"kid": "x", "cert": "..."} })))
        .mount(&server)
        .await;
    server
}

fn verifier(server: &MockServer) -> CloudflareAccessVerifier {
    CloudflareAccessVerifier::with_certs_url(http_client(), ISSUER, &format!("{}/cdn-cgi/access/certs", server.uri()), AUDIENCE)
}

#[tokio::test]
async fn access_assertions_are_verified_end_to_end() {
    let key = signing_key("kid-1");
    let other = signing_key("kid-2");
    let server = certs_server(&[&key]).await;
    let verifier = verifier(&server);

    // A valid assertion yields its email.
    let email = verifier.verify(&token(&key, claims("ops@discordy.dev", AUDIENCE, ISSUER, 300))).await.unwrap();
    assert_eq!(email, "ops@discordy.dev");

    // Wrong audience, wrong issuer and expired tokens are rejected.
    for bad in [
        claims("ops@discordy.dev", "another-app", ISSUER, 300),
        claims("ops@discordy.dev", AUDIENCE, "https://evil.cloudflareaccess.com", 300),
        claims("ops@discordy.dev", AUDIENCE, ISSUER, -600),
    ] {
        assert!(matches!(verifier.verify(&token(&key, bad)).await, Err(AccessTokenError::Invalid(_))));
    }

    // A token signed by a key the team never published is rejected.
    assert!(matches!(verifier.verify(&token(&other, claims("ops@discordy.dev", AUDIENCE, ISSUER, 300))).await, Err(AccessTokenError::Invalid(_))));

    // A token without an email claim is rejected.
    let mut no_email = claims("", AUDIENCE, ISSUER, 300);
    no_email.as_object_mut().unwrap().remove("email");
    assert!(matches!(verifier.verify(&token(&key, no_email)).await, Err(AccessTokenError::Invalid(_))));

    // Garbage and HS256 tokens are rejected before any key lookup.
    assert!(matches!(verifier.verify("not-a-jwt").await, Err(AccessTokenError::Invalid(_))));
    let hs = encode(&Header::new(Algorithm::HS256), &claims("ops@discordy.dev", AUDIENCE, ISSUER, 300), &EncodingKey::from_secret(b"s")).unwrap();
    assert!(matches!(verifier.verify(&hs).await, Err(AccessTokenError::Invalid(_))));
}

#[tokio::test]
async fn an_unreachable_certs_endpoint_is_a_key_outage() {
    let key = signing_key("kid-1");
    let verifier = CloudflareAccessVerifier::with_certs_url(http_client(), ISSUER, "http://127.0.0.1:9/cdn-cgi/access/certs", AUDIENCE);
    let result = verifier.verify(&token(&key, claims("ops@discordy.dev", AUDIENCE, ISSUER, 300))).await;
    assert!(matches!(result, Err(AccessTokenError::KeysUnavailable(_))));
}
