//! SNMPv3 message format (RFC 3412) and the User-based Security Model
//! (RFC 3414): key derivation and localisation, HMAC authentication with
//! MD5 / SHA-1 (RFC 3414) and SHA-2 (RFC 7860), DES-CBC (RFC 3414) and
//! AES-CFB-128 (RFC 3826) privacy. AES-192 / AES-256 use the key extension of
//! draft-blumenthal-aes-usm-04 (what net-snmp calls `AES192` / `AES256`).
//!
//! HMAC and the two block modes are written out here on top of the hash /
//! block cipher primitives (`md-5`, `sha1`, `sha2`, `des`, `aes`).

use std::fmt;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

use aes::cipher::{BlockCipherDecrypt, BlockCipherEncrypt, KeyInit};
use sha2::Digest;

use crate::ber::{self, tag, BerError, Reader};
use crate::pdu::AnyPdu;

/// `msgFlags` bits.
pub mod flags {
    /// authFlag.
    pub const AUTH: u8 = 0x01;
    /// privFlag.
    pub const PRIV: u8 = 0x02;
    /// reportableFlag.
    pub const REPORTABLE: u8 = 0x04;
}

/// USM security model number.
pub const SECURITY_MODEL_USM: i64 = 3;

/// Default `msgMaxSize` advertised.
pub const MAX_MESSAGE_SIZE: i32 = 65_507;

/// Minimum pass phrase length (RFC 3414 §11.2 recommends 8; agents enforce it).
pub const MIN_PASSWORD_LEN: usize = 8;

/// USM errors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UsmError {
    /// A pass phrase shorter than [`MIN_PASSWORD_LEN`].
    #[error("pass phrase must have at least {MIN_PASSWORD_LEN} characters")]
    PasswordTooShort,
    /// Privacy requested without authentication.
    #[error("privacy requires authentication (authPriv)")]
    PrivWithoutAuth,
    /// Unknown protocol name.
    #[error("unknown protocol `{0}`")]
    UnknownProtocol(String),
    /// The message's HMAC does not match.
    #[error("authentication failed (wrong digest)")]
    WrongDigest,
    /// The message is encrypted but cannot be decrypted.
    #[error("decryption failed: {0}")]
    Decrypt(String),
    /// The message names a user that is not configured.
    #[error("unknown user `{0}`")]
    UnknownUser(String),
    /// The message's security level does not match the user.
    #[error("unsupported security level for user `{0}`")]
    SecurityLevel(String),
    /// Malformed BER.
    #[error(transparent)]
    Ber(#[from] BerError),
}

/// Authentication protocols.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum AuthProtocol {
    /// HMAC-MD5-96.
    #[serde(rename = "MD5")]
    Md5,
    /// HMAC-SHA-96.
    #[serde(rename = "SHA", alias = "SHA1", alias = "SHA-1")]
    Sha1,
    /// HMAC-SHA-224 (usmHMAC128SHA224AuthProtocol).
    #[serde(rename = "SHA-224", alias = "SHA224")]
    Sha224,
    /// HMAC-SHA-256 (usmHMAC192SHA256AuthProtocol).
    #[serde(rename = "SHA-256", alias = "SHA256")]
    Sha256,
    /// HMAC-SHA-384 (usmHMAC256SHA384AuthProtocol).
    #[serde(rename = "SHA-384", alias = "SHA384")]
    Sha384,
    /// HMAC-SHA-512 (usmHMAC384SHA512AuthProtocol).
    #[serde(rename = "SHA-512", alias = "SHA512")]
    Sha512,
}

impl AuthProtocol {
    /// All protocols.
    pub const ALL: [AuthProtocol; 6] = [
        AuthProtocol::Md5,
        AuthProtocol::Sha1,
        AuthProtocol::Sha224,
        AuthProtocol::Sha256,
        AuthProtocol::Sha384,
        AuthProtocol::Sha512,
    ];

    /// Display name (`MD5`, `SHA`, `SHA-256`, ...).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            AuthProtocol::Md5 => "MD5",
            AuthProtocol::Sha1 => "SHA",
            AuthProtocol::Sha224 => "SHA-224",
            AuthProtocol::Sha256 => "SHA-256",
            AuthProtocol::Sha384 => "SHA-384",
            AuthProtocol::Sha512 => "SHA-512",
        }
    }

    /// Length of the transmitted MAC (`msgAuthenticationParameters`).
    #[must_use]
    pub const fn mac_len(self) -> usize {
        match self {
            AuthProtocol::Md5 | AuthProtocol::Sha1 => 12,
            AuthProtocol::Sha224 => 16,
            AuthProtocol::Sha256 => 24,
            AuthProtocol::Sha384 => 32,
            AuthProtocol::Sha512 => 48,
        }
    }

    const fn block_len(self) -> usize {
        match self {
            AuthProtocol::Sha384 | AuthProtocol::Sha512 => 128,
            _ => 64,
        }
    }

    /// The hash of `parts` concatenated.
    #[must_use]
    pub fn hash(self, parts: &[&[u8]]) -> Vec<u8> {
        fn h<D: Digest>(parts: &[&[u8]]) -> Vec<u8> {
            let mut d = D::new();
            for p in parts {
                d.update(p);
            }
            d.finalize().to_vec()
        }
        match self {
            AuthProtocol::Md5 => h::<md5::Md5>(parts),
            AuthProtocol::Sha1 => h::<sha1::Sha1>(parts),
            AuthProtocol::Sha224 => h::<sha2::Sha224>(parts),
            AuthProtocol::Sha256 => h::<sha2::Sha256>(parts),
            AuthProtocol::Sha384 => h::<sha2::Sha384>(parts),
            AuthProtocol::Sha512 => h::<sha2::Sha512>(parts),
        }
    }

    /// HMAC (RFC 2104) of `data` with `key`, full length.
    #[must_use]
    pub fn hmac(self, key: &[u8], data: &[u8]) -> Vec<u8> {
        let block = self.block_len();
        let mut k = if key.len() > block {
            self.hash(&[key])
        } else {
            key.to_vec()
        };
        k.resize(block, 0);
        let ipad: Vec<u8> = k.iter().map(|b| b ^ 0x36).collect();
        let opad: Vec<u8> = k.iter().map(|b| b ^ 0x5c).collect();
        let inner = self.hash(&[&ipad, data]);
        self.hash(&[&opad, &inner])
    }

    /// Password to key (RFC 3414 A.2): the hash of 1 MiB of the repeated pass
    /// phrase.
    ///
    /// # Errors
    /// [`UsmError::PasswordTooShort`].
    pub fn password_to_key(self, password: &[u8]) -> Result<Vec<u8>, UsmError> {
        fn h<D: Digest>(pw: &[u8]) -> Vec<u8> {
            let mut d = D::new();
            let mut buf = [0u8; 64];
            let mut idx = 0usize;
            for _ in 0..(1_048_576 / 64) {
                for b in &mut buf {
                    *b = pw[idx % pw.len()];
                    idx += 1;
                }
                d.update(buf);
            }
            d.finalize().to_vec()
        }
        if password.len() < MIN_PASSWORD_LEN {
            return Err(UsmError::PasswordTooShort);
        }
        Ok(match self {
            AuthProtocol::Md5 => h::<md5::Md5>(password),
            AuthProtocol::Sha1 => h::<sha1::Sha1>(password),
            AuthProtocol::Sha224 => h::<sha2::Sha224>(password),
            AuthProtocol::Sha256 => h::<sha2::Sha256>(password),
            AuthProtocol::Sha384 => h::<sha2::Sha384>(password),
            AuthProtocol::Sha512 => h::<sha2::Sha512>(password),
        })
    }

    /// Key localisation (RFC 3414 A.2): `H(Ku || engineID || Ku)`.
    #[must_use]
    pub fn localize(self, ku: &[u8], engine_id: &[u8]) -> Vec<u8> {
        self.hash(&[ku, engine_id, ku])
    }
}

impl fmt::Display for AuthProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for AuthProtocol {
    type Err = UsmError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(
            match s
                .trim()
                .to_ascii_uppercase()
                .replace(['-', '_'], "")
                .as_str()
            {
                "MD5" => AuthProtocol::Md5,
                "SHA" | "SHA1" => AuthProtocol::Sha1,
                "SHA224" => AuthProtocol::Sha224,
                "SHA256" => AuthProtocol::Sha256,
                "SHA384" => AuthProtocol::Sha384,
                "SHA512" => AuthProtocol::Sha512,
                _ => return Err(UsmError::UnknownProtocol(s.to_owned())),
            },
        )
    }
}

/// Privacy protocols.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum PrivProtocol {
    /// CBC-DES (RFC 3414).
    #[serde(rename = "DES")]
    Des,
    /// CFB128-AES-128 (RFC 3826).
    #[serde(rename = "AES", alias = "AES128", alias = "AES-128")]
    Aes128,
    /// CFB128-AES-192 (draft-blumenthal-aes-usm-04 key extension).
    #[serde(rename = "AES-192", alias = "AES192")]
    Aes192,
    /// CFB128-AES-256 (draft-blumenthal-aes-usm-04 key extension).
    #[serde(rename = "AES-256", alias = "AES256")]
    Aes256,
}

impl PrivProtocol {
    /// All protocols.
    pub const ALL: [PrivProtocol; 4] = [
        PrivProtocol::Des,
        PrivProtocol::Aes128,
        PrivProtocol::Aes192,
        PrivProtocol::Aes256,
    ];

    /// Display name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            PrivProtocol::Des => "DES",
            PrivProtocol::Aes128 => "AES",
            PrivProtocol::Aes192 => "AES-192",
            PrivProtocol::Aes256 => "AES-256",
        }
    }

    /// Key octets the cipher needs (DES: key + pre-IV).
    #[must_use]
    pub const fn key_len(self) -> usize {
        match self {
            PrivProtocol::Des | PrivProtocol::Aes128 => 16,
            PrivProtocol::Aes192 => 24,
            PrivProtocol::Aes256 => 32,
        }
    }

    /// The privacy key from a localised key: truncated, or extended with
    /// `H(key)` blocks when the hash is shorter (AES-192/256 with MD5 / SHA-1).
    #[must_use]
    pub fn derive_key(self, auth: AuthProtocol, localized: &[u8]) -> Vec<u8> {
        let need = self.key_len();
        let mut k = localized.to_vec();
        while k.len() < need {
            let h = auth.hash(&[&k]);
            k.extend_from_slice(&h);
        }
        k.truncate(need);
        k
    }

    /// Encrypts a serialised scoped PDU. Returns (ciphertext,
    /// `msgPrivacyParameters`).
    #[must_use]
    pub fn encrypt(
        self,
        key: &[u8],
        boots: u32,
        time: u32,
        salt: u64,
        plain: &[u8],
    ) -> (Vec<u8>, Vec<u8>) {
        match self {
            PrivProtocol::Des => {
                #[allow(clippy::cast_possible_truncation)] // low 32 bits of the counter
                let low = salt as u32;
                let mut s = Vec::with_capacity(8);
                s.extend_from_slice(&boots.to_be_bytes());
                s.extend_from_slice(&low.to_be_bytes());
                let iv: Vec<u8> = key[8..16].iter().zip(&s).map(|(a, b)| a ^ b).collect();
                (des_cbc_encrypt(&key[..8], &iv, plain), s)
            }
            _ => {
                let s = salt.to_be_bytes().to_vec();
                let iv = aes_iv(boots, time, &s);
                (aes_cfb(self, key, &iv, plain, true), s)
            }
        }
    }

    /// Decrypts `msgData`'s encryptedPDU.
    ///
    /// # Errors
    /// Bad privacy parameters or ciphertext length.
    pub fn decrypt(
        self,
        key: &[u8],
        boots: u32,
        time: u32,
        priv_params: &[u8],
        cipher: &[u8],
    ) -> Result<Vec<u8>, UsmError> {
        if priv_params.len() != 8 {
            return Err(UsmError::Decrypt(format!(
                "privacy parameters of {} octets",
                priv_params.len()
            )));
        }
        match self {
            PrivProtocol::Des => {
                if !cipher.len().is_multiple_of(8) {
                    return Err(UsmError::Decrypt(format!(
                        "DES ciphertext of {} octets",
                        cipher.len()
                    )));
                }
                let iv: Vec<u8> = key[8..16]
                    .iter()
                    .zip(priv_params)
                    .map(|(a, b)| a ^ b)
                    .collect();
                Ok(des_cbc_decrypt(&key[..8], &iv, cipher))
            }
            _ => {
                let iv = aes_iv(boots, time, priv_params);
                Ok(aes_cfb(self, key, &iv, cipher, false))
            }
        }
    }
}

impl fmt::Display for PrivProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for PrivProtocol {
    type Err = UsmError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(
            match s
                .trim()
                .to_ascii_uppercase()
                .replace(['-', '_'], "")
                .as_str()
            {
                "DES" => PrivProtocol::Des,
                "AES" | "AES128" => PrivProtocol::Aes128,
                "AES192" => PrivProtocol::Aes192,
                "AES256" => PrivProtocol::Aes256,
                _ => return Err(UsmError::UnknownProtocol(s.to_owned())),
            },
        )
    }
}

fn aes_iv(boots: u32, time: u32, salt: &[u8]) -> [u8; 16] {
    let mut iv = [0u8; 16];
    iv[..4].copy_from_slice(&boots.to_be_bytes());
    iv[4..8].copy_from_slice(&time.to_be_bytes());
    iv[8..].copy_from_slice(&salt[..8]);
    iv
}

fn des_cbc_encrypt(key: &[u8], iv: &[u8], plain: &[u8]) -> Vec<u8> {
    let c = des::Des::new_from_slice(key).expect("8-octet DES key");
    let mut prev = [0u8; 8];
    prev.copy_from_slice(iv);
    let mut data = plain.to_vec();
    // RFC 3414 §8.1.1.2: pad to a multiple of 8 (the value does not matter).
    data.resize(plain.len().div_ceil(8) * 8, 0);
    let mut out = Vec::with_capacity(data.len());
    for chunk in data.chunks(8) {
        let mut b = des::cipher::Block::<des::Des>::default();
        for i in 0..8 {
            b[i] = chunk[i] ^ prev[i];
        }
        c.encrypt_block(&mut b);
        prev.copy_from_slice(&b);
        out.extend_from_slice(&b);
    }
    out
}

fn des_cbc_decrypt(key: &[u8], iv: &[u8], cipher: &[u8]) -> Vec<u8> {
    let c = des::Des::new_from_slice(key).expect("8-octet DES key");
    let mut prev = [0u8; 8];
    prev.copy_from_slice(iv);
    let mut out = Vec::with_capacity(cipher.len());
    for chunk in cipher.chunks(8) {
        let mut b = des::cipher::Block::<des::Des>::default();
        b.copy_from_slice(chunk);
        c.decrypt_block(&mut b);
        for i in 0..8 {
            out.push(b[i] ^ prev[i]);
        }
        prev.copy_from_slice(chunk);
    }
    out
}

fn aes_cfb(p: PrivProtocol, key: &[u8], iv: &[u8; 16], data: &[u8], encrypt: bool) -> Vec<u8> {
    fn run<C: BlockCipherEncrypt + KeyInit>(
        key: &[u8],
        iv: &[u8; 16],
        data: &[u8],
        encrypt: bool,
    ) -> Vec<u8> {
        let c = C::new_from_slice(key).expect("AES key length");
        let mut reg = *iv;
        let mut out = Vec::with_capacity(data.len());
        for chunk in data.chunks(16) {
            let mut ks = aes::Block::default();
            ks.copy_from_slice(&reg);
            // AES-128/192/256 all have 16-octet blocks.
            let mut blk = aes::cipher::Array::try_from(&ks[..]).expect("16-octet block");
            c.encrypt_block(&mut blk);
            let start = out.len();
            for (i, x) in chunk.iter().enumerate() {
                out.push(x ^ blk[i]);
            }
            if chunk.len() == 16 {
                let ct = if encrypt { &out[start..] } else { chunk };
                reg.copy_from_slice(ct);
            }
        }
        out
    }
    match p {
        PrivProtocol::Aes192 => run::<aes::Aes192>(key, iv, data, encrypt),
        PrivProtocol::Aes256 => run::<aes::Aes256>(key, iv, data, encrypt),
        _ => run::<aes::Aes128>(key, iv, data, encrypt),
    }
}

/// Monotonic salt counter for the privacy IVs (random start).
fn next_salt() -> u64 {
    static SALT: OnceLock<AtomicU64> = OnceLock::new();
    SALT.get_or_init(|| {
        let mut b = [0u8; 8];
        let _ = getrandom::fill(&mut b);
        AtomicU64::new(u64::from_be_bytes(b))
    })
    .fetch_add(1, Ordering::Relaxed)
}

/// A USM user with pass phrases (never persisted by wpg).
#[derive(Clone, PartialEq, Eq, Default)]
pub struct UsmUser {
    /// `msgUserName`.
    pub name: String,
    /// Authentication protocol and pass phrase.
    pub auth: Option<(AuthProtocol, String)>,
    /// Privacy protocol and pass phrase (requires `auth`).
    pub privacy: Option<(PrivProtocol, String)>,
}

impl fmt::Debug for UsmUser {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UsmUser")
            .field("name", &self.name)
            .field("auth", &self.auth.as_ref().map(|a| a.0))
            .field("privacy", &self.privacy.as_ref().map(|p| p.0))
            .finish()
    }
}

/// Security level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SecurityLevel {
    /// noAuthNoPriv.
    NoAuthNoPriv,
    /// authNoPriv.
    AuthNoPriv,
    /// authPriv.
    AuthPriv,
}

impl SecurityLevel {
    /// The `msgFlags` bits.
    #[must_use]
    pub const fn flags(self) -> u8 {
        match self {
            SecurityLevel::NoAuthNoPriv => 0,
            SecurityLevel::AuthNoPriv => flags::AUTH,
            SecurityLevel::AuthPriv => flags::AUTH | flags::PRIV,
        }
    }
}

impl UsmUser {
    /// The level the user's configuration implies.
    #[must_use]
    pub fn level(&self) -> SecurityLevel {
        match (&self.auth, &self.privacy) {
            (None, _) => SecurityLevel::NoAuthNoPriv,
            (Some(_), None) => SecurityLevel::AuthNoPriv,
            (Some(_), Some(_)) => SecurityLevel::AuthPriv,
        }
    }

    /// Checks pass phrase lengths and the auth / priv combination.
    ///
    /// # Errors
    /// [`UsmError::PasswordTooShort`], [`UsmError::PrivWithoutAuth`].
    pub fn validate(&self) -> Result<(), UsmError> {
        if self.privacy.is_some() && self.auth.is_none() {
            return Err(UsmError::PrivWithoutAuth);
        }
        for pw in self
            .auth
            .iter()
            .map(|a| &a.1)
            .chain(self.privacy.iter().map(|p| &p.1))
        {
            if pw.len() < MIN_PASSWORD_LEN {
                return Err(UsmError::PasswordTooShort);
            }
        }
        Ok(())
    }

    /// Derives the master keys (`Ku`, expensive: 1 MiB of hashing per key).
    ///
    /// # Errors
    /// As [`UsmUser::validate`].
    pub fn master_keys(&self) -> Result<MasterKeys, UsmError> {
        self.validate()?;
        let auth = match &self.auth {
            Some((p, pw)) => Some((*p, p.password_to_key(pw.as_bytes())?)),
            None => None,
        };
        let privacy = match (&self.privacy, &auth) {
            (Some((pp, pw)), Some((ap, _))) => Some((*pp, ap.password_to_key(pw.as_bytes())?)),
            _ => None,
        };
        Ok(MasterKeys {
            name: self.name.clone(),
            auth,
            privacy,
        })
    }
}

/// `Ku` of a user (engine independent).
#[derive(Clone)]
pub struct MasterKeys {
    name: String,
    auth: Option<(AuthProtocol, Vec<u8>)>,
    privacy: Option<(PrivProtocol, Vec<u8>)>,
}

impl fmt::Debug for MasterKeys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MasterKeys")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl MasterKeys {
    /// The user name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The level.
    #[must_use]
    pub fn level(&self) -> SecurityLevel {
        match (&self.auth, &self.privacy) {
            (None, _) => SecurityLevel::NoAuthNoPriv,
            (Some(_), None) => SecurityLevel::AuthNoPriv,
            (Some(_), Some(_)) => SecurityLevel::AuthPriv,
        }
    }

    /// Localises the keys for `engine_id`.
    #[must_use]
    pub fn localize(&self, engine_id: &[u8]) -> LocalKeys {
        let auth = self
            .auth
            .as_ref()
            .map(|(p, ku)| (*p, p.localize(ku, engine_id)));
        let privacy = match (&self.privacy, &self.auth) {
            (Some((pp, ku)), Some((ap, _))) => {
                let kul = ap.localize(ku, engine_id);
                Some((*pp, pp.derive_key(*ap, &kul)))
            }
            _ => None,
        };
        LocalKeys {
            user: self.name.as_bytes().to_vec(),
            engine_id: engine_id.to_vec(),
            auth,
            privacy,
        }
    }
}

/// Keys localised to one engine.
#[derive(Clone, Default)]
pub struct LocalKeys {
    /// `msgUserName`.
    pub user: Vec<u8>,
    /// The engine the keys belong to.
    pub engine_id: Vec<u8>,
    /// Authentication key.
    pub auth: Option<(AuthProtocol, Vec<u8>)>,
    /// Privacy key.
    pub privacy: Option<(PrivProtocol, Vec<u8>)>,
}

impl fmt::Debug for LocalKeys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalKeys")
            .field("user", &String::from_utf8_lossy(&self.user))
            .finish_non_exhaustive()
    }
}

impl LocalKeys {
    /// Keys of the empty user for engine discovery (noAuthNoPriv).
    #[must_use]
    pub fn discovery() -> Self {
        Self::default()
    }

    /// The level.
    #[must_use]
    pub fn level(&self) -> SecurityLevel {
        match (&self.auth, &self.privacy) {
            (None, _) => SecurityLevel::NoAuthNoPriv,
            (Some(_), None) => SecurityLevel::AuthNoPriv,
            (Some(_), Some(_)) => SecurityLevel::AuthPriv,
        }
    }
}

/// `msgSecurityParameters` (`UsmSecurityParameters`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UsmParams {
    /// `msgAuthoritativeEngineID`.
    pub engine_id: Vec<u8>,
    /// `msgAuthoritativeEngineBoots`.
    pub boots: u32,
    /// `msgAuthoritativeEngineTime`.
    pub time: u32,
    /// `msgUserName`.
    pub user: Vec<u8>,
    /// `msgAuthenticationParameters`.
    pub auth_params: Vec<u8>,
    /// `msgPrivacyParameters`.
    pub priv_params: Vec<u8>,
}

impl UsmParams {
    /// Encoding, and the offset of the authentication parameters' content in it.
    fn encode(&self) -> (Vec<u8>, usize) {
        let e = ber::octets(&self.engine_id);
        let b = ber::integer(i64::from(self.boots));
        let t = ber::integer(i64::from(self.time));
        let u = ber::octets(&self.user);
        let a = ber::octets(&self.auth_params);
        let p = ber::octets(&self.priv_params);
        let content_len = e.len() + b.len() + t.len() + u.len() + a.len() + p.len();
        let off = ber::header_len(content_len)
            + e.len()
            + b.len()
            + t.len()
            + u.len()
            + ber::header_len(self.auth_params.len());
        (ber::seq(tag::SEQUENCE, &[&e, &b, &t, &u, &a, &p]), off)
    }
}

/// `ScopedPDU`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedPdu {
    /// `contextEngineID`.
    pub context_engine_id: Vec<u8>,
    /// `contextName`.
    pub context_name: Vec<u8>,
    /// The PDU.
    pub pdu: AnyPdu,
}

impl ScopedPdu {
    /// BER encoding.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        ber::seq(
            tag::SEQUENCE,
            &[
                &ber::octets(&self.context_engine_id),
                &ber::octets(&self.context_name),
                &self.pdu.encode(),
            ],
        )
    }

    /// Decodes a scoped PDU.
    ///
    /// # Errors
    /// Malformed BER.
    pub fn decode(data: &[u8]) -> Result<ScopedPdu, BerError> {
        let mut r = Reader::new(data);
        let s = r.expect(tag::SEQUENCE)?;
        let mut sr = Reader::inner(&s);
        let context_engine_id = sr.octets()?.to_vec();
        let context_name = sr.octets()?.to_vec();
        let pdu = AnyPdu::decode(&mut sr)?;
        Ok(ScopedPdu {
            context_engine_id,
            context_name,
            pdu,
        })
    }
}

/// `msgData`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MsgData {
    /// Plaintext scoped PDU.
    Plain(ScopedPdu),
    /// `encryptedPDU`.
    Encrypted(Vec<u8>),
}

/// A decoded SNMPv3 message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct V3Message {
    /// `msgID`.
    pub msg_id: i32,
    /// `msgMaxSize`.
    pub max_size: i32,
    /// `msgFlags`.
    pub flags: u8,
    /// `msgSecurityModel`.
    pub security_model: i64,
    /// USM parameters.
    pub usm: UsmParams,
    /// `msgData`.
    pub data: MsgData,
    /// Offset of the authentication parameters' content in the input (decoded
    /// messages only).
    pub auth_offset: usize,
}

/// Inputs of [`encode_v3`].
#[derive(Debug, Clone)]
pub struct V3Out<'a> {
    /// `msgID`.
    pub msg_id: i32,
    /// Set the reportable flag (confirmed-class PDUs).
    pub reportable: bool,
    /// Keys (their level selects the flags).
    pub keys: &'a LocalKeys,
    /// Authoritative engine id.
    pub engine_id: &'a [u8],
    /// Authoritative engine boots.
    pub boots: u32,
    /// Authoritative engine time.
    pub time: u32,
    /// The scoped PDU.
    pub scoped: &'a ScopedPdu,
}

/// Builds an SNMPv3 message: encrypts and authenticates as the keys require.
#[must_use]
pub fn encode_v3(o: &V3Out<'_>) -> Vec<u8> {
    let level = o.keys.level();
    let mut fl = level.flags();
    if o.reportable {
        fl |= flags::REPORTABLE;
    }
    let plain = o.scoped.encode();
    let (data, priv_params) = match &o.keys.privacy {
        Some((p, key)) => {
            let (c, s) = p.encrypt(key, o.boots, o.time, next_salt(), &plain);
            (ber::octets(&c), s)
        }
        None => (plain, Vec::new()),
    };
    let mac_len = o.keys.auth.as_ref().map_or(0, |(p, _)| p.mac_len());
    let usm = UsmParams {
        engine_id: o.engine_id.to_vec(),
        boots: o.boots,
        time: o.time,
        user: o.keys.user.clone(),
        auth_params: vec![0; mac_len],
        priv_params,
    };
    let (usm_enc, usm_auth_off) = usm.encode();
    let version = ber::integer(3);
    let global = ber::seq(
        tag::SEQUENCE,
        &[
            &ber::integer(i64::from(o.msg_id)),
            &ber::integer(i64::from(MAX_MESSAGE_SIZE)),
            &ber::octets(&[fl]),
            &ber::integer(SECURITY_MODEL_USM),
        ],
    );
    let sec = ber::octets(&usm_enc);
    let content_len = version.len() + global.len() + sec.len() + data.len();
    let auth_off = ber::header_len(content_len)
        + version.len()
        + global.len()
        + ber::header_len(usm_enc.len())
        + usm_auth_off;
    let mut msg = ber::seq(tag::SEQUENCE, &[&version, &global, &sec, &data]);
    if let Some((p, key)) = &o.keys.auth {
        let mac = p.hmac(key, &msg);
        msg[auth_off..auth_off + mac_len].copy_from_slice(&mac[..mac_len]);
    }
    msg
}

/// Decodes an SNMPv3 message (no authentication / decryption yet).
///
/// # Errors
/// Malformed BER, not version 3.
pub fn decode_v3(data: &[u8]) -> Result<V3Message, BerError> {
    let mut r = Reader::new(data);
    let outer = r.expect(tag::SEQUENCE)?;
    let mut mr = Reader::inner(&outer);
    if mr.integer()? != 3 {
        return Err(BerError::Invalid("not an SNMPv3 message".into()));
    }
    let g = mr.expect(tag::SEQUENCE)?;
    let mut gr = Reader::inner(&g);
    let msg_id = gr.int32()?;
    let max_size = gr.int32()?;
    let fl = gr.octets()?;
    let flags = fl.first().copied().unwrap_or(0);
    let security_model = gr.integer()?;
    let sp = mr.expect(tag::OCTET_STRING)?;
    let mut spr = Reader::new(sp.content);
    let us = spr.expect(tag::SEQUENCE)?;
    let mut ur = Reader::inner(&us);
    let engine_id = ur.octets()?.to_vec();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // 0..2^31-1
    let boots = ur.integer()?.max(0) as u32;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let time = ur.integer()?.max(0) as u32;
    let user = ur.octets()?.to_vec();
    let a = ur.expect(tag::OCTET_STRING)?;
    let auth_params = a.content.to_vec();
    let auth_offset = sp.content_offset + a.content_offset;
    let priv_params = ur.octets()?.to_vec();
    let data = if flags & flags::PRIV != 0 {
        MsgData::Encrypted(mr.octets()?.to_vec())
    } else {
        let s = mr.read()?;
        let whole = &data[s.offset..s.content_offset + s.content.len()];
        MsgData::Plain(ScopedPdu::decode(whole)?)
    };
    Ok(V3Message {
        msg_id,
        max_size,
        flags,
        security_model,
        usm: UsmParams {
            engine_id,
            boots,
            time,
            user,
            auth_params,
            priv_params,
        },
        data,
        auth_offset,
    })
}

/// Checks the HMAC of `raw` (the message bytes `m` was decoded from) and
/// decrypts the scoped PDU with `keys`.
///
/// # Errors
/// [`UsmError::SecurityLevel`] when the flags do not fit the keys,
/// [`UsmError::WrongDigest`], [`UsmError::Decrypt`], malformed data.
pub fn open_v3(m: &V3Message, raw: &[u8], keys: &LocalKeys) -> Result<ScopedPdu, UsmError> {
    let want_auth = m.flags & flags::AUTH != 0;
    let want_priv = m.flags & flags::PRIV != 0;
    if want_auth {
        let Some((p, key)) = &keys.auth else {
            return Err(UsmError::SecurityLevel(
                String::from_utf8_lossy(&m.usm.user).into_owned(),
            ));
        };
        let n = p.mac_len();
        if m.usm.auth_params.len() != n || m.auth_offset + n > raw.len() {
            return Err(UsmError::WrongDigest);
        }
        let mut copy = raw.to_vec();
        copy[m.auth_offset..m.auth_offset + n].fill(0);
        let mac = p.hmac(key, &copy);
        if !constant_eq(&mac[..n], &m.usm.auth_params) {
            return Err(UsmError::WrongDigest);
        }
    }
    match &m.data {
        MsgData::Plain(s) => Ok(s.clone()),
        MsgData::Encrypted(c) => {
            let (Some((p, key)), true) = (&keys.privacy, want_priv) else {
                return Err(UsmError::SecurityLevel(
                    String::from_utf8_lossy(&m.usm.user).into_owned(),
                ));
            };
            let plain = p.decrypt(key, m.usm.boots, m.usm.time, &m.usm.priv_params, c)?;
            // DES padding follows the SEQUENCE: decode just the first TLV.
            let mut r = Reader::new(&plain);
            let t = r.read().map_err(|e| UsmError::Decrypt(e.to_string()))?;
            ScopedPdu::decode(&plain[..t.content_offset + t.content.len()])
                .map_err(|e| UsmError::Decrypt(e.to_string()))
        }
    }
}

fn constant_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Hex without separators (engine ids).
#[must_use]
pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Parses an engine id given as hex (`0x8000...` or plain).
#[must_use]
pub fn parse_engine_id(s: &str) -> Option<Vec<u8>> {
    let t = s.trim();
    let t = t
        .strip_prefix("0x")
        .or_else(|| t.strip_prefix("0X"))
        .unwrap_or(t);
    let t: String = t
        .chars()
        .filter(|c| !matches!(c, ':' | ' ' | '-'))
        .collect();
    if t.is_empty() || !t.len().is_multiple_of(2) || t.len() > 64 {
        return None;
    }
    (0..t.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&t[i..i + 2], 16).ok())
        .collect()
}

/// A fresh local engine id in the RFC 3411 text format (`80 00 00 00 04`
/// followed by `wpg-` and 12 random hex digits). wpg has no enterprise number;
/// the id only needs to be unique among the engines a receiver talks to.
#[must_use]
pub fn random_engine_id() -> Vec<u8> {
    let mut r = [0u8; 6];
    let _ = getrandom::fill(&mut r);
    let mut v = vec![0x80, 0x00, 0x00, 0x00, 0x04];
    v.extend_from_slice(b"wpg-");
    v.extend_from_slice(hex(&r).as_bytes());
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oid::Oid;
    use crate::pdu::{Pdu, PduType};
    use crate::value::VarBind;

    fn unhex(s: &str) -> Vec<u8> {
        parse_engine_id(&s.replace(' ', "")).unwrap()
    }

    /// RFC 3414 A.3.1 / A.3.2.
    #[test]
    fn rfc3414_key_localization() {
        let engine = unhex("000000000000000000000002");
        let ku = AuthProtocol::Md5.password_to_key(b"maplesyrup").unwrap();
        assert_eq!(hex(&ku), "9faf3283884e92834ebc9847d8edd963");
        assert_eq!(
            hex(&AuthProtocol::Md5.localize(&ku, &engine)),
            "526f5eed9fcce26f8964c2930787d82b"
        );
        let ku = AuthProtocol::Sha1.password_to_key(b"maplesyrup").unwrap();
        assert_eq!(hex(&ku), "9fb5cc0381497b3793528939ff788d5d79145211");
        assert_eq!(
            hex(&AuthProtocol::Sha1.localize(&ku, &engine)),
            "6695febc9288e36282235fc7151f128497b38f3f"
        );
    }

    /// SHA-2 keys (RFC 7860 uses the RFC 3414 algorithm with SHA-2); expected
    /// values computed independently with Python's hashlib.
    #[test]
    fn sha2_key_localization() {
        let engine = unhex("000000000000000000000002");
        for (p, expected) in [
            (
                AuthProtocol::Sha224,
                "0bd8827c6e29f8065e08e09237f177e410f69b90e1782be682075674",
            ),
            (
                AuthProtocol::Sha256,
                "8982e0e549e866db361a6b625d84cccc11162d453ee8ce3a6445c2d6776f0f8b",
            ),
            (
                AuthProtocol::Sha384,
                "3b298f16164a11184279d5432bf169e2d2a48307de02b3d3f7e2b4f36eb6f0455a53689a3937eea07319a633d2ccba78",
            ),
            (
                AuthProtocol::Sha512,
                "22a5a36cedfcc085807a128d7bc6c2382167ad6c0dbc5fdff856740f3d84c099ad1ea87a8db096714d9788bd544047c9021e4229ce27e4c0a69250adfcffbb0b",
            ),
        ] {
            let ku = p.password_to_key(b"maplesyrup").unwrap();
            assert_eq!(hex(&p.localize(&ku, &engine)), expected, "{p}");
        }
    }

    /// RFC 2202 / RFC 4231 HMAC test case 2 ("Jefe").
    #[test]
    fn hmac_vectors() {
        let d = b"what do ya want for nothing?";
        assert_eq!(
            hex(&AuthProtocol::Md5.hmac(b"Jefe", d)),
            "750c783e6ab0b503eaa86e310a5db738"
        );
        assert_eq!(
            hex(&AuthProtocol::Sha1.hmac(b"Jefe", d)),
            "effcdf6ae5eb2fa2d27416d5f184df9c259a7c79"
        );
        assert_eq!(
            hex(&AuthProtocol::Sha256.hmac(b"Jefe", d)),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
        assert_eq!(
            hex(&AuthProtocol::Sha512.hmac(b"Jefe", d)),
            "164b7a7bfcf819e2e395fbe73b56e0a387bd64222e831fd610270cd7ea2505549758bf75c05a994a6d034f65f8f0e6fdcaeab1a34d4a6b4b636e070a38bce737"
        );
    }

    #[test]
    fn short_password_rejected() {
        assert_eq!(
            AuthProtocol::Md5.password_to_key(b"short"),
            Err(UsmError::PasswordTooShort)
        );
        let u = UsmUser {
            name: "u".into(),
            auth: None,
            privacy: Some((PrivProtocol::Des, "longenough".into())),
        };
        assert_eq!(u.validate(), Err(UsmError::PrivWithoutAuth));
    }

    /// FIPS 197 C.1 (AES-128) via one CFB block with a zero IV: the first
    /// keystream block is E(K, 0).
    #[test]
    fn aes_known_answer() {
        let key = unhex("000102030405060708090a0b0c0d0e0f");
        // E(K, 00112233...) from FIPS 197: use CFB with IV = plaintext and a
        // zero data block, so the output equals the cipher block.
        let iv: [u8; 16] = unhex("00112233445566778899aabbccddeeff")
            .try_into()
            .unwrap();
        let out = aes_cfb(PrivProtocol::Aes128, &key, &iv, &[0u8; 16], true);
        assert_eq!(hex(&out), "69c4e0d86a7b0430d8cdb78070b4c55a");
    }

    /// FIPS 81 / the classic DES test: key 0123456789abcdef, "Now is t".
    #[test]
    fn des_known_answer() {
        let out = des_cbc_encrypt(&unhex("0123456789abcdef"), &[0u8; 8], b"Now is t");
        assert_eq!(hex(&out), "3fa40e8a984d4815");
        assert_eq!(
            des_cbc_decrypt(&unhex("0123456789abcdef"), &[0u8; 8], &out),
            b"Now is t"
        );
    }

    fn sample_scoped() -> ScopedPdu {
        ScopedPdu {
            context_engine_id: unhex("8000000001020304"),
            context_name: Vec::new(),
            pdu: AnyPdu::Pdu(Pdu::new(
                PduType::Get,
                42,
                vec![VarBind::null("1.3.6.1.2.1.1.1.0".parse::<Oid>().unwrap())],
            )),
        }
    }

    #[test]
    fn v3_round_trip_all_levels() {
        let engine = unhex("8000000001020304");
        let mut users = vec![UsmUser {
            name: "noauth".into(),
            ..UsmUser::default()
        }];
        for a in AuthProtocol::ALL {
            users.push(UsmUser {
                name: format!("auth-{a}"),
                auth: Some((a, "authpass123".into())),
                privacy: None,
            });
            for p in PrivProtocol::ALL {
                users.push(UsmUser {
                    name: format!("priv-{a}-{p}"),
                    auth: Some((a, "authpass123".into())),
                    privacy: Some((p, "privpass456".into())),
                });
            }
        }
        let scoped = sample_scoped();
        for u in users {
            let keys = u.master_keys().unwrap().localize(&engine);
            let raw = encode_v3(&V3Out {
                msg_id: 7,
                reportable: true,
                keys: &keys,
                engine_id: &engine,
                boots: 3,
                time: 12345,
                scoped: &scoped,
            });
            let m = decode_v3(&raw).unwrap();
            assert_eq!(m.msg_id, 7);
            assert_eq!(m.usm.user, u.name.as_bytes());
            assert_eq!(m.flags & 3, u.level().flags(), "{}", u.name);
            assert_eq!(open_v3(&m, &raw, &keys).unwrap(), scoped, "{}", u.name);
            if keys.auth.is_some() {
                // A flipped bit anywhere breaks the digest.
                let mut bad = raw.clone();
                let last = bad.len() - 1;
                bad[last] ^= 1;
                let m = decode_v3(&bad).unwrap_or_else(|_| m.clone());
                assert!(open_v3(&m, &bad, &keys).is_err(), "{}", u.name);
                // Wrong password.
                let mut other = u.clone();
                other.auth.as_mut().unwrap().1 = "different-pass".into();
                let k2 = other.master_keys().unwrap().localize(&engine);
                let m = decode_v3(&raw).unwrap();
                assert_eq!(open_v3(&m, &raw, &k2), Err(UsmError::WrongDigest));
            }
        }
    }

    #[test]
    fn privacy_key_extension() {
        let ku = AuthProtocol::Md5.password_to_key(b"maplesyrup").unwrap();
        let kul = AuthProtocol::Md5.localize(&ku, &unhex("000000000000000000000002"));
        let k = PrivProtocol::Aes256.derive_key(AuthProtocol::Md5, &kul);
        assert_eq!(k.len(), 32);
        assert_eq!(&k[..16], &kul[..]);
        assert_eq!(&k[16..], &AuthProtocol::Md5.hash(&[&kul])[..]);
        assert_eq!(
            PrivProtocol::Aes128
                .derive_key(AuthProtocol::Sha1, &[1; 20])
                .len(),
            16
        );
        assert_eq!(
            "aes-256".parse::<PrivProtocol>().unwrap(),
            PrivProtocol::Aes256
        );
        assert_eq!("sha".parse::<AuthProtocol>().unwrap(), AuthProtocol::Sha1);
        assert_eq!(
            "SHA-512".parse::<AuthProtocol>().unwrap(),
            AuthProtocol::Sha512
        );
    }
}
