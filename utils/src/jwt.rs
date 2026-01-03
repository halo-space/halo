//! JWT 纯工具（不含 axum/actix 依赖）。
//!
//! 当前仅提供最小能力：验证 HS256 token 是否有效（包括 exp 校验）。

use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use serde::Deserialize;
use serde_json::{Map, Value};

#[derive(Debug, Deserialize)]
struct Claims {
    /// 标准字段：exp（秒）
    #[serde(default)]
    #[allow(dead_code)]
    exp: Option<u64>,
}

/// 校验 HS256 JWT：
/// - 校验签名
/// - 校验 exp（若 token 不含 exp，则会判定为无效）
pub fn validate_hs256(token: &str, secret: &str) -> bool {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    validation.leeway = 0;
    validation.required_spec_claims.insert("exp".to_string());

    let key = DecodingKey::from_secret(secret.as_bytes());
    decode::<Claims>(token, &key, &validation).is_ok()
}

/// 解析并校验 HS256 JWT，返回 claims（MapClaims 风格）。
///
/// - 先用 `secret` 校验；失败再尝试 `prev_secret`（如提供）。
/// - 强制要求 `exp` 存在且已校验（`leeway=0`）。
pub fn decode_hs256_map_claims(
    token: &str,
    secret: &str,
    prev_secret: Option<&str>,
) -> Option<Map<String, Value>> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    validation.leeway = 0;
    validation.required_spec_claims.insert("exp".to_string());

    let try_decode = |s: &str| {
        let key = DecodingKey::from_secret(s.as_bytes());
        decode::<Map<String, Value>>(token, &key, &validation).ok()
    };

    if let Some(d) = try_decode(secret) {
        return Some(d.claims);
    }
    if let Some(ps) = prev_secret
        && let Some(d) = try_decode(ps)
    {
        return Some(d.claims);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{EncodingKey, Header, encode};
    use serde::Serialize;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[derive(Debug, Serialize)]
    struct TestClaims {
        exp: u64,
    }

    fn now_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    #[test]
    fn validate_hs256_should_accept_valid_token() {
        let secret = "s";
        let token = encode(
            &Header::default(),
            &TestClaims {
                exp: now_secs() + 60,
            },
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .unwrap();

        assert!(validate_hs256(&token, secret));
    }

    #[test]
    fn validate_hs256_should_reject_invalid_secret() {
        let token = encode(
            &Header::default(),
            &TestClaims {
                exp: now_secs() + 60,
            },
            &EncodingKey::from_secret(b"right"),
        )
        .unwrap();

        assert!(!validate_hs256(&token, "wrong"));
    }

    #[test]
    fn validate_hs256_should_reject_expired_token() {
        let secret = "s";
        let token = encode(
            &Header::default(),
            &TestClaims {
                exp: now_secs().saturating_sub(3600),
            },
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .unwrap();

        assert!(!validate_hs256(&token, secret));
    }

    #[test]
    fn decode_hs256_map_claims_should_support_prev_secret() {
        #[derive(Debug, Serialize)]
        struct C {
            exp: u64,
            uid: i64,
            sub: String,
        }

        let token = encode(
            &Header::default(),
            &C {
                exp: now_secs() + 60,
                uid: 42,
                sub: "s".to_string(),
            },
            &EncodingKey::from_secret(b"prev"),
        )
        .unwrap();

        let claims = decode_hs256_map_claims(&token, "current", Some("prev")).unwrap();
        assert_eq!(claims.get("uid").and_then(|v| v.as_i64()), Some(42));
        assert_eq!(claims.get("sub").and_then(|v| v.as_str()), Some("s"));
    }
}
