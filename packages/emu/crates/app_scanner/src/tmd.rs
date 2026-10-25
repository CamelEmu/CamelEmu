use crate::format::{BinaryFormat, FormatError, Reader, Writer};

const BODY_SIZE: usize = 0x9C4;
const CHUNK_SIZE: usize = 0x30;
const SIGNATURE_RSA_4096_SHA1: u32 = 0x10000;
const SIGNATURE_RSA_2048_SHA1: u32 = 0x10001;
const SIGNATURE_ELLIPTIC_SHA1: u32 = 0x10002;
const SIGNATURE_RSA_4096_SHA256: u32 = 0x10003;
const SIGNATURE_RSA_2048_SHA256: u32 = 0x10004;
const SIGNATURE_ECDSA_SHA256: u32 = 0x10005;

pub const CONTENT_OPTIONAL: u16 = 1 << 14;

pub fn signature_size(signature_type: u32) -> Option<usize> {
    match signature_type {
        SIGNATURE_RSA_4096_SHA1 | SIGNATURE_RSA_4096_SHA256 => Some(0x200),
        SIGNATURE_RSA_2048_SHA1 | SIGNATURE_RSA_2048_SHA256 => Some(0x100),
        SIGNATURE_ELLIPTIC_SHA1 | SIGNATURE_ECDSA_SHA256 => Some(0x3C),
        _ => None,
    }
}

fn align_up(value: usize, alignment: usize) -> usize {
    value.div_ceil(alignment) * alignment
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ContentChunk {
    pub id: u32,
    pub index: u16,
    pub kind: u16,
    pub size: u64,
    pub hash: [u8; 0x20],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tmd {
    pub signature_type: u32,
    pub title_id: u64,
    pub title_type: u32,
    pub title_version: u16,
    pub chunks: Vec<ContentChunk>,
}

impl Default for Tmd {
    fn default() -> Self {
        Self {
            signature_type: SIGNATURE_RSA_2048_SHA256,
            title_id: 0,
            title_type: 0,
            title_version: 0,
            chunks: Vec::new(),
        }
    }
}

impl Tmd {
    pub fn content_id(&self, index: usize) -> Option<u32> {
        self.chunks.get(index).map(|chunk| chunk.id)
    }

    pub fn content_type(&self, index: usize) -> Option<u16> {
        self.chunks.get(index).map(|chunk| chunk.kind)
    }
}

impl BinaryFormat for Tmd {
    fn encode(&self) -> Vec<u8> {
        let signature_size = signature_size(self.signature_type).unwrap_or(0);
        let body_start = align_up(signature_size + 4, 0x40);
        let mut writer = Writer::new();
        writer
            .u32_be(self.signature_type)
            .zeros(signature_size)
            .pad_to(body_start);
        writer
            .zeros(0x40)
            .zeros(4)
            .u64_be(0)
            .u64_be(self.title_id)
            .u32_be(self.title_type)
            .u16_be(0)
            .u32_be(0)
            .u32_be(0)
            .zeros(4)
            .u8(0)
            .zeros(0x31)
            .u32_be(0)
            .u16_be(self.title_version)
            .u16_be(self.chunks.len() as u16)
            .u16_be(0)
            .zeros(2)
            .zeros(0x20)
            .pad_to(body_start + BODY_SIZE);
        for chunk in &self.chunks {
            writer
                .u32_be(chunk.id)
                .u16_be(chunk.index)
                .u16_be(chunk.kind)
                .u64_be(chunk.size)
                .put(&chunk.hash);
        }
        writer.finish()
    }

    fn decode(bytes: &[u8]) -> Result<Self, FormatError> {
        let mut reader = Reader::new(bytes);
        let signature_type = reader.u32_be()?;
        let signature_size =
            signature_size(signature_type).ok_or(FormatError::Invalid("TMD signature type"))?;
        let body_start = align_up(signature_size + 4, 0x40);
        if bytes.len() < body_start + BODY_SIZE {
            return Err(FormatError::Truncated);
        }
        reader.skip(body_start - 4 + 0x40 + 4 + 8)?;
        let title_id = reader.u64_be()?;
        let title_type = reader.u32_be()?;
        reader.skip(2 + 4 + 4 + 4 + 1 + 0x31 + 4)?;
        let title_version = reader.u16_be()?;
        let content_count = reader.u16_be()? as usize;
        reader.skip(BODY_SIZE - (0x40 + 4 + 8 + 8 + 4 + 2 + 4 + 4 + 4 + 1 + 0x31 + 4 + 2 + 2))?;
        if bytes.len() < body_start + BODY_SIZE + content_count * CHUNK_SIZE {
            return Err(FormatError::Invalid("TMD content chunk table"));
        }
        let mut chunks = Vec::with_capacity(content_count);
        for _ in 0..content_count {
            chunks.push(ContentChunk {
                id: reader.u32_be()?,
                index: reader.u16_be()?,
                kind: reader.u16_be()?,
                size: reader.u64_be()?,
                hash: reader.array()?,
            });
        }
        Ok(Self {
            signature_type,
            title_id,
            title_type,
            title_version,
            chunks,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmd(signature_type: u32) -> Tmd {
        Tmd {
            signature_type,
            title_id: 0x0004000000123400,
            title_type: 0x40,
            title_version: 0x0102,
            chunks: vec![
                ContentChunk { id: 0x1234, index: 0, kind: 0, size: 0x4000, hash: [7; 0x20] },
                ContentChunk { id: 0x5678, index: 1, kind: CONTENT_OPTIONAL, size: 9, hash: [0; 0x20] },
            ],
        }
    }

    #[test]
    fn body_starts_after_the_signature_aligned_to_0x40() {
        let bytes = tmd(SIGNATURE_RSA_2048_SHA256).encode();
        assert_eq!(&bytes[..4], &[0, 1, 0, 4]);
        assert_eq!(bytes.len(), 0x140 + BODY_SIZE + 2 * CHUNK_SIZE);
        assert_eq!(&bytes[0x140 + 0x4C..0x140 + 0x54], &0x0004000000123400u64.to_be_bytes());
    }

    #[test]
    fn round_trip_for_every_signature_type() {
        for signature_type in [
            SIGNATURE_RSA_4096_SHA1,
            SIGNATURE_RSA_2048_SHA1,
            SIGNATURE_ELLIPTIC_SHA1,
            SIGNATURE_RSA_4096_SHA256,
            SIGNATURE_RSA_2048_SHA256,
            SIGNATURE_ECDSA_SHA256,
        ] {
            let tmd = tmd(signature_type);
            assert_eq!(Tmd::decode(&tmd.encode()), Ok(tmd));
        }
    }

    #[test]
    fn rejects_an_unknown_signature_type_and_short_data() {
        let mut bytes = tmd(SIGNATURE_RSA_2048_SHA256).encode();
        assert_eq!(Tmd::decode(&bytes[..0x100]), Err(FormatError::Truncated));
        bytes[3] = 0x99;
        assert!(Tmd::decode(&bytes).is_err());
    }
}
