/// The physical memory of the console, as the PICA reads and writes it.
pub trait PhysicalMemory {
    /// Returns a pointer to the physical address, or null when nothing is mapped there.
    fn get_physical_pointer(&self, address: u32) -> *mut u8;
}
