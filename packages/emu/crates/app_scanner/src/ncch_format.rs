use crate::format::{BinaryFormat, FormatError, Reader, Writer};

pub const BLOCK_SIZE: u64 = 0x200;
pub const NCSD_HEADER_SIZE: usize = 0x200;
pub const NCCH_HEADER_SIZE: usize = 0x200;
pub const EXHEADER_SIZE: usize = 0x800;
pub const EXEFS_HEADER_SIZE: usize = 0x200;
pub const EXEFS_SECTIONS: usize = 8;

const NCSD_MAGIC: [u8; 4] = *b"NCSD";
const NCCH_MAGIC: [u8; 4] = *b"NCCH";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NcsdPartition {
    pub offset: u32,
    pub size: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NcsdHeader {
    pub signature: [u8; 0x100],
    pub media_size: u32,
    pub media_id: [u8; 8],
    pub partition_fs_type: [u8; 8],
    pub partition_crypt_type: [u8; 8],
    pub partitions: [NcsdPartition; 8],
    pub extended_header_hash: [u8; 0x20],
    pub additional_header_size: u32,
    pub sector_zero_offset: u32,
    pub partition_flags: [u8; 8],
    pub partition_id_table: [u8; 0x40],
}

impl Default for NcsdHeader {
    fn default() -> Self {
        Self {
            signature: [0; 0x100],
            media_size: 0,
            media_id: [0; 8],
            partition_fs_type: [0; 8],
            partition_crypt_type: [0; 8],
            partitions: [NcsdPartition::default(); 8],
            extended_header_hash: [0; 0x20],
            additional_header_size: 0,
            sector_zero_offset: 0,
            partition_flags: [0; 8],
            partition_id_table: [0; 0x40],
        }
    }
}

impl BinaryFormat for NcsdHeader {
    fn encode(&self) -> Vec<u8> {
        let mut writer = Writer::new();
        writer
            .put(&self.signature)
            .put(&NCSD_MAGIC)
            .u32_le(self.media_size)
            .put(&self.media_id)
            .put(&self.partition_fs_type)
            .put(&self.partition_crypt_type);
        for partition in &self.partitions {
            writer.u32_le(partition.offset).u32_le(partition.size);
        }
        writer
            .put(&self.extended_header_hash)
            .u32_le(self.additional_header_size)
            .u32_le(self.sector_zero_offset)
            .put(&self.partition_flags)
            .put(&self.partition_id_table)
            .zeros(0x30);
        writer.finish()
    }

    fn decode(bytes: &[u8]) -> Result<Self, FormatError> {
        let mut reader = Reader::new(bytes);
        let signature = reader.array()?;
        if reader.array::<4>()? != NCSD_MAGIC {
            return Err(FormatError::Invalid("NCSD header"));
        }
        let media_size = reader.u32_le()?;
        let media_id = reader.array()?;
        let partition_fs_type = reader.array()?;
        let partition_crypt_type = reader.array()?;
        let mut partitions = [NcsdPartition::default(); 8];
        for partition in &mut partitions {
            partition.offset = reader.u32_le()?;
            partition.size = reader.u32_le()?;
        }
        Ok(Self {
            signature,
            media_size,
            media_id,
            partition_fs_type,
            partition_crypt_type,
            partitions,
            extended_header_hash: reader.array()?,
            additional_header_size: reader.u32_le()?,
            sector_zero_offset: reader.u32_le()?,
            partition_flags: reader.array()?,
            partition_id_table: reader.array()?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NcchHeader {
    pub signature: [u8; 0x100],
    pub content_size: u32,
    pub partition_id: [u8; 8],
    pub maker_code: u16,
    pub version: u16,
    pub program_id: u64,
    pub logo_region_hash: [u8; 0x20],
    pub product_code: [u8; 0x10],
    pub extended_header_hash: [u8; 0x20],
    pub extended_header_size: u32,
    pub reserved_flags: [u8; 3],
    pub secondary_key_slot: u8,
    pub platform: u8,
    pub is_data: bool,
    pub is_executable: bool,
    pub content_type: u8,
    pub content_unit_size: u8,
    pub fixed_key: bool,
    pub no_romfs: bool,
    pub no_crypto: bool,
    pub seed_crypto: bool,
    pub plain_region_offset: u32,
    pub plain_region_size: u32,
    pub logo_region_offset: u32,
    pub logo_region_size: u32,
    pub exefs_offset: u32,
    pub exefs_size: u32,
    pub exefs_hash_region_size: u32,
    pub romfs_offset: u32,
    pub romfs_size: u32,
    pub romfs_hash_region_size: u32,
    pub exefs_super_block_hash: [u8; 0x20],
    pub romfs_super_block_hash: [u8; 0x20],
}

impl Default for NcchHeader {
    fn default() -> Self {
        Self {
            signature: [0; 0x100],
            content_size: 0,
            partition_id: [0; 8],
            maker_code: 0,
            version: 0,
            program_id: 0,
            logo_region_hash: [0; 0x20],
            product_code: [0; 0x10],
            extended_header_hash: [0; 0x20],
            extended_header_size: 0,
            reserved_flags: [0; 3],
            secondary_key_slot: 0,
            platform: 0,
            is_data: false,
            is_executable: true,
            content_type: 0,
            content_unit_size: 0,
            fixed_key: false,
            no_romfs: false,
            no_crypto: true,
            seed_crypto: false,
            plain_region_offset: 0,
            plain_region_size: 0,
            logo_region_offset: 0,
            logo_region_size: 0,
            exefs_offset: 0,
            exefs_size: 0,
            exefs_hash_region_size: 0,
            romfs_offset: 0,
            romfs_size: 0,
            romfs_hash_region_size: 0,
            exefs_super_block_hash: [0; 0x20],
            romfs_super_block_hash: [0; 0x20],
        }
    }
}

impl BinaryFormat for NcchHeader {
    fn encode(&self) -> Vec<u8> {
        let type_flags = u8::from(self.is_data)
            | u8::from(self.is_executable) << 1
            | (self.content_type & 0x7) << 2;
        let crypto_flags = u8::from(self.fixed_key)
            | u8::from(self.no_romfs) << 1
            | u8::from(self.no_crypto) << 2
            | u8::from(self.seed_crypto) << 5;
        let mut writer = Writer::new();
        writer
            .put(&self.signature)
            .put(&NCCH_MAGIC)
            .u32_le(self.content_size)
            .put(&self.partition_id)
            .u16_le(self.maker_code)
            .u16_le(self.version)
            .zeros(4)
            .u64_le(self.program_id)
            .zeros(0x10)
            .put(&self.logo_region_hash)
            .put(&self.product_code)
            .put(&self.extended_header_hash)
            .u32_le(self.extended_header_size)
            .zeros(4)
            .put(&self.reserved_flags)
            .u8(self.secondary_key_slot)
            .u8(self.platform)
            .u8(type_flags)
            .u8(self.content_unit_size)
            .u8(crypto_flags)
            .u32_le(self.plain_region_offset)
            .u32_le(self.plain_region_size)
            .u32_le(self.logo_region_offset)
            .u32_le(self.logo_region_size)
            .u32_le(self.exefs_offset)
            .u32_le(self.exefs_size)
            .u32_le(self.exefs_hash_region_size)
            .zeros(4)
            .u32_le(self.romfs_offset)
            .u32_le(self.romfs_size)
            .u32_le(self.romfs_hash_region_size)
            .zeros(4)
            .put(&self.exefs_super_block_hash)
            .put(&self.romfs_super_block_hash);
        writer.finish()
    }

    fn decode(bytes: &[u8]) -> Result<Self, FormatError> {
        let mut reader = Reader::new(bytes);
        let signature = reader.array()?;
        if reader.array::<4>()? != NCCH_MAGIC {
            return Err(FormatError::Invalid("NCCH header"));
        }
        let content_size = reader.u32_le()?;
        let partition_id = reader.array()?;
        let maker_code = reader.u16_le()?;
        let version = reader.u16_le()?;
        reader.skip(4)?;
        let program_id = reader.u64_le()?;
        reader.skip(0x10)?;
        let logo_region_hash = reader.array()?;
        let product_code = reader.array()?;
        let extended_header_hash = reader.array()?;
        let extended_header_size = reader.u32_le()?;
        reader.skip(4)?;
        let reserved_flags = reader.array()?;
        let secondary_key_slot = reader.u8()?;
        let platform = reader.u8()?;
        let type_flags = reader.u8()?;
        let content_unit_size = reader.u8()?;
        let crypto_flags = reader.u8()?;
        let plain_region_offset = reader.u32_le()?;
        let plain_region_size = reader.u32_le()?;
        let logo_region_offset = reader.u32_le()?;
        let logo_region_size = reader.u32_le()?;
        let exefs_offset = reader.u32_le()?;
        let exefs_size = reader.u32_le()?;
        let exefs_hash_region_size = reader.u32_le()?;
        reader.skip(4)?;
        let romfs_offset = reader.u32_le()?;
        let romfs_size = reader.u32_le()?;
        let romfs_hash_region_size = reader.u32_le()?;
        reader.skip(4)?;
        Ok(Self {
            signature,
            content_size,
            partition_id,
            maker_code,
            version,
            program_id,
            logo_region_hash,
            product_code,
            extended_header_hash,
            extended_header_size,
            reserved_flags,
            secondary_key_slot,
            platform,
            is_data: type_flags & 1 != 0,
            is_executable: type_flags & 2 != 0,
            content_type: (type_flags >> 2) & 0x7,
            content_unit_size,
            fixed_key: crypto_flags & 1 != 0,
            no_romfs: crypto_flags & 2 != 0,
            no_crypto: crypto_flags & 4 != 0,
            seed_crypto: crypto_flags & 0x20 != 0,
            plain_region_offset,
            plain_region_size,
            logo_region_offset,
            logo_region_size,
            exefs_offset,
            exefs_size,
            exefs_hash_region_size,
            romfs_offset,
            romfs_size,
            romfs_hash_region_size,
            exefs_super_block_hash: reader.array()?,
            romfs_super_block_hash: reader.array()?,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExHeader {
    pub name: [u8; 8],
    pub compressed: bool,
    pub jump_id: u64,
    pub program_id: u64,
}

impl BinaryFormat for ExHeader {
    fn encode(&self) -> Vec<u8> {
        let mut writer = Writer::new();
        writer
            .put(&self.name)
            .zeros(5)
            .u8(u8::from(self.compressed))
            .pad_to(0x1C8)
            .u64_le(self.jump_id)
            .pad_to(0x200)
            .u64_le(self.program_id)
            .pad_to(EXHEADER_SIZE);
        writer.finish()
    }

    fn decode(bytes: &[u8]) -> Result<Self, FormatError> {
        if bytes.len() < EXHEADER_SIZE {
            return Err(FormatError::Truncated);
        }
        let mut reader = Reader::new(bytes);
        let name = reader.array()?;
        reader.skip(5)?;
        let compressed = reader.u8()? & 1 != 0;
        reader.skip(0x1C8 - 14)?;
        let jump_id = reader.u64_le()?;
        reader.skip(0x200 - 0x1D0)?;
        Ok(Self {
            name,
            compressed,
            jump_id,
            program_id: reader.u64_le()?,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExeFsSection {
    pub name: String,
    pub offset: u32,
    pub size: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExeFsHeader {
    pub sections: Vec<ExeFsSection>,
}

impl BinaryFormat for ExeFsHeader {
    fn encode(&self) -> Vec<u8> {
        let mut writer = Writer::new();
        for index in 0..EXEFS_SECTIONS {
            let section = self.sections.get(index).cloned().unwrap_or_default();
            let mut name = [0u8; 8];
            let bytes = section.name.as_bytes();
            name[..bytes.len().min(8)].copy_from_slice(&bytes[..bytes.len().min(8)]);
            writer.put(&name).u32_le(section.offset).u32_le(section.size);
        }
        writer.pad_to(EXEFS_HEADER_SIZE);
        writer.finish()
    }

    fn decode(bytes: &[u8]) -> Result<Self, FormatError> {
        let mut reader = Reader::new(bytes);
        let mut sections = Vec::new();
        for _ in 0..EXEFS_SECTIONS {
            let name: [u8; 8] = reader.array()?;
            let length = name.iter().position(|byte| *byte == 0).unwrap_or(8);
            sections.push(ExeFsSection {
                name: String::from_utf8_lossy(&name[..length]).into_owned(),
                offset: reader.u32_le()?,
                size: reader.u32_le()?,
            });
        }
        reader.skip(EXEFS_HEADER_SIZE - EXEFS_SECTIONS * 16)?;
        Ok(Self { sections })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headers_have_the_size_of_the_core_structures() {
        assert_eq!(NcsdHeader::default().encode().len(), NCSD_HEADER_SIZE);
        assert_eq!(NcchHeader::default().encode().len(), NCCH_HEADER_SIZE);
        assert_eq!(ExHeader::default().encode().len(), EXHEADER_SIZE);
        assert_eq!(ExeFsHeader::default().encode().len(), EXEFS_HEADER_SIZE);
    }

    #[test]
    fn ncch_field_offsets_match_the_core_structure() {
        let header = NcchHeader {
            content_size: 0x11223344,
            program_id: 0x0004000000ABCDEF,
            extended_header_size: 0x400,
            is_executable: true,
            no_crypto: true,
            exefs_offset: 5,
            exefs_size: 6,
            romfs_offset: 7,
            ..Default::default()
        };
        let bytes = header.encode();
        assert_eq!(&bytes[0x100..0x104], b"NCCH");
        assert_eq!(&bytes[0x104..0x108], &0x11223344u32.to_le_bytes());
        assert_eq!(&bytes[0x118..0x120], &0x0004000000ABCDEFu64.to_le_bytes());
        assert_eq!(&bytes[0x180..0x184], &0x400u32.to_le_bytes());
        assert_eq!(bytes[0x18D], 0b10);
        assert_eq!(bytes[0x18F], 0b100);
        assert_eq!(&bytes[0x1A0..0x1A4], &5u32.to_le_bytes());
        assert_eq!(&bytes[0x1A4..0x1A8], &6u32.to_le_bytes());
        assert_eq!(&bytes[0x1B0..0x1B4], &7u32.to_le_bytes());
    }

    #[test]
    fn ncsd_partitions_start_after_the_media_fields() {
        let mut header = NcsdHeader::default();
        header.partitions[0] = NcsdPartition { offset: 0x80, size: 0x100 };
        let bytes = header.encode();
        assert_eq!(&bytes[0x100..0x104], b"NCSD");
        assert_eq!(&bytes[0x120..0x124], &0x80u32.to_le_bytes());
        assert_eq!(&bytes[0x124..0x128], &0x100u32.to_le_bytes());
    }

    #[test]
    fn headers_round_trip() {
        let ncch = NcchHeader {
            content_size: 9,
            program_id: 0x00040010_00001234,
            product_code: *b"CTR-P-TEST\0\0\0\0\0\0",
            fixed_key: true,
            no_crypto: false,
            seed_crypto: true,
            exefs_offset: 4,
            exefs_size: 8,
            ..Default::default()
        };
        assert_eq!(NcchHeader::decode(&ncch.encode()), Ok(ncch));
        let mut ncsd = NcsdHeader::default();
        ncsd.partitions[0] = NcsdPartition { offset: 1, size: 2 };
        ncsd.media_size = 77;
        assert_eq!(NcsdHeader::decode(&ncsd.encode()), Ok(ncsd));
        let exheader = ExHeader {
            name: *b"APP\0\0\0\0\0",
            compressed: true,
            jump_id: 5,
            program_id: 6,
        };
        assert_eq!(ExHeader::decode(&exheader.encode()), Ok(exheader));
        let exefs = ExeFsHeader {
            sections: vec![
                ExeFsSection { name: "icon".into(), offset: 0, size: 0x36C0 },
                ExeFsSection { name: ".code".into(), offset: 0x3800, size: 4 },
            ],
        };
        let decoded = ExeFsHeader::decode(&exefs.encode()).unwrap();
        assert_eq!(&decoded.sections[..2], &exefs.sections[..]);
        assert!(decoded.sections[2..].iter().all(|s| s.name.is_empty()));
    }

    #[test]
    fn rejects_the_wrong_magic() {
        assert!(NcchHeader::decode(&NcsdHeader::default().encode()).is_err());
        assert!(NcsdHeader::decode(&NcchHeader::default().encode()).is_err());
    }
}
