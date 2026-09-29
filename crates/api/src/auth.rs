use subtle::ConstantTimeEq;
use worker::Request;

pub fn authorized(req: &Request, token: &str) -> bool {
    if token.is_empty() {
        return false;
    }
    let provided = req
        .headers()
        .get("Authorization")
        .ok()
        .flatten()
        .and_then(|h| h.strip_prefix("Bearer ").map(str::to_owned))
        .unwrap_or_default();
    bool::from(provided.as_bytes().ct_eq(token.as_bytes()))
}
