//! The standard security handler: RC4 and AES (128 and 256 bit) with the
//! empty user password. Datasheets are often "protected" this way: anyone
//! can open them, only copying and printing are flagged.

use aes::cipher::array::Array;
use aes::cipher::consts::U16;
use aes::cipher::{BlockCipherDecrypt, BlockCipherEncrypt, KeyInit};
use aes::{Aes128, Aes256};
use md5::{Digest as _, Md5};
use sha2::{Sha256, Sha384, Sha512};

use super::object::{Dict, Object};

/// The padding string of the standard security handler.
const PAD: [u8; 32] = [
    0x28, 0xBF, 0x4E, 0x5E, 0x4E, 0x75, 0x8A, 0x41, 0x64, 0x00, 0x4E, 0x56, 0xFF, 0xFA, 0x01, 0x08,
    0x2E, 0x2E, 0x00, 0xB6, 0xD0, 0x68, 0x3E, 0x80, 0x2F, 0x0C, 0xA9, 0xFE, 0x64, 0x53, 0x69, 0x7A,
];

/// How strings or streams are encrypted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// Not encrypted (the `Identity` crypt filter).
    None,
    /// RC4 with a key per object.
    Rc4,
    /// AES-128 with a key per object.
    Aes128,
    /// AES-256 with the file key.
    Aes256,
}

/// Why an encrypted file cannot be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CryptError {
    /// Another security handler (certificates, a plug-in).
    #[error("unsupported security handler")]
    Handler,
    /// A password is needed to open the file.
    #[error("the file needs a password")]
    Password,
}

/// Decrypts the strings and streams of an encrypted file.
#[derive(Debug, Clone)]
pub struct Decryptor {
    key: Vec<u8>,
    strings: Method,
    streams: Method,
}

fn get<'a>(dict: &'a Dict, key: &str) -> Option<&'a Object> {
    dict.get(key.as_bytes())
}

fn bytes(dict: &Dict, key: &str) -> Vec<u8> {
    get(dict, key)
        .and_then(Object::string)
        .map(<[u8]>::to_vec)
        .unwrap_or_default()
}

fn padded(password: &[u8]) -> [u8; 32] {
    let mut out = PAD;
    let n = password.len().min(32);
    out[..n].copy_from_slice(&password[..n]);
    out[n..].copy_from_slice(&PAD[..32 - n]);
    out
}

/// RC4 (encryption and decryption are the same).
#[must_use]
pub fn rc4(key: &[u8], data: &[u8]) -> Vec<u8> {
    if key.is_empty() {
        return data.to_vec();
    }
    let mut s: [u8; 256] = std::array::from_fn(|i| i as u8);
    let mut j = 0u8;
    for i in 0..256 {
        j = j.wrapping_add(s[i]).wrapping_add(key[i % key.len()]);
        s.swap(i, usize::from(j));
    }
    let (mut i, mut j) = (0u8, 0u8);
    data.iter()
        .map(|&b| {
            i = i.wrapping_add(1);
            j = j.wrapping_add(s[usize::from(i)]);
            s.swap(usize::from(i), usize::from(j));
            let k = s[usize::from(s[usize::from(i)].wrapping_add(s[usize::from(j)]))];
            b ^ k
        })
        .collect()
}

fn block(data: &[u8]) -> Array<u8, U16> {
    let mut out = Array::<u8, U16>::default();
    out.copy_from_slice(&data[..16]);
    out
}

/// AES-CBC decryption without padding removal; `data` is cut to whole
/// blocks.
fn cbc_decrypt(key: &[u8], iv: &[u8], data: &[u8]) -> Vec<u8> {
    fn run<C: BlockCipherDecrypt<BlockSize = U16>>(cipher: &C, iv: &[u8], data: &[u8]) -> Vec<u8> {
        let mut previous = block(iv);
        let mut out = Vec::with_capacity(data.len());
        for chunk in data.chunks_exact(16) {
            let mut b = block(chunk);
            cipher.decrypt_block(&mut b);
            for (x, p) in b.iter_mut().zip(previous.iter()) {
                *x ^= p;
            }
            out.extend_from_slice(&b);
            previous = block(chunk);
        }
        out
    }
    match key.len() {
        16 => Aes128::new_from_slice(key).map_or_else(|_| Vec::new(), |c| run(&c, iv, data)),
        32 => Aes256::new_from_slice(key).map_or_else(|_| Vec::new(), |c| run(&c, iv, data)),
        _ => Vec::new(),
    }
}

/// AES-CBC encryption of whole blocks (no padding added).
fn cbc_encrypt(key: &[u8], iv: &[u8], data: &[u8]) -> Vec<u8> {
    fn run<C: BlockCipherEncrypt<BlockSize = U16>>(cipher: &C, iv: &[u8], data: &[u8]) -> Vec<u8> {
        let mut previous = block(iv);
        let mut out = Vec::with_capacity(data.len());
        for chunk in data.chunks_exact(16) {
            let mut b = block(chunk);
            for (x, p) in b.iter_mut().zip(previous.iter()) {
                *x ^= p;
            }
            cipher.encrypt_block(&mut b);
            out.extend_from_slice(&b);
            previous = b;
        }
        out
    }
    match key.len() {
        16 => Aes128::new_from_slice(key).map_or_else(|_| Vec::new(), |c| run(&c, iv, data)),
        32 => Aes256::new_from_slice(key).map_or_else(|_| Vec::new(), |c| run(&c, iv, data)),
        _ => Vec::new(),
    }
}

/// AES-CBC decryption of a PDF string or stream: the first block is the
/// initialisation vector, PKCS#7 padding is removed when present.
#[must_use]
pub fn aes_decrypt(key: &[u8], data: &[u8]) -> Vec<u8> {
    if data.len() < 32 {
        return Vec::new();
    }
    let (iv, rest) = data.split_at(16);
    let mut out = cbc_decrypt(key, iv, rest);
    if let Some(&n) = out.last() {
        let n = usize::from(n);
        if (1..=16).contains(&n) && n <= out.len() {
            out.truncate(out.len() - n);
        }
    }
    out
}

/// AES-CBC encryption with PKCS#7 padding and `iv` in front (for writing
/// test files).
#[must_use]
pub fn aes_encrypt(key: &[u8], iv: &[u8; 16], data: &[u8]) -> Vec<u8> {
    let mut plain = data.to_vec();
    let n = 16 - plain.len() % 16;
    plain.extend(std::iter::repeat_n(n as u8, n));
    let mut out = iv.to_vec();
    out.extend(cbc_encrypt(key, iv, &plain));
    out
}

/// The file key of revisions 2 to 4 (algorithm 2).
fn file_key(password: &[u8], dict: &Dict, id0: &[u8], revision: i64, length: usize) -> Vec<u8> {
    let mut md = Md5::new();
    md.update(padded(password));
    let owner = bytes(dict, "O");
    md.update(&owner[..owner.len().min(32)]);
    let p = get(dict, "P").and_then(Object::int).unwrap_or(-1) as i32;
    md.update(p.to_le_bytes());
    md.update(id0);
    let metadata = !matches!(get(dict, "EncryptMetadata"), Some(Object::Bool(false)));
    if revision >= 4 && !metadata {
        md.update([0xff; 4]);
    }
    let mut hash = md.finalize().to_vec();
    let n = if revision == 2 { 5 } else { length };
    if revision >= 3 {
        for _ in 0..50 {
            hash = Md5::digest(&hash[..n]).to_vec();
        }
    }
    hash.truncate(n);
    hash
}

/// The `U` check of revisions 2 to 4 (algorithms 4 and 5).
fn user_check(key: &[u8], id0: &[u8], revision: i64) -> Vec<u8> {
    if revision == 2 {
        return rc4(key, &PAD);
    }
    let mut md = Md5::new();
    md.update(PAD);
    md.update(id0);
    let mut x = rc4(key, &md.finalize());
    for i in 1..=19u8 {
        let k: Vec<u8> = key.iter().map(|b| b ^ i).collect();
        x = rc4(&k, &x);
    }
    x
}

/// The RC4 key made from the owner password (algorithm 3, steps a–d).
fn owner_key(owner: &[u8], revision: i64, length: usize) -> Vec<u8> {
    let mut hash = Md5::digest(padded(owner)).to_vec();
    let n = if revision == 2 { 5 } else { length };
    if revision >= 3 {
        for _ in 0..50 {
            hash = Md5::digest(&hash[..n]).to_vec();
        }
    }
    hash.truncate(n);
    hash
}

/// The hash of revision 6 (algorithm 2.B); revision 5 is plain SHA-256.
fn hash_v5(password: &[u8], salt: &[u8], udata: &[u8], revision: i64) -> Vec<u8> {
    let mut first = Sha256::new();
    first.update(password);
    first.update(salt);
    first.update(udata);
    let mut k = first.finalize().to_vec();
    if revision < 6 {
        return k;
    }
    let mut round = 0usize;
    loop {
        let mut k1 = Vec::with_capacity(64 * (password.len() + k.len() + udata.len()));
        for _ in 0..64 {
            k1.extend_from_slice(password);
            k1.extend_from_slice(&k);
            k1.extend_from_slice(udata);
        }
        let e = cbc_encrypt(&k[..16], &k[16..32], &k1);
        let sum: u32 = e[..16].iter().map(|&b| u32::from(b)).sum();
        k = match sum % 3 {
            0 => Sha256::digest(&e).to_vec(),
            1 => Sha384::digest(&e).to_vec(),
            _ => Sha512::digest(&e).to_vec(),
        };
        round += 1;
        let last = usize::from(*e.last().unwrap_or(&0));
        if round >= 64 && last + 32 <= round {
            break;
        }
    }
    k.truncate(32);
    k
}

fn method_of(dict: &Dict, filter: &str, default: Method) -> Method {
    let Some(name) = get(dict, filter).and_then(Object::name) else {
        return Method::None;
    };
    if name == b"Identity" {
        return Method::None;
    }
    let cfm = get(dict, "CF")
        .and_then(Object::dict)
        .and_then(|cf| cf.get(name))
        .and_then(Object::dict)
        .and_then(|d| d.get(b"CFM".as_slice()))
        .and_then(Object::name);
    match cfm {
        Some(b"V2") => Method::Rc4,
        Some(b"AESV2") => Method::Aes128,
        Some(b"AESV3") => Method::Aes256,
        Some(b"None") => Method::None,
        _ => default,
    }
}

impl Decryptor {
    /// Opens the security handler of `dict` (the `/Encrypt` dictionary)
    /// with the empty user password, or else the empty owner password.
    ///
    /// # Errors
    ///
    /// Another handler than the standard one, or a file that needs a
    /// password.
    pub fn new(dict: &Dict, id0: &[u8]) -> Result<Self, CryptError> {
        if get(dict, "Filter").and_then(Object::name) != Some(b"Standard") {
            return Err(CryptError::Handler);
        }
        let version = get(dict, "V").and_then(Object::int).unwrap_or(0);
        let revision = get(dict, "R").and_then(Object::int).unwrap_or(2);
        if version >= 5 {
            return Self::new_v5(dict, revision);
        }
        let mut bits = get(dict, "Length").and_then(Object::int).unwrap_or(40);
        if bits <= 16 {
            // Some writers give bytes.
            bits *= 8;
        }
        let length = (bits / 8).clamp(5, 16) as usize;
        let (strings, streams) = if version == 4 {
            (
                method_of(dict, "StrF", Method::Rc4),
                method_of(dict, "StmF", Method::Rc4),
            )
        } else {
            (Method::Rc4, Method::Rc4)
        };
        let length = if version == 4 && (strings == Method::Aes128 || streams == Method::Aes128) {
            16
        } else {
            length
        };
        let user = bytes(dict, "U");
        let compare = if revision == 2 { 32 } else { 16 };
        let matches = |key: &[u8]| {
            let check = user_check(key, id0, revision);
            user.len() >= compare && check.len() >= compare && check[..compare] == user[..compare]
        };
        let key = file_key(b"", dict, id0, revision, length);
        if matches(&key) {
            return Ok(Self {
                key,
                strings,
                streams,
            });
        }
        // The empty owner password gives the user password.
        let rc4_key = owner_key(b"", revision, length);
        let mut user_password = bytes(dict, "O");
        if revision == 2 {
            user_password = rc4(&rc4_key, &user_password);
        } else {
            for i in (0..=19u8).rev() {
                let k: Vec<u8> = rc4_key.iter().map(|b| b ^ i).collect();
                user_password = rc4(&k, &user_password);
            }
        }
        let key = file_key(&user_password, dict, id0, revision, length);
        if matches(&key) {
            return Ok(Self {
                key,
                strings,
                streams,
            });
        }
        Err(CryptError::Password)
    }

    fn new_v5(dict: &Dict, revision: i64) -> Result<Self, CryptError> {
        let user = bytes(dict, "U");
        let owner = bytes(dict, "O");
        let strings = method_of(dict, "StrF", Method::Aes256);
        let streams = method_of(dict, "StmF", Method::Aes256);
        if user.len() >= 48 && hash_v5(b"", &user[32..40], b"", revision) == user[..32] {
            let inter = hash_v5(b"", &user[40..48], b"", revision);
            let key = cbc_decrypt(&inter, &[0; 16], &bytes(dict, "UE"));
            return Ok(Self {
                key,
                strings,
                streams,
            });
        }
        if owner.len() >= 48
            && user.len() >= 48
            && hash_v5(b"", &owner[32..40], &user[..48], revision) == owner[..32]
        {
            let inter = hash_v5(b"", &owner[40..48], &user[..48], revision);
            let key = cbc_decrypt(&inter, &[0; 16], &bytes(dict, "OE"));
            return Ok(Self {
                key,
                strings,
                streams,
            });
        }
        Err(CryptError::Password)
    }

    fn object_key(&self, number: u32, generation: u16, aes: bool) -> Vec<u8> {
        let mut md = Md5::new();
        md.update(&self.key);
        md.update(&number.to_le_bytes()[..3]);
        md.update(generation.to_le_bytes());
        if aes {
            md.update(b"sAlT");
        }
        let mut out = md.finalize().to_vec();
        out.truncate((self.key.len() + 5).min(16));
        out
    }

    /// Decrypts a string (`stream` false) or stream data of an object.
    #[must_use]
    pub fn decrypt(&self, number: u32, generation: u16, data: &[u8], stream: bool) -> Vec<u8> {
        match if stream { self.streams } else { self.strings } {
            Method::None => data.to_vec(),
            Method::Rc4 => rc4(&self.object_key(number, generation, false), data),
            Method::Aes128 => aes_decrypt(&self.object_key(number, generation, true), data),
            Method::Aes256 => aes_decrypt(&self.key, data),
        }
    }
}

/// What a writer of a 128-bit RC4 file (revision 3) with an empty user
/// password needs: the `O` and `U` entries and the file key.
#[must_use]
pub fn standard_r3(owner: &[u8], id0: &[u8], permissions: i32) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let revision = 3;
    let length = 16;
    let rc4_key = owner_key(owner, revision, length);
    let mut o = rc4(&rc4_key, &padded(b""));
    for i in 1..=19u8 {
        let k: Vec<u8> = rc4_key.iter().map(|b| b ^ i).collect();
        o = rc4(&k, &o);
    }
    let mut dict = Dict::new();
    dict.insert(b"O".to_vec(), Object::String(o.clone()));
    dict.insert(b"P".to_vec(), Object::Int(i64::from(permissions)));
    let key = file_key(b"", &dict, id0, revision, length);
    let mut u = user_check(&key, id0, revision);
    u.extend_from_slice(&[0; 16]);
    (o, u, key)
}

/// The key of one object for a writer (RC4 or, with `aes`, AES-128).
#[must_use]
pub fn object_key(file_key: &[u8], number: u32, generation: u16, aes: bool) -> Vec<u8> {
    Decryptor {
        key: file_key.to_vec(),
        strings: Method::None,
        streams: Method::None,
    }
    .object_key(number, generation, aes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rc4_matches_the_known_vector() {
        // "Key" / "Plaintext" from the RC4 test vectors.
        assert_eq!(
            rc4(b"Key", b"Plaintext"),
            vec![0xBB, 0xF3, 0x16, 0xE8, 0xD9, 0x40, 0xAF, 0x0A, 0xD3]
        );
    }

    #[test]
    fn aes_round_trips() {
        let key = [7u8; 16];
        let iv = [3u8; 16];
        let data = b"BT /F1 12 Tf (Pin 1) Tj ET";
        let encrypted = aes_encrypt(&key, &iv, data);
        assert_eq!(encrypted.len(), 16 + 32);
        assert_eq!(aes_decrypt(&key, &encrypted), data);
    }

    #[test]
    fn opens_a_revision_3_file_with_the_empty_user_password() {
        let id0 = b"0123456789abcdef";
        let (o, u, key) = standard_r3(b"secret", id0, -44);
        let mut dict = Dict::new();
        dict.insert(b"Filter".to_vec(), Object::Name(b"Standard".to_vec()));
        dict.insert(b"V".to_vec(), Object::Int(2));
        dict.insert(b"R".to_vec(), Object::Int(3));
        dict.insert(b"Length".to_vec(), Object::Int(128));
        dict.insert(b"O".to_vec(), Object::String(o));
        dict.insert(b"U".to_vec(), Object::String(u));
        dict.insert(b"P".to_vec(), Object::Int(-44));
        let decryptor = Decryptor::new(&dict, id0).unwrap_or_else(|e| panic!("{e}"));
        let secret = rc4(&object_key(&key, 5, 0, false), b"Pin table");
        assert_eq!(decryptor.decrypt(5, 0, &secret, true), b"Pin table");
    }

    #[test]
    fn refuses_other_handlers() {
        let mut dict = Dict::new();
        dict.insert(b"Filter".to_vec(), Object::Name(b"Adobe.PubSec".to_vec()));
        assert!(matches!(
            Decryptor::new(&dict, b""),
            Err(CryptError::Handler)
        ));
    }
}
