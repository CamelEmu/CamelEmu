use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FormatError {
    #[error("the data ends before the structure does")]
    Truncated,
    #[error("the data is not a valid {0}")]
    Invalid(&'static str),
}

pub trait BinaryFormat: Sized {
    fn encode(&self) -> Vec<u8>;

    fn decode(bytes: &[u8]) -> Result<Self, FormatError>;
}

pub struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    pub fn position(&self) -> usize {
        self.position
    }

    pub fn take(&mut self, length: usize) -> Result<&'a [u8], FormatError> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(FormatError::Truncated)?;
        let slice = self
            .bytes
            .get(self.position..end)
            .ok_or(FormatError::Truncated)?;
        self.position = end;
        Ok(slice)
    }

    pub fn skip(&mut self, length: usize) -> Result<(), FormatError> {
        self.take(length).map(|_| ())
    }

    pub fn array<const N: usize>(&mut self) -> Result<[u8; N], FormatError> {
        let mut array = [0; N];
        array.copy_from_slice(self.take(N)?);
        Ok(array)
    }

    pub fn u8(&mut self) -> Result<u8, FormatError> {
        Ok(self.array::<1>()?[0])
    }

    pub fn u16_le(&mut self) -> Result<u16, FormatError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    pub fn u32_le(&mut self) -> Result<u32, FormatError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    pub fn u64_le(&mut self) -> Result<u64, FormatError> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    pub fn u16_be(&mut self) -> Result<u16, FormatError> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    pub fn u32_be(&mut self) -> Result<u32, FormatError> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    pub fn u64_be(&mut self) -> Result<u64, FormatError> {
        Ok(u64::from_be_bytes(self.array()?))
    }
}

#[derive(Default)]
pub struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub fn put(&mut self, bytes: &[u8]) -> &mut Self {
        self.bytes.extend_from_slice(bytes);
        self
    }

    pub fn zeros(&mut self, length: usize) -> &mut Self {
        self.bytes.resize(self.bytes.len() + length, 0);
        self
    }

    pub fn u8(&mut self, value: u8) -> &mut Self {
        self.put(&[value])
    }

    pub fn u16_le(&mut self, value: u16) -> &mut Self {
        self.put(&value.to_le_bytes())
    }

    pub fn u32_le(&mut self, value: u32) -> &mut Self {
        self.put(&value.to_le_bytes())
    }

    pub fn u64_le(&mut self, value: u64) -> &mut Self {
        self.put(&value.to_le_bytes())
    }

    pub fn u16_be(&mut self, value: u16) -> &mut Self {
        self.put(&value.to_be_bytes())
    }

    pub fn u32_be(&mut self, value: u32) -> &mut Self {
        self.put(&value.to_be_bytes())
    }

    pub fn u64_be(&mut self, value: u64) -> &mut Self {
        self.put(&value.to_be_bytes())
    }

    pub fn pad_to(&mut self, length: usize) -> &mut Self {
        if self.bytes.len() < length {
            self.bytes.resize(length, 0);
        }
        self
    }

    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reader_reports_truncation() {
        let mut reader = Reader::new(&[1, 2, 3]);
        assert_eq!(reader.u16_le(), Ok(0x0201));
        assert_eq!(reader.u16_le(), Err(FormatError::Truncated));
    }

    #[test]
    fn writer_and_reader_agree_on_endianness() {
        let mut writer = Writer::new();
        writer.u32_le(0x11223344).u32_be(0x11223344);
        let bytes = writer.finish();
        assert_eq!(bytes, [0x44, 0x33, 0x22, 0x11, 0x11, 0x22, 0x33, 0x44]);
        let mut reader = Reader::new(&bytes);
        assert_eq!(reader.u32_le(), Ok(0x11223344));
        assert_eq!(reader.u32_be(), Ok(0x11223344));
    }
}
