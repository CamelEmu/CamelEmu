/// GSP interrupt ID
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterruptId {
    PSC0 = 0x00,
    PSC1 = 0x01,
    PDC0 = 0x02,
    PDC1 = 0x03,
    PPF = 0x04,
    P3D = 0x05,
    DMA = 0x06,
}

pub type InterruptHandler = std::rc::Rc<dyn Fn(InterruptId)>;
