use std::fs::File;
use std::sync::Mutex;

use aes::Aes128;
use cbc::{Decryptor, Encryptor};
use cipher::block_padding::NoPadding;
use cipher::{BlockModeDecrypt, BlockModeEncrypt, KeyIvInit, StreamCipher, StreamCipherSeek};
use ctr::Ctr128BE;
use emu_crypto::ecc;
use sha2::{Digest, Sha256};

use crate::format::{BinaryFormat, FormatError, Reader, Writer};
use crate::source::{Source, read_fully, read_whole_file};

pub const OTP_SIZE: usize = 0x100;
pub const OTP_MAGIC: u32 = 0xDEAD_B00F;
const OTP_BODY_SIZE: usize = 0xE0;
const FILE_ID_NCCH: u32 = 0;

const ISSUER_RETAIL: &str = "Nintendo CA - G3_NintendoCTR2prod";
const ISSUER_DEV: &str = "Nintendo CA - G3_NintendoCTR2dev";
const KEY_TYPE_ECC: u32 = 2;

const DEFAULT_KEYS: &[u8] = include_bytes!("../data/default_keys.enc");

pub type Key = [u8; 16];

pub fn cbc_decrypt(key: &Key, iv: &Key, data: &mut [u8]) -> Option<()> {
    let decryptor = Decryptor::<Aes128>::new_from_slices(key, iv).ok()?;
    decryptor.decrypt_padded::<NoPadding>(data).ok()?;
    Some(())
}

pub fn cbc_encrypt(key: &Key, iv: &Key, data: &mut [u8]) -> Option<()> {
    let length = data.len();
    let encryptor = Encryptor::<Aes128>::new_from_slices(key, iv).ok()?;
    encryptor
        .encrypt_padded::<NoPadding>(data, length)
        .ok()?;
    Some(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeysFile {
    pub otp_key: Key,
    pub otp_iv: Key,
    pub root_public_xy: [u8; ecc::PUBLIC_KEY_SIZE],
}

impl Default for KeysFile {
    fn default() -> Self {
        Self {
            otp_key: [0; 16],
            otp_iv: [0; 16],
            root_public_xy: [0; ecc::PUBLIC_KEY_SIZE],
        }
    }
}

fn hex_bytes(text: &str) -> Option<Vec<u8>> {
    (0..text.len() / 2)
        .map(|index| u8::from_str_radix(text.get(index * 2..index * 2 + 2)?, 16).ok())
        .collect()
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

impl KeysFile {
    pub fn built_in() -> Option<Self> {
        let mut data = DEFAULT_KEYS.to_vec();
        cbc_decrypt(&[0; 16], &[0; 16], &mut data)?;
        Self::decode(&data).ok()
    }

    pub fn load() -> Option<Self> {
        match read_whole_file(&format!("{}sysdata/keys.txt", emu_storage::user_path())) {
            Some(bytes) => Self::decode(&bytes).ok(),
            None => Self::built_in(),
        }
    }
}

impl BinaryFormat for KeysFile {
    fn encode(&self) -> Vec<u8> {
        format!(
            ":AES\notpKey={}\notpIV={}\n:ECC\nrootPublicXY={}\n",
            to_hex(&self.otp_key),
            to_hex(&self.otp_iv),
            to_hex(&self.root_public_xy)
        )
        .into_bytes()
    }

    fn decode(bytes: &[u8]) -> Result<Self, FormatError> {
        let text = String::from_utf8_lossy(bytes);
        let mut keys = Self::default();
        let mut mode = "";
        for line in text.split('\n') {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(name) = line.strip_prefix(':') {
                mode = name;
                continue;
            }
            let parts: Vec<&str> = line.split('=').collect();
            let [name, value] = parts[..] else {
                continue;
            };
            match (mode, name) {
                ("AES", "otpKey") | ("AES", "otpIV") => {
                    let Some(key) = value.get(..32).and_then(hex_bytes) else {
                        continue;
                    };
                    let key: Key = key.try_into().unwrap();
                    if name == "otpKey" {
                        keys.otp_key = key;
                    } else {
                        keys.otp_iv = key;
                    }
                }
                ("ECC", "rootPublicXY") => {
                    if let Some(key) = hex_bytes(value) {
                        let length = key.len().min(keys.root_public_xy.len());
                        keys.root_public_xy[..length].copy_from_slice(&key[..length]);
                    }
                }
                _ => {}
            }
        }
        Ok(keys)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OtpBin {
    pub device_id: u32,
    pub fallback_movable_key_y: Key,
    pub otp_version: u8,
    pub system_type: u8,
    pub manufacture_date: [u8; 6],
    pub ct_expiry: [u8; 4],
    pub ct_private_key: [u8; 0x20],
    pub ct_signature: [u8; 0x3C],
    pub random_key_seed: [u8; 0x50],
}

impl Default for OtpBin {
    fn default() -> Self {
        Self {
            device_id: 0,
            fallback_movable_key_y: [0; 16],
            otp_version: 0,
            system_type: 0,
            manufacture_date: [0; 6],
            ct_expiry: [0; 4],
            ct_private_key: [0; 0x20],
            ct_signature: [0; 0x3C],
            random_key_seed: [0; 0x50],
        }
    }
}

impl OtpBin {
    pub fn is_dev(&self) -> bool {
        self.system_type != 0
    }

    pub fn ct_expiration(&self) -> u32 {
        if self.otp_version < 5 {
            u32::from_be_bytes(self.ct_expiry)
        } else {
            u32::from_le_bytes(self.ct_expiry)
        }
    }

    pub fn encode_encrypted(&self, keys: &KeysFile) -> Vec<u8> {
        let mut bytes = self.encode();
        cbc_encrypt(&keys.otp_key, &keys.otp_iv, &mut bytes).expect("the OTP is a whole number of blocks");
        bytes
    }
}

impl BinaryFormat for OtpBin {
    fn encode(&self) -> Vec<u8> {
        let mut writer = Writer::new();
        writer
            .u32_le(OTP_MAGIC)
            .u32_le(self.device_id)
            .put(&self.fallback_movable_key_y)
            .u8(self.otp_version)
            .u8(self.system_type)
            .put(&self.manufacture_date)
            .put(&self.ct_expiry)
            .put(&self.ct_private_key)
            .put(&self.ct_signature)
            .zeros(0x10)
            .put(&self.random_key_seed);
        let mut bytes = writer.finish();
        let hash = Sha256::digest(&bytes);
        bytes.extend_from_slice(&hash);
        bytes
    }

    fn decode(bytes: &[u8]) -> Result<Self, FormatError> {
        if bytes.len() < OTP_SIZE {
            return Err(FormatError::Truncated);
        }
        let mut reader = Reader::new(bytes);
        if reader.u32_le()? != OTP_MAGIC {
            return Err(FormatError::Invalid("OTP"));
        }
        let otp = Self {
            device_id: reader.u32_le()?,
            fallback_movable_key_y: reader.array()?,
            otp_version: reader.u8()?,
            system_type: reader.u8()?,
            manufacture_date: reader.array()?,
            ct_expiry: reader.array()?,
            ct_private_key: reader.array()?,
            ct_signature: reader.array()?,
            random_key_seed: {
                reader.skip(0x10)?;
                reader.array()?
            },
        };
        let hash: [u8; 0x20] = reader.array()?;
        if Sha256::digest(&bytes[..OTP_BODY_SIZE])[..] != hash {
            return Err(FormatError::Invalid("OTP hash"));
        }
        Ok(otp)
    }
}

fn padded<const N: usize>(text: &str) -> [u8; N] {
    let mut bytes = [0; N];
    let source = text.as_bytes();
    bytes[..source.len().min(N)].copy_from_slice(&source[..source.len().min(N)]);
    bytes
}

pub fn certificate_body(otp: &OtpBin, public_key: &[u8; ecc::PUBLIC_KEY_SIZE]) -> Vec<u8> {
    let issuer = if otp.is_dev() { ISSUER_DEV } else { ISSUER_RETAIL };
    let name = format!("CT{:08X}-{:02X}", otp.device_id, otp.system_type);
    let mut writer = Writer::new();
    writer
        .put(&padded::<0x40>(issuer))
        .u32_be(KEY_TYPE_ECC)
        .put(&padded::<0x40>(&name))
        .u32_be(otp.ct_expiration())
        .put(public_key)
        .pad_to(0x100);
    writer.finish()
}

#[derive(Clone, Copy)]
struct CryptoParams {
    key: Key,
    iv: Key,
}

fn load_params() -> Option<CryptoParams> {
    let keys = KeysFile::load()?;
    let mut otp_bytes = read_whole_file(&format!("{}sysdata/otp.bin", emu_storage::user_path()))?;
    if otp_bytes.len() != OTP_SIZE {
        return None;
    }
    if u32::from_le_bytes(otp_bytes[..4].try_into().ok()?) != OTP_MAGIC {
        cbc_decrypt(&keys.otp_key, &keys.otp_iv, &mut otp_bytes)?;
    }
    let otp = OtpBin::decode(&otp_bytes).ok()?;
    let public_key = ecc::public_key(&otp.ct_private_key)?;
    let body = certificate_body(&otp, &public_key);
    if !ecc::verify(&body, &otp.ct_signature, &keys.root_public_xy) {
        return None;
    }
    Some(derive_params(&public_key, otp.device_id))
}

fn derive_params(public_key: &[u8; ecc::PUBLIC_KEY_SIZE], device_id: u32) -> CryptoParams {
    let mut hasher = Sha256::new();
    hasher.update(public_key);
    hasher.update(device_id.to_le_bytes());
    hasher.update(FILE_ID_NCCH.to_le_bytes());
    let digest = hasher.finalize();
    let mut key = [0u8; 16];
    key.copy_from_slice(&digest[..16]);
    let mut iv = [0u8; 16];
    iv[..12].copy_from_slice(&digest[16..28]);
    CryptoParams { key, iv }
}

static PARAMS: Mutex<Option<Option<CryptoParams>>> = Mutex::new(None);

pub fn reset() {
    *PARAMS.lock().unwrap_or_else(|error| error.into_inner()) = None;
}

fn params() -> Option<CryptoParams> {
    *PARAMS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .get_or_insert_with(load_params)
}

pub fn encrypt_file_contents(otp: &OtpBin, plain: &[u8]) -> Option<Vec<u8>> {
    let params = derive_params(&ecc::public_key(&otp.ct_private_key)?, otp.device_id);
    let mut cipher = Ctr128BE::<Aes128>::new_from_slices(&params.key, &params.iv).ok()?;
    let mut data = plain.to_vec();
    cipher.apply_keystream(&mut data);
    Some(data)
}

pub struct CryptoSource {
    file: File,
    size: u64,
    params: CryptoParams,
}

impl Source for CryptoSource {
    fn size(&self) -> u64 {
        self.size
    }

    fn read_at(&self, offset: u64, buffer: &mut [u8]) -> usize {
        let read = read_fully(&self.file, offset, buffer);
        let Ok(mut cipher) = Ctr128BE::<Aes128>::new_from_slices(&self.params.key, &self.params.iv)
        else {
            return 0;
        };
        cipher.seek(offset);
        cipher.apply_keystream(&mut buffer[..read]);
        read
    }

    fn is_crypto(&self) -> bool {
        true
    }
}

pub fn open_crypto_source(path: &str) -> Option<CryptoSource> {
    let params = params()?;
    let file = emu_storage::open_file(
        path,
        emu_storage::OpenMode {
            read: true,
            write: false,
            append: false,
            truncate: false,
        },
    )?;
    let size = file.metadata().ok()?.len();
    Some(CryptoSource { file, size, params })
}
