
use crate::format::{BinaryFormat, FormatError, Reader, Writer};

pub const SMDH_SIZE: usize = 0x36C0;
pub const LARGE_ICON_SIZE: usize = 48;
pub const SMALL_ICON_SIZE: usize = 24;

const MAGIC: [u8; 4] = *b"SMDH";
const TITLE_COUNT: usize = 16;
const SHORT_TITLE_UNITS: usize = 0x40;
const LONG_TITLE_UNITS: usize = 0x80;
const PUBLISHER_UNITS: usize = 0x40;

pub const ENGLISH: usize = 1;
pub const FLAG_VISIBLE: u32 = 1;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SmdhTitle {
    pub short: String,
    pub long: String,
    pub publisher: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Smdh {
    pub version: u16,
    pub titles: [SmdhTitle; TITLE_COUNT],
    pub ratings: [u8; 16],
    pub region_lockout: u32,
    pub match_maker_id: u32,
    pub match_maker_bit_id: u64,
    pub flags: u32,
    pub eula_version: u16,
    pub banner_animation_frame: f32,
    pub cec_id: u32,
    pub small_icon: Vec<u16>,
    pub large_icon: Vec<u16>,
}

impl Default for Smdh {
    fn default() -> Self {
        Self {
            version: 0,
            titles: Default::default(),
            ratings: [0; 16],
            region_lockout: 0,
            match_maker_id: 0,
            match_maker_bit_id: 0,
            flags: 0,
            eula_version: 0,
            banner_animation_frame: 0.0,
            cec_id: 0,
            small_icon: vec![0; SMALL_ICON_SIZE * SMALL_ICON_SIZE],
            large_icon: vec![0; LARGE_ICON_SIZE * LARGE_ICON_SIZE],
        }
    }
}

fn morton_interleave(x: usize, y: usize) -> usize {
    const XLUT: [usize; 8] = [0x00, 0x01, 0x04, 0x05, 0x10, 0x11, 0x14, 0x15];
    const YLUT: [usize; 8] = [0x00, 0x02, 0x08, 0x0a, 0x20, 0x22, 0x28, 0x2a];
    XLUT[x % 8] + YLUT[y % 8]
}

fn icon_offset(x: usize, y: usize, size: usize) -> usize {
    let morton = (morton_interleave(x, y) + (x & !7) * 8) * 2;
    morton + (y & !7) * size * 2
}

fn decode_icon(bytes: &[u8], size: usize) -> Vec<u16> {
    let mut icon = vec![0; size * size];
    for x in 0..size {
        for y in 0..size {
            let offset = icon_offset(x, y, size);
            icon[x + size * y] = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
        }
    }
    icon
}

fn encode_icon(icon: &[u16], size: usize) -> Vec<u8> {
    let mut bytes = vec![0; size * size * 2];
    for x in 0..size {
        for y in 0..size {
            let offset = icon_offset(x, y, size);
            let pixel = icon.get(x + size * y).copied().unwrap_or(0).to_le_bytes();
            bytes[offset] = pixel[0];
            bytes[offset + 1] = pixel[1];
        }
    }
    bytes
}

fn decode_utf16(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair))
        .take_while(|unit| *unit != 0)
        .collect();
    String::from_utf16_lossy(&units)
}

fn encode_utf16(text: &str, units: usize, writer: &mut Writer) {
    let mut written = 0;
    for unit in text.encode_utf16().take(units) {
        writer.u16_le(unit);
        written += 1;
    }
    writer.zeros((units - written) * 2);
}

impl Smdh {
    pub fn is_valid(bytes: &[u8]) -> bool {
        bytes.len() >= SMDH_SIZE && bytes[..4] == MAGIC
    }

    pub fn title(&self, language: usize) -> &SmdhTitle {
        &self.titles[language]
    }

    pub fn is_visible(&self) -> bool {
        self.flags & FLAG_VISIBLE != 0
    }
}

impl BinaryFormat for Smdh {
    fn encode(&self) -> Vec<u8> {
        let mut writer = Writer::new();
        writer.put(&MAGIC).u16_le(self.version).zeros(2);
        for title in &self.titles {
            encode_utf16(&title.short, SHORT_TITLE_UNITS, &mut writer);
            encode_utf16(&title.long, LONG_TITLE_UNITS, &mut writer);
            encode_utf16(&title.publisher, PUBLISHER_UNITS, &mut writer);
        }
        writer
            .put(&self.ratings)
            .u32_le(self.region_lockout)
            .u32_le(self.match_maker_id)
            .u64_le(self.match_maker_bit_id)
            .u32_le(self.flags)
            .u16_le(self.eula_version)
            .zeros(2)
            .u32_le(self.banner_animation_frame.to_bits())
            .u32_le(self.cec_id)
            .zeros(8)
            .put(&encode_icon(&self.small_icon, SMALL_ICON_SIZE))
            .put(&encode_icon(&self.large_icon, LARGE_ICON_SIZE));
        writer.finish()
    }

    fn decode(bytes: &[u8]) -> Result<Self, FormatError> {
        if !Self::is_valid(bytes) {
            return Err(FormatError::Invalid("SMDH"));
        }
        let mut reader = Reader::new(bytes);
        reader.skip(4)?;
        let version = reader.u16_le()?;
        reader.skip(2)?;
        let mut titles: [SmdhTitle; TITLE_COUNT] = Default::default();
        for title in &mut titles {
            title.short = decode_utf16(reader.take(SHORT_TITLE_UNITS * 2)?);
            title.long = decode_utf16(reader.take(LONG_TITLE_UNITS * 2)?);
            title.publisher = decode_utf16(reader.take(PUBLISHER_UNITS * 2)?);
        }
        let ratings = reader.array()?;
        let region_lockout = reader.u32_le()?;
        let match_maker_id = reader.u32_le()?;
        let match_maker_bit_id = reader.u64_le()?;
        let flags = reader.u32_le()?;
        let eula_version = reader.u16_le()?;
        reader.skip(2)?;
        let banner_animation_frame = f32::from_bits(reader.u32_le()?);
        let cec_id = reader.u32_le()?;
        reader.skip(8)?;
        let small_icon = decode_icon(reader.take(SMALL_ICON_SIZE * SMALL_ICON_SIZE * 2)?, SMALL_ICON_SIZE);
        let large_icon = decode_icon(reader.take(LARGE_ICON_SIZE * LARGE_ICON_SIZE * 2)?, LARGE_ICON_SIZE);
        Ok(Self {
            version,
            titles,
            ratings,
            region_lockout,
            match_maker_id,
            match_maker_bit_id,
            flags,
            eula_version,
            banner_animation_frame,
            cec_id,
            small_icon,
            large_icon,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoded_size_matches_the_core_structure() {
        assert_eq!(Smdh::default().encode().len(), SMDH_SIZE);
    }

    #[test]
    fn field_offsets_match_the_core_structure() {
        let mut smdh = Smdh {
            version: 0x0102,
            region_lockout: 0x7F,
            flags: 1,
            ..Default::default()
        };
        smdh.titles[ENGLISH].long = "A".into();
        smdh.titles[ENGLISH].publisher = "B".into();
        let bytes = smdh.encode();
        assert_eq!(&bytes[..4], b"SMDH");
        assert_eq!(&bytes[4..6], &[0x02, 0x01]);
        assert_eq!(bytes[8 + 0x200 + 0x80], b'A');
        assert_eq!(bytes[8 + 0x200 + 0x80 + 0x100], b'B');
        assert_eq!(&bytes[0x2018..0x201C], &[0x7F, 0, 0, 0]);
        assert_eq!(&bytes[0x2028..0x202C], &[1, 0, 0, 0]);
    }

    #[test]
    fn round_trip_keeps_titles_icons_and_flags() {
        let mut smdh = Smdh::default();
        smdh.titles[0].long = "日本語のタイトル".into();
        smdh.titles[ENGLISH].short = "Short".into();
        smdh.titles[ENGLISH].long = "Long title".into();
        smdh.titles[ENGLISH].publisher = "Publisher".into();
        smdh.region_lockout = 0b0000_0110;
        smdh.flags = FLAG_VISIBLE;
        smdh.banner_animation_frame = 1.5;
        for (index, pixel) in smdh.large_icon.iter_mut().enumerate() {
            *pixel = (index as u16).wrapping_mul(31);
        }
        for (index, pixel) in smdh.small_icon.iter_mut().enumerate() {
            *pixel = (index as u16).wrapping_mul(17);
        }
        assert_eq!(Smdh::decode(&smdh.encode()), Ok(smdh));
    }

    #[test]
    fn icon_pixels_are_stored_in_morton_order() {
        let mut smdh = Smdh::default();
        smdh.large_icon[0] = 0xAAAA;
        smdh.large_icon[1] = 0xBBBB;
        smdh.large_icon[LARGE_ICON_SIZE] = 0xCCCC;
        let bytes = smdh.encode();
        let icon = &bytes[0x24C0..];
        assert_eq!(&icon[0..2], &[0xAA, 0xAA]);
        assert_eq!(&icon[2..4], &[0xBB, 0xBB]);
        assert_eq!(&icon[4..6], &[0xCC, 0xCC]);
    }

    #[test]
    fn rejects_data_without_the_magic() {
        let mut bytes = Smdh::default().encode();
        bytes[0] = b'X';
        assert_eq!(Smdh::decode(&bytes), Err(FormatError::Invalid("SMDH")));
        assert_eq!(Smdh::decode(&bytes[..100]), Err(FormatError::Invalid("SMDH")));
    }
}
