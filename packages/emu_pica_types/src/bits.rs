pub const fn mask(width: u32) -> u32 {
    if width >= 32 {
        u32::MAX
    } else {
        (1u32 << width) - 1
    }
}

pub const fn bits(value: u32, position: u32, width: u32) -> u32 {
    (value >> position) & mask(width)
}

pub const fn sbits(value: u32, position: u32, width: u32) -> i32 {
    ((value << (32 - position - width)) as i32) >> (32 - width)
}

/// Stores `field` in the bits of `value`, truncated to the width as `BitField::Assign` does.
pub fn assign(value: &mut u32, position: u32, width: u32, field: u32) {
    let field_mask = mask(width) << position;
    *value = (*value & !field_mask) | ((field << position) & field_mask);
}

/// An enumeration stored in a bit field. Like the cast of the core, any raw value is kept, so the
/// values that have no name stay distinct.
#[macro_export]
macro_rules! pica_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident = $value:expr),* $(,)? }) => {
        $(#[$meta])*
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
        pub struct $name(pub u32);

        impl $name {
            $(pub const $variant: Self = Self($value);)*
        }
    };
}

pub use pica_enum;
