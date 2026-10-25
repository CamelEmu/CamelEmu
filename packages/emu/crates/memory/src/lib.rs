use thiserror::Error;

const DUMP_CHUNK: usize = 1 << 20;

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("the memory is not available")]
    Unavailable,
    #[error("the 3DS memory range {address:#010x}+{len} is not mapped")]
    InvalidAddress { address: u32, len: u32 },
    #[error("the core reported status {0}")]
    Core(i32),
}

pub type Result<T> = std::result::Result<T, MemoryError>;

pub trait MemorySource {
    fn fcram_size(&self) -> usize;
    fn read_memory(&self, address: u32, len: usize) -> Result<Vec<u8>>;
    fn read_fcram(&self, offset: usize, len: usize) -> Result<Vec<u8>>;
}

/// A read-only view of the memory behind one [`MemorySource`].
///
/// The view borrows the source, so the borrow checker guarantees it cannot
/// outlive it and that the memory is never read after it was freed.
pub struct Memory3ds<'a> {
    source: &'a dyn MemorySource,
}

impl<'a> Memory3ds<'a> {
    pub fn new(source: &'a dyn MemorySource) -> Self {
        Self { source }
    }

    pub fn fcram_size(&self) -> usize {
        self.source.fcram_size()
    }

    pub fn read_block(&self, address: u32, len: usize) -> Result<Vec<u8>> {
        self.source.read_memory(address, len)
    }

    pub fn read_u8(&self, address: u32) -> Result<u8> {
        Ok(self.read_array::<1>(address)?[0])
    }

    pub fn read_u16(&self, address: u32) -> Result<u16> {
        Ok(u16::from_le_bytes(self.read_array(address)?))
    }

    pub fn read_u32(&self, address: u32) -> Result<u32> {
        Ok(u32::from_le_bytes(self.read_array(address)?))
    }

    pub fn read_u64(&self, address: u32) -> Result<u64> {
        Ok(u64::from_le_bytes(self.read_array(address)?))
    }

    pub fn dump_fcram(&self) -> Result<Vec<u8>> {
        let size = self.fcram_size();
        if size == 0 {
            return Err(MemoryError::Unavailable);
        }
        let mut dump = Vec::with_capacity(size);
        let mut offset = 0;
        while offset < size {
            let len = DUMP_CHUNK.min(size - offset);
            dump.extend_from_slice(&self.source.read_fcram(offset, len)?);
            offset += len;
        }
        Ok(dump)
    }

    fn read_array<const N: usize>(&self, address: u32) -> Result<[u8; N]> {
        let bytes = self.source.read_memory(address, N)?;
        Ok(bytes
            .try_into()
            .expect("the source returns as many bytes as were asked for"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake {
        fcram: Vec<u8>,
    }

    impl MemorySource for Fake {
        fn fcram_size(&self) -> usize {
            self.fcram.len()
        }

        fn read_memory(&self, address: u32, len: usize) -> Result<Vec<u8>> {
            let start = address as usize;
            self.fcram
                .get(start..start + len)
                .map(<[u8]>::to_vec)
                .ok_or(MemoryError::InvalidAddress {
                    address,
                    len: len as u32,
                })
        }

        fn read_fcram(&self, offset: usize, len: usize) -> Result<Vec<u8>> {
            self.read_memory(offset as u32, len)
        }
    }

    fn fake(len: usize) -> Fake {
        Fake {
            fcram: (0..len).map(|index| index as u8).collect(),
        }
    }

    #[test]
    fn reads_integers_little_endian() {
        let source = fake(16);
        let memory = Memory3ds::new(&source);
        assert_eq!(memory.read_u8(1).unwrap(), 0x01);
        assert_eq!(memory.read_u16(1).unwrap(), 0x0201);
        assert_eq!(memory.read_u32(1).unwrap(), 0x0403_0201);
        assert_eq!(memory.read_u64(1).unwrap(), 0x0807_0605_0403_0201);
    }

    #[test]
    fn read_block_outside_the_memory_is_invalid() {
        let source = fake(4);
        let memory = Memory3ds::new(&source);
        assert!(matches!(
            memory.read_block(2, 4),
            Err(MemoryError::InvalidAddress { address: 2, len: 4 })
        ));
    }

    #[test]
    fn dump_spans_more_than_one_chunk() {
        let source = fake(DUMP_CHUNK + 3);
        let memory = Memory3ds::new(&source);
        assert_eq!(memory.dump_fcram().unwrap(), source.fcram);
    }

    #[test]
    fn dump_without_fcram_is_unavailable() {
        let source = fake(0);
        let memory = Memory3ds::new(&source);
        assert!(matches!(memory.dump_fcram(), Err(MemoryError::Unavailable)));
    }
}
