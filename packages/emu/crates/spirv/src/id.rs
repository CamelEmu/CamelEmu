#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Id {
    pub value: u32,
}

impl Id {
    pub fn is_valid(self) -> bool {
        self.value != 0
    }
}
