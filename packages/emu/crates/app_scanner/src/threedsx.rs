use crate::format::{BinaryFormat, FormatError, Reader, Writer};

pub const HEADER_SIZE: u16 = 0x2C;
const MAGIC: [u8; 4] = *b"3DSX";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ThreeDsxHeader {
    pub header_size: u16,
    pub reloc_header_size: u16,
    pub format_version: u32,
    pub flags: u32,
    pub code_segment_size: u32,
    pub rodata_segment_size: u32,
    pub data_segment_size: u32,
    pub bss_size: u32,
    pub smdh_offset: u32,
    pub smdh_size: u32,
    pub fs_offset: u32,
}

impl ThreeDsxHeader {
    pub fn with_smdh(smdh_offset: u32, smdh_size: u32) -> Self {
        Self {
            header_size: HEADER_SIZE,
            reloc_header_size: 0xC,
            smdh_offset,
            smdh_size,
            ..Default::default()
        }
    }
}

impl BinaryFormat for ThreeDsxHeader {
    fn encode(&self) -> Vec<u8> {
        let mut writer = Writer::new();
        writer
            .put(&MAGIC)
            .u16_le(self.header_size)
            .u16_le(self.reloc_header_size)
            .u32_le(self.format_version)
            .u32_le(self.flags)
            .u32_le(self.code_segment_size)
            .u32_le(self.rodata_segment_size)
            .u32_le(self.data_segment_size)
            .u32_le(self.bss_size)
            .u32_le(self.smdh_offset)
            .u32_le(self.smdh_size)
            .u32_le(self.fs_offset);
        writer.finish()
    }

    fn decode(bytes: &[u8]) -> Result<Self, FormatError> {
        let mut reader = Reader::new(bytes);
        reader.skip(MAGIC.len())?;
        Ok(Self {
            header_size: reader.u16_le()?,
            reloc_header_size: reader.u16_le()?,
            format_version: reader.u32_le()?,
            flags: reader.u32_le()?,
            code_segment_size: reader.u32_le()?,
            rodata_segment_size: reader.u32_le()?,
            data_segment_size: reader.u32_le()?,
            bss_size: reader.u32_le()?,
            smdh_offset: reader.u32_le()?,
            smdh_size: reader.u32_le()?,
            fs_offset: reader.u32_le()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_has_the_size_of_the_core_structure() {
        assert_eq!(ThreeDsxHeader::default().encode().len(), HEADER_SIZE as usize);
    }

    #[test]
    fn smdh_location_is_stored_after_the_segment_sizes() {
        let bytes = ThreeDsxHeader::with_smdh(0x100, 0x36C0).encode();
        assert_eq!(&bytes[0..4], b"3DSX");
        assert_eq!(&bytes[0x20..0x24], &0x100u32.to_le_bytes());
        assert_eq!(&bytes[0x24..0x28], &0x36C0u32.to_le_bytes());
    }

    #[test]
    fn round_trip() {
        let header = ThreeDsxHeader::with_smdh(7, 9);
        assert_eq!(ThreeDsxHeader::decode(&header.encode()), Ok(header));
    }
}
