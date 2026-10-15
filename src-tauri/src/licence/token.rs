//! Signed passes from the Hearken website: `HK1.<payload>.<signature>`, where the payload is
//! base64url JSON and the signature is Ed25519 over `HK1.<payload>`. Only the website holds the
//! private key; the app checks passes offline with the public key below.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

const PREFIX: &str = "HK1";
/// The website's public key (raw Ed25519). The private key is kept off the app and its repo.
const PUBLIC_KEY: [u8; 32] = [
    0x2d, 0x67, 0x95, 0x2e, 0x9c, 0x26, 0xc5, 0xc1, 0xb6, 0x0c, 0x33, 0x3a, 0xa5, 0xd2, 0x62, 0x0d, 0x67, 0x5f, 0xbd, 0x5d, 0xa1, 0x86, 0xc6, 0x6a, 0x6a,
    0x0f, 0x69, 0xba, 0xcb, 0x2f, 0xb1, 0xe2,
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Pass {
    /// This computer's free trial, as registered on the website. Times are Unix seconds.
    #[serde(rename_all = "camelCase")]
    Trial { machine: String, starts: i64, expires: i64, issued: i64, nonce: String },
    /// A paid licence, pasted in by the church.
    #[serde(rename_all = "camelCase")]
    Licence { id: String, church: String, plan: String, issued: i64, expires: i64 },
    /// Signed in on this computer, until `expires` (renewed whenever the app is online).
    #[serde(rename_all = "camelCase")]
    Account { user: String, email: String, church: String, machine: String, issued: i64, expires: i64 },
    /// The website's clock, to settle a computer whose date looks wrong.
    #[serde(rename_all = "camelCase")]
    Time { issued: i64, nonce: String },
}

/// Checks a pass's signature and reads it. `None` if it's malformed or not signed by the website.
pub fn verify(token: &str) -> Option<Pass> {
    verify_with(token, &VerifyingKey::from_bytes(&PUBLIC_KEY).ok()?)
}

fn verify_with(token: &str, key: &VerifyingKey) -> Option<Pass> {
    let token: String = token.chars().filter(|c| !c.is_whitespace()).collect();
    let (signed, sig) = token.rsplit_once('.')?;
    let (prefix, payload) = signed.split_once('.')?;
    if prefix != PREFIX {
        return None;
    }
    let sig = Signature::from_slice(&B64.decode(sig).ok()?).ok()?;
    key.verify_strict(signed.as_bytes(), &sig).ok()?;
    serde_json::from_slice(&B64.decode(payload).ok()?).ok()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    /// Signs as the website would, with a test key.
    pub fn sign(pass: &Pass, key: &SigningKey) -> String {
        let signed = format!("{PREFIX}.{}", B64.encode(serde_json::to_vec(pass).unwrap()));
        let sig = B64.encode(key.sign(signed.as_bytes()).to_bytes());
        format!("{signed}.{sig}")
    }

    pub fn test_key() -> SigningKey {
        SigningKey::from_bytes(&[7; 32])
    }

    pub fn verify_test(token: &str) -> Option<Pass> {
        verify_with(token, &test_key().verifying_key())
    }

    #[test]
    fn a_signed_pass_reads_back_and_a_changed_one_doesnt() {
        let pass = Pass::Licence { id: "L1".into(), church: "Grace Chapel".into(), plan: "standard".into(), issued: 1, expires: 2 };
        let token = sign(&pass, &test_key());
        assert_eq!(verify_test(&token), Some(pass));
        // Pasted with line breaks and spaces: still fine.
        let wrapped: String = token.chars().enumerate().flat_map(|(i, c)| if i % 40 == 39 { vec![c, '\n', ' '] } else { vec![c] }).collect();
        assert!(verify_test(&wrapped).is_some());
        // Someone extends their licence by editing the payload: the signature no longer matches.
        let (_, payload, sig) = {
            let mut p = token.split('.');
            (p.next().unwrap(), p.next().unwrap(), p.next().unwrap())
        };
        let mut json: serde_json::Value = serde_json::from_slice(&B64.decode(payload).unwrap()).unwrap();
        json["expires"] = 9_999_999_999i64.into();
        let forged = format!("HK1.{}.{sig}", B64.encode(serde_json::to_vec(&json).unwrap()));
        assert_eq!(verify_test(&forged), None);
        // Signed by anyone else: rejected.
        assert_eq!(verify_test(&sign(&Pass::Time { issued: 1, nonce: "n".into() }, &SigningKey::from_bytes(&[9; 32]))), None);
        assert_eq!(verify_test("not a key"), None);
    }

    /// A pass signed by the website's real key (made by `scripts/issue-licence.mjs` in the site
    /// repo), so the app and the website are known to agree on the format.
    #[test]
    fn reads_a_pass_signed_by_the_website() {
        let token = include_str!("../../tests/fixtures/website-licence.txt");
        match verify(token) {
            Some(Pass::Licence { church, .. }) => assert_eq!(church, "Test Church (not for use)"),
            other => panic!("{other:?}"),
        }
    }
}
