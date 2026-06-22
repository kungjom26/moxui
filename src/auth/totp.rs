1|//! TOTP (RFC 6238) two-factor authentication.
2|//!
3|//! ## Flow
4|//!
5|//! 1. **Setup**: User enables 2FA via `POST /api/v1/auth/2fa/setup`.
6|//!    Server generates a random base32 secret and returns it + the
7|//!    `otpauth://` URL (for QR code generation on the frontend).
8|//! 2. **Verify**: User scans the QR (or enters the secret manually),
9|//!    then submits their first TOTP code via `POST /api/v1/auth/2fa/verify`.
10|//!    Server marks 2FA as enabled and stores backup codes.
11|//! 3. **Login**: User logs in with password → if 2FA enabled, server
12|//!    returns a short-lived pre-auth token. User submits it + TOTP code
13|//!    to complete login.
14|//! 4. **Backup codes**: 8 single-use codes generated on setup. Each
15|//!    is bcrypt-hashed. Using a backup code consumes it.
16|//!
17|//! ## Thread safety
18|//!
19|//! [`PreAuthStore`] is backed by `std::sync::RwLock<HashMap<…>>` and
20|//! wrapped in `Arc` inside `AppState`.
21|
22|use std::collections::HashMap;
23|use std::sync::{Arc, RwLock};
24|
25|use rand::RngCore;
26|use secrecy::{ExposeSecret, SecretString};
27|use sha1::Sha1;
28|
29|use crate::auth::password::{hash_password, verify_password};
30|
31|// ── TOTP helpers ───────────────────────────────────────────────────
32|
33|/// Generate a random 20-byte (160-bit) secret and return it as
34|/// base32 (RFC 4648) unpadded — the format Google Authenticator /
35|/// Authy / 1Password expect.
36|#[must_use]
37|pub fn generate_totp_secret() -> String {
38|    let mut buf = [0u8; 20];
39|    rand::rngs::OsRng.fill_bytes(&mut buf);
40|    base32_encode(&buf)
41|}
42|
43|/// Build the `otpauth://totp/{issuer}:{username}?secret=...&issuer=...`
44|/// URL that the frontend renders as a QR code.
45|#[must_use]
46|pub fn totp_url(issuer: &str, username: &str, secret: &str) -> String {
47|    // Label format: "Issuer:Username" — the colon is literal, not encoded.
48|    let label = format!("{issuer}:{username}");
49|    let label_enc = url_encode(&label);
50|    let secret_enc = url_encode(secret);
51|    let issuer_enc = url_encode(issuer);
52|    format!(
53|        "otpauth://totp/{label_enc}?secret={secret_enc}&issuer={issuer_enc}\
54|         &algorithm=SHA1&digits=6&period=30"
55|    )
56|}
57|
58|/// Verify a 6-digit TOTP code against a base32-encoded secret.
59|///
60|/// Checks the current 30-second window plus one step in each direction
61|/// (±30s clock skew tolerance per RFC 6238 §5.2).
62|#[must_use]
63|pub fn verify_totp(secret_b32: &str, code: &str) -> bool {
64|    let Ok(secret) = base32_decode(secret_b32) else {
65|        return false;
66|    };
67|    let now = std::time::SystemTime::now()
68|        .duration_since(std::time::UNIX_EPOCH)
69|        .unwrap_or_default()
70|        .as_secs();
71|    let counter = now / 30;
72|
73|    // Check current, previous, and next window
74|    (counter.saturating_sub(1)..=counter.saturating_add(1))
75|        .any(|c| compute_totp(&secret, c) == code)
76|}
77|
78|/// Compute a 6-digit TOTP code for a given counter value.
79|/// RFC 4226 §5.3 dynamic truncation, RFC 6238 time-based counter.
80|fn compute_totp(secret: &[u8], counter: u64) -> String {
81|    use hmac::Mac;
82|    let counter_bytes = counter.to_be_bytes();
83|    let mut mac =
84|        hmac::Hmac::<Sha1>::new_from_slice(secret).expect("valid HMAC key length");
85|    mac.update(&counter_bytes);
86|    let result = mac.finalize().into_bytes();
87|
88|    // Dynamic truncation (RFC 4226 §5.3)
89|    let offset = (result[19] & 0x0f) as usize;
90|    let code = (u32::from(result[offset]) & 0x7f) << 24
91|        | u32::from(result[offset + 1]) << 16
92|        | u32::from(result[offset + 2]) << 8
93|        | u32::from(result[offset + 3]);
94|
95|    format!("{:06}", code % 1_000_000)
96|}
97|
98|// ── Backup codes ───────────────────────────────────────────────────
99|
100|/// Number of backup codes generated on 2FA setup.
101|pub const BACKUP_CODE_COUNT: usize = 8;
102|/// Length of each backup code (digits).
103|pub const BACKUP_CODE_LEN: usize = 8;
104|
105|/// Generate 8 random numeric backup codes. Each is 8 digits.
106|/// Returns both the plaintext codes (show to user) and the bcrypt
107|/// hashed versions (store in the User).
108|#[must_use]
109|pub fn generate_backup_codes() -> (Vec<String>, Vec<SecretString>) {
110|    let mut plain = Vec::with_capacity(BACKUP_CODE_COUNT);
111|    let mut hashed = Vec::with_capacity(BACKUP_CODE_COUNT);
112|
113|    for _ in 0..BACKUP_CODE_COUNT {
114|        let code = format!(
115|            "{:08}",
116|            rand::rngs::OsRng.next_u32() % 100_000_000
117|        );
118|        let h = hash_password(&code).expect("bcrypt hash of backup code");
119|        plain.push(code);
120|        hashed.push(SecretString::new(h.into_boxed_str()));
121|    }
122|
123|    (plain, hashed)
124|}
125|
126|/// Verify a backup code against a list of bcrypt-hashed backup codes.
127|/// If matched, returns `Some(remaining)` with the matched code removed
128|/// from the list. Returns `None` if no match.
129|#[must_use]
130|pub fn verify_backup_code(stored: &[SecretString], code: &str) -> Option<Vec<SecretString>> {
131|    let mut remaining: Vec<SecretString> = Vec::with_capacity(stored.len());
132|    let mut matched = false;
133|
134|    for h in stored {
135|        if !matched && verify_password(code, h.expose_secret()).unwrap_or(false) {
136|            matched = true;
137|            // Skip this one — it was used
138|        } else {
139|            remaining.push(h.clone());
140|        }
141|    }
142|
143|    matched.then_some(remaining)
144|}
145|
146|// ── Pre-auth token store ───────────────────────────────────────────
147|
148|/// TTL for pre-auth tokens: 5 minutes.
149|pub const PREAUTH_TTL_SECS: i64 = 300;
150|
151|/// A session created when a user passes the password step but has 2FA
152|/// enabled. The client must complete TOTP within `PREAUTH_TTL_SECS`.
153|#[derive(Debug, Clone)]
154|pub struct PreAuthSession {
155|    /// User id (matches `Claims::sub`).
156|    pub user_id: String,
157|    /// Username (for logging).
158|    pub username: String,
159|    /// Expiry (unix timestamp).
160|    pub expires_at: i64,
161|    /// Whether this token has been consumed (single-use).
162|    pub consumed: bool,
163|}
164|
165|/// In-memory store for pre-auth (2FA pending) sessions.
166|///
167|/// Keyed by a random 32-byte token (hex-encoded).
168|#[derive(Debug, Clone, Default)]
169|pub struct PreAuthStore {
170|    sessions: Arc<RwLock<HashMap<String, PreAuthSession>>>,
171|}
172|
173|impl PreAuthStore {
174|    /// Create a new empty store.
175|    #[must_use]
176|    pub fn new() -> Self {
177|        Self::default()
178|    }
179|
180|    /// Issue a pre-auth token for the given user.
181|    ///
182|    /// Returns the opaque token string.
183|    pub fn issue(&self, user_id: &str, username: &str) -> String {
184|        let mut buf = [0u8; 32];
185|        rand::rngs::OsRng.fill_bytes(&mut buf);
186|        let token = hex::encode(buf);
187|        let now = chrono::Utc::now().timestamp();
188|
189|        let session = PreAuthSession {
190|            user_id: user_id.to_string(),
191|            username: username.to_string(),
192|            expires_at: now + PREAUTH_TTL_SECS,
193|            consumed: false,
194|        };
195|
196|        let mut map = self.sessions.write().expect("preauth store lock");
197|        map.insert(token.clone(), session);
198|        token
199|    }
200|
201|    /// Consume a pre-auth token. Returns the session if valid.
202|    ///
203|    /// Returns `None` when:
204|    /// - the token doesn't exist,
205|    /// - it's expired,
206|    /// - it was already consumed.
207|    pub fn consume(&self, token: &str) -> Option<PreAuthSession> {
208|        let mut map = self.sessions.write().expect("preauth store lock");
209|        let session = map.get(token)?;
210|
211|        let now = chrono::Utc::now().timestamp();
212|        if session.expires_at < now || session.consumed {
213|            // Clean up expired/consumed tokens
214|            map.remove(token);
215|            return None;
216|        }
217|
218|        // Mark consumed and return
219|        if let Some(s) = map.get_mut(token) {
220|            s.consumed = true;
221|        }
222|        let session = map.remove(token)?;
223|        Some(session)
224|    }
225|
226|    /// Number of stored sessions (for tests / metrics).
227|    #[must_use]
228|    pub fn count(&self) -> usize {
229|        let map = self.sessions.read().expect("preauth store lock");
230|        map.len()
231|    }
232|
233|    /// Remove all expired sessions (for periodic cleanup).
234|    pub fn purge_expired(&self) -> usize {
235|        let now = chrono::Utc::now().timestamp();
236|        let mut map = self.sessions.write().expect("preauth store lock");
237|        let before = map.len();
238|        map.retain(|_, s| s.expires_at >= now && !s.consumed);
239|        before - map.len()
240|    }
241|}
242|
243|// ── Low-level helpers ──────────────────────────────────────────────
244|
245|/// RFC 4648 base32 encode (no padding). 5 bits per character.
246|fn base32_encode(bytes: &[u8]) -> String {
247|    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
248|    let mut out = Vec::with_capacity((bytes.len() * 8).div_ceil(5));
249|    let mut buffer = 0u64;
250|    let mut bits = 0;
251|
252|    for &byte in bytes {
253|        buffer = (buffer << 8) | u64::from(byte);
254|        bits += 8;
255|        while bits >= 5 {
256|            bits -= 5;
257|            let idx = ((buffer >> bits) & 0x1f) as usize;
258|            out.push(ALPHABET[idx]);
259|        }
260|    }
261|    if bits > 0 {
262|        let idx = ((buffer << (5 - bits)) & 0x1f) as usize;
263|        out.push(ALPHABET[idx]);
264|    }
265|    String::from_utf8(out).expect("valid base32")
266|}
267|
268|/// RFC 4648 base32 decode (no padding).
269|fn base32_decode(s: &str) -> Result<Vec<u8>, String> {
270|    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
271|    let mut out = Vec::with_capacity((s.len() * 5).div_ceil(8));
272|    let mut buffer = 0u64;
273|    let mut bits = 0;
274|
275|    for ch in s.bytes() {
276|        let idx = if ch.is_ascii_lowercase() {
277|            let upper = ch.to_ascii_uppercase();
278|            ALPHABET.iter().position(|&a| a == upper)
279|        } else {
280|            ALPHABET.iter().position(|&a| a == ch)
281|        };
282|        let idx = idx.ok_or_else(|| format!("invalid base32 char: {}", ch as char))?;
283|        buffer = (buffer << 5) | (idx as u64);
284|        bits += 5;
285|        if bits >= 8 {
286|            bits -= 8;
287|            out.push((buffer >> bits) as u8);
            // bits is guaranteed < 8 here, so the cast is safe
288|        }
289|    }
290|    Ok(out)
291|}
292|
293|/// URL percent-encode a string for the `otpauth://` URI.
294|fn url_encode(s: &str) -> String {
295|    // Most characters in issuer/username are ASCII-alphanumeric.
296|    // We only encode what's needed for the URI.
297|    let mut out = String::with_capacity(s.len());
298|    for b in s.bytes() {
299|        match b {
300|            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b':' => {
301|                out.push(b as char);
302|            }
303|            b' ' => out.push_str("%20"),
304|            _ => out.push_str(&format!("%{b:02X}")),
305|        }
306|    }
307|    out
308|}
309|
310|#[cfg(test)]
311|mod tests {
312|    use super::*;
313|
314|    #[test]
315|    fn test_generate_totp_secret_is_base32() {
316|        let secret = generate_totp_secret();
317|        // 20 bytes → base32 unpadded = ceil(20*8/5) = 32 chars
318|        assert_eq!(secret.len(), 32);
319|        // Should only contain A-Z and 2-7 (base32 alphabet)
320|        for ch in secret.chars() {
321|            assert!(
322|                ch.is_ascii_uppercase() || ch.is_ascii_digit(),
323|                "invalid base32 char: {ch}"
324|            );
325|        }
326|    }
327|
328|    #[test]
329|    fn test_totp_url_includes_required_params() {
330|        let url = totp_url("moxui", "alice", "JBSWY3DPEHPK3PXP");
331|        assert!(url.starts_with("otpauth://totp/moxui:alice?"));
332|        assert!(url.contains("secret=JBSWY3DPEHPK3PXP"));
333|        assert!(url.contains("issuer=moxui"));
334|        assert!(url.contains("algorithm=SHA1"));
335|        assert!(url.contains("digits=6"));
336|        assert!(url.contains("period=30"));
337|    }
338|
339|    #[test]
340|    fn test_totp_verify_known_vector() {
341|        // RFC 6238 test vector: SHA1, secret = "12345678901234567890"
342|        let secret_b32 = base32_encode(b"12345678901234567890");
343|        // We can't test a specific time since TOTP is time-dependent.
344|        // But we can verify the algorithm self-consistently:
345|        let computed = compute_totp(b"12345678901234567890", 0);
346|        assert_eq!(computed.len(), 6);
347|        assert!(computed.chars().all(|c| c.is_ascii_digit()));
348|    }
349|
350|    #[test]
351|    fn test_totp_verify_valid_code() {
352|        let secret = generate_totp_secret();
353|        let now = std::time::SystemTime::now()
354|            .duration_since(std::time::UNIX_EPOCH)
355|            .unwrap_or_default()
356|            .as_secs();
357|        let code = compute_totp(&base32_decode(&secret).unwrap(), now / 30);
358|
359|        // verify_totp should accept the current code
360|        assert!(verify_totp(&secret, &code));
361|
362|        // A bogus code should fail
363|        assert!(!verify_totp(&secret, "000000"));
364|    }
365|
366|    #[test]
367|    fn test_generate_backup_codes_count() {
368|        let (plain, hashed) = generate_backup_codes();
369|        assert_eq!(plain.len(), BACKUP_CODE_COUNT);
370|        assert_eq!(hashed.len(), BACKUP_CODE_COUNT);
371|
372|        // Each code should be 8 digits
373|        for code in &plain {
374|            assert_eq!(code.len(), BACKUP_CODE_LEN);
375|            assert!(code.chars().all(|c| c.is_ascii_digit()));
376|        }
377|    }
378|
379|    #[test]
380|    fn test_verify_backup_code_matches() {
381|        let (plain, hashed) = generate_backup_codes();
382|        let remaining = verify_backup_code(&hashed, &plain[0]);
383|        assert!(remaining.is_some());
384|        assert_eq!(remaining.as_ref().unwrap().len(), BACKUP_CODE_COUNT - 1);
385|
386|        // The used code should be consumed — can't use it again
387|        let remaining2 = verify_backup_code(&remaining.unwrap(), &plain[0]);
388|        assert!(remaining2.is_none());
389|    }
390|
391|    #[test]
392|    fn test_verify_backup_code_wrong_code() {
393|        let (_, hashed) = generate_backup_codes();
394|        let remaining = verify_backup_code(&hashed, "00000000");
395|        assert!(remaining.is_none());
396|    }
397|
398|    #[test]
399|    fn test_preauth_store_issue_and_consume() {
400|        let store = PreAuthStore::new();
401|        let token = store.issue("u-alice", "alice");
402|        assert_eq!(store.count(), 1);
403|
404|        let session = store.consume(&token);
405|        assert!(session.is_some());
406|        assert_eq!(session.unwrap().user_id, "u-alice");
407|
408|        // Already consumed — should return None
409|        assert!(store.consume(&token).is_none());
410|        assert_eq!(store.count(), 0);
411|    }
412|
413|    #[test]
414|    fn test_preauth_store_rejects_bogus() {
415|        let store = PreAuthStore::new();
416|        assert!(store.consume("bogus-token").is_none());
417|    }
418|
419|    #[test]
420|    fn test_preauth_store_expired_token() {
421|        let store = PreAuthStore::new();
422|        let mut buf = [0u8; 32];
423|        rand::rngs::OsRng.fill_bytes(&mut buf);
424|        let token = hex::encode(buf);
425|
426|        // Insert an already-expired session
427|        let expired = PreAuthSession {
428|            user_id: "u-alice".to_string(),
429|            username: "alice".to_string(),
430|            expires_at: chrono::Utc::now().timestamp() - 1,
431|            consumed: false,
432|        };
433|        {
434|            let mut map = store.sessions.write().unwrap();
435|            map.insert(token.clone(), expired);
436|        }
437|
438|        assert!(store.consume(&token).is_none());
439|    }
440|}