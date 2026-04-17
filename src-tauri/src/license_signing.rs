// People Partner - License Signature Verification (issue #22)
//
// Verifies Ed25519-signed JWTs returned by the license server. Two defenses:
//
//   1. Response forgery. The pre-signing response shape (`{valid, reason,
//      message}`) can be forged by a user pointing `peoplepartner.io` at a
//      local HTTP intercept. Signed tokens can't be forged without the
//      private key (held only by the site server).
//
//   2. Cache portability. Every cached signed_token carries the device_id
//      it was issued for. An app reading a cached row re-verifies the
//      token and its device_id claim against the local device_id; a
//      cache stolen from another machine fails verification.
//
// Transition mode: `PUBLIC_KEY_PEM` starts as `None` so builds work today
// without coordinating a server-side key rotation. When `None`, any token
// the server returns is accepted without verification (log-only, at call
// site). When `Some`, tokens without a valid signature + claims are
// rejected as Invalid.

use serde::{Deserialize, Serialize};

/// Ed25519 public key (PEM, SubjectPublicKeyInfo form) used to verify
/// license server signatures.
///
/// `None` disables verification — transition mode during rollout.
///
/// To enable:
///   1. Generate an Ed25519 keypair (`openssl genpkey -algorithm ed25519
///      -out priv.pem` then `openssl pkey -in priv.pem -pubout -out
///      pub.pem`).
///   2. Put the **private** key in the site's Vercel env var
///      `LICENSE_SIGNING_PRIVATE_KEY` (site/lib/server/entitlements/signing.ts
///      reads from there).
///   3. Paste the **public** key contents below in PEM form, including the
///      `-----BEGIN PUBLIC KEY-----` / `-----END PUBLIC KEY-----` lines:
///
///      ```
///      const PUBLIC_KEY_PEM: Option<&str> = Some(
///          "-----BEGIN PUBLIC KEY-----\n\
///           MCowBQYDK2VwAyEA...\n\
///           -----END PUBLIC KEY-----"
///      );
///      ```
///   4. Rebuild + release. Old app versions will keep accepting unsigned
///      responses; new app versions will enforce signatures.
pub const PUBLIC_KEY_PEM: Option<&str> = None;

/// JWT claims issued by the license server. Must stay in sync with
/// site/lib/server/entitlements/signing.ts.
#[derive(Debug, Serialize, Deserialize)]
pub struct LicenseClaims {
    pub license_key: String,
    pub device_id: String,
    /// Matches `server_status` enum used in license_cache (VALID, REVOKED,
    /// INVALID, SEAT_LIMIT_EXCEEDED, LEGACY_ASSUMED_VALID).
    pub status: String,
    /// Unix seconds. JWT "iat" claim.
    pub iat: i64,
    /// Unix seconds. JWT "exp" claim. Server issues ~24h tokens.
    pub exp: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error("Signature verification failed: {0}")]
    Signature(String),
    #[error("Token claim mismatch: expected {field} '{expected}', got '{actual}'")]
    ClaimMismatch {
        field: &'static str,
        expected: String,
        actual: String,
    },
    #[error("Token expired at {exp}, now {now}")]
    Expired { exp: i64, now: i64 },
}

/// Verify a signed license token. Returns Ok iff:
///   - PUBLIC_KEY_PEM is configured,
///   - the JWT signature validates under it,
///   - claims.license_key == expected_license_key,
///   - claims.device_id == expected_device_id,
///   - claims.exp > now.
///
/// Returns Ok(None) (no-op) when PUBLIC_KEY_PEM is unconfigured — this is
/// the transition-mode behavior. Callers should treat Ok(None) as "token
/// received but not yet verifiable, accept for now".
pub fn verify_signed_token(
    token: &str,
    expected_license_key: &str,
    expected_device_id: &str,
) -> Result<Option<LicenseClaims>, VerifyError> {
    let Some(pem) = PUBLIC_KEY_PEM else {
        return Ok(None);
    };

    let key = jsonwebtoken::DecodingKey::from_ed_pem(pem.as_bytes())
        .map_err(|e| VerifyError::Signature(format!("decode public key: {e}")))?;

    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::EdDSA);
    // jsonwebtoken's default validation checks exp; we explicitly re-check
    // below so the error type is our own and the "now" comparison is
    // captured for the diagnostic.
    validation.validate_exp = false;
    // No aud/iss enforcement yet — we control both ends, and the license_key
    // + device_id claim-match below is tighter than aud/iss would be.
    validation.validate_aud = false;
    validation.required_spec_claims = std::collections::HashSet::new();

    let decoded = jsonwebtoken::decode::<LicenseClaims>(token, &key, &validation)
        .map_err(|e| VerifyError::Signature(e.to_string()))?;
    let claims = decoded.claims;

    if claims.license_key != expected_license_key {
        return Err(VerifyError::ClaimMismatch {
            field: "license_key",
            expected: expected_license_key.to_string(),
            actual: claims.license_key,
        });
    }
    if claims.device_id != expected_device_id {
        return Err(VerifyError::ClaimMismatch {
            field: "device_id",
            expected: expected_device_id.to_string(),
            actual: claims.device_id,
        });
    }

    let now = chrono::Utc::now().timestamp();
    if claims.exp <= now {
        return Err(VerifyError::Expired {
            exp: claims.exp,
            now,
        });
    }

    Ok(Some(claims))
}

/// True when signature verification is active (PUBLIC_KEY_PEM is set).
/// Used by callers that want to log "transition mode" warnings.
pub const fn signing_enabled() -> bool {
    PUBLIC_KEY_PEM.is_some()
}

#[cfg(test)]
mod tests {
    //! These tests exercise the verification code path with an ephemeral
    //! Ed25519 keypair (generated at test time via `ring`, already a
    //! transitive dep of `reqwest`'s TLS stack). Runtime `PUBLIC_KEY_PEM`
    //! is a compile-time `const` so we can't swap it in tests — instead
    //! we call a test-only `verify_with_key` shim that takes the PEM
    //! directly.

    use super::*;
    use jsonwebtoken::{encode, EncodingKey, Header};

    fn test_keypair_pem() -> (String, String) {
        // Well-known Ed25519 test vectors from RFC 8032 §7.1 (test 1).
        // PKCS#8 v1 encoding of the private key seed, PEM-wrapped.
        //
        // seed = 9d61b19deffd5a60ba844af492ec2cc4 4449c5697b326919703bac031cae7f60
        // pub  = d75a980182b10ab7d54bfed3c964073a 0ee172f3daa62325af021a68f707511a
        //
        // PKCS#8 DER:
        //   30 2e                                  SEQUENCE
        //     02 01 00                             INTEGER 0
        //     30 05                                SEQUENCE
        //       06 03 2b 65 70                     OID 1.3.101.112 (Ed25519)
        //     04 22                                OCTET STRING (34 bytes)
        //       04 20 <32-byte seed>
        let priv_pkcs8_der: [u8; 48] = [
            0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22,
            0x04, 0x20,
            // seed (32 bytes)
            0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec,
            0x2c, 0xc4, 0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03,
            0x1c, 0xae, 0x7f, 0x60,
        ];
        let priv_pem = pem_wrap("PRIVATE KEY", &priv_pkcs8_der);

        // SubjectPublicKeyInfo DER:
        //   30 2a                                  SEQUENCE
        //     30 05                                SEQUENCE
        //       06 03 2b 65 70                     OID 1.3.101.112
        //     03 21                                BIT STRING (33 bytes total)
        //       00 <32-byte pubkey>
        let pub_spki_der: [u8; 44] = [
            0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
            // pubkey (32 bytes)
            0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64,
            0x07, 0x3a, 0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68,
            0xf7, 0x07, 0x51, 0x1a,
        ];
        let pub_pem = pem_wrap("PUBLIC KEY", &pub_spki_der);

        (priv_pem, pub_pem)
    }

    fn pem_wrap(label: &str, der: &[u8]) -> String {
        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(der);
        let mut out = format!("-----BEGIN {label}-----\n");
        for chunk in b64.as_bytes().chunks(64) {
            out.push_str(std::str::from_utf8(chunk).unwrap());
            out.push('\n');
        }
        out.push_str(&format!("-----END {label}-----\n"));
        out
    }

    /// Test-only verification against a caller-supplied PEM. Mirrors
    /// `verify_signed_token` but skips the const-key lookup.
    fn verify_with_key(
        token: &str,
        expected_license_key: &str,
        expected_device_id: &str,
        pub_pem: &str,
    ) -> Result<LicenseClaims, VerifyError> {
        let key = jsonwebtoken::DecodingKey::from_ed_pem(pub_pem.as_bytes())
            .map_err(|e| VerifyError::Signature(format!("decode public key: {e}")))?;
        let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::EdDSA);
        validation.validate_exp = false;
        validation.validate_aud = false;
        validation.required_spec_claims = std::collections::HashSet::new();
        let decoded = jsonwebtoken::decode::<LicenseClaims>(token, &key, &validation)
            .map_err(|e| VerifyError::Signature(e.to_string()))?;
        let claims = decoded.claims;
        if claims.license_key != expected_license_key {
            return Err(VerifyError::ClaimMismatch {
                field: "license_key",
                expected: expected_license_key.to_string(),
                actual: claims.license_key,
            });
        }
        if claims.device_id != expected_device_id {
            return Err(VerifyError::ClaimMismatch {
                field: "device_id",
                expected: expected_device_id.to_string(),
                actual: claims.device_id,
            });
        }
        let now = chrono::Utc::now().timestamp();
        if claims.exp <= now {
            return Err(VerifyError::Expired {
                exp: claims.exp,
                now,
            });
        }
        Ok(claims)
    }

    fn sign_test_token(priv_pem: &str, claims: &LicenseClaims) -> String {
        let key = EncodingKey::from_ed_pem(priv_pem.as_bytes()).unwrap();
        let header = Header::new(jsonwebtoken::Algorithm::EdDSA);
        encode(&header, claims, &key).unwrap()
    }

    fn fresh_claims() -> LicenseClaims {
        let now = chrono::Utc::now().timestamp();
        LicenseClaims {
            license_key: "PP-TEST-1234-5678-ABCD-EF01-2345".into(),
            device_id: "device-abc".into(),
            status: "VALID".into(),
            iat: now,
            exp: now + 24 * 3600,
        }
    }

    #[test]
    fn valid_token_with_matching_claims_verifies() {
        let (priv_pem, pub_pem) = test_keypair_pem();
        let claims = fresh_claims();
        let token = sign_test_token(&priv_pem, &claims);

        let verified = verify_with_key(&token, "PP-TEST-1234-5678-ABCD-EF01-2345", "device-abc", &pub_pem)
            .expect("valid token must verify");
        assert_eq!(verified.license_key, "PP-TEST-1234-5678-ABCD-EF01-2345");
        assert_eq!(verified.device_id, "device-abc");
        assert_eq!(verified.status, "VALID");
    }

    #[test]
    fn wrong_device_id_rejected() {
        let (priv_pem, pub_pem) = test_keypair_pem();
        let claims = fresh_claims();
        let token = sign_test_token(&priv_pem, &claims);

        let err = verify_with_key(&token, "PP-TEST-1234-5678-ABCD-EF01-2345", "different-device", &pub_pem)
            .expect_err("device_id mismatch must reject");
        assert!(matches!(err, VerifyError::ClaimMismatch { field: "device_id", .. }));
    }

    #[test]
    fn wrong_license_key_rejected() {
        let (priv_pem, pub_pem) = test_keypair_pem();
        let claims = fresh_claims();
        let token = sign_test_token(&priv_pem, &claims);

        let err = verify_with_key(&token, "PP-OTHER-KEY-ZZZZ-ZZZZ-ZZZZ-ZZZZ", "device-abc", &pub_pem)
            .expect_err("license_key mismatch must reject");
        assert!(matches!(err, VerifyError::ClaimMismatch { field: "license_key", .. }));
    }

    #[test]
    fn expired_token_rejected() {
        let (priv_pem, pub_pem) = test_keypair_pem();
        let now = chrono::Utc::now().timestamp();
        let claims = LicenseClaims {
            license_key: "PP-TEST-1234-5678-ABCD-EF01-2345".into(),
            device_id: "device-abc".into(),
            status: "VALID".into(),
            iat: now - 48 * 3600,
            exp: now - 3600,
        };
        let token = sign_test_token(&priv_pem, &claims);

        let err = verify_with_key(&token, "PP-TEST-1234-5678-ABCD-EF01-2345", "device-abc", &pub_pem)
            .expect_err("expired token must reject");
        assert!(matches!(err, VerifyError::Expired { .. }));
    }

    #[test]
    fn tampered_signature_rejected() {
        let (priv_pem, pub_pem) = test_keypair_pem();
        let claims = fresh_claims();
        let token = sign_test_token(&priv_pem, &claims);

        // Flip the last byte of the signature. JWTs are header.payload.sig;
        // the final segment is the signature.
        let mut parts: Vec<&str> = token.split('.').collect();
        assert_eq!(parts.len(), 3);
        let mut sig_bytes = parts[2].as_bytes().to_vec();
        let last = sig_bytes.len() - 1;
        sig_bytes[last] = sig_bytes[last].wrapping_add(1);
        let tampered_sig = String::from_utf8(sig_bytes).unwrap();
        parts[2] = &tampered_sig;
        let tampered = parts.join(".");

        let err = verify_with_key(&tampered, "PP-TEST-1234-5678-ABCD-EF01-2345", "device-abc", &pub_pem)
            .expect_err("tampered signature must reject");
        assert!(matches!(err, VerifyError::Signature(_)));
    }

    #[test]
    fn transition_mode_returns_none() {
        // PUBLIC_KEY_PEM is None in this build (the default), so any input
        // is accepted as "unverifiable, pass-through". This is the
        // rollout-safety property: an app built before the key is baked in
        // never rejects a valid license just because it hasn't been
        // upgraded yet.
        assert!(!signing_enabled(), "test requires transition mode (PUBLIC_KEY_PEM = None)");
        let result = verify_signed_token("garbage", "anything", "anything").unwrap();
        assert!(result.is_none(), "transition mode must return Ok(None)");
    }
}
