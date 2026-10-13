use emu_pica_types::Vec4;

/// The component `index` of the vector, as `Common::Vec4::operator[]` returns it.
pub fn component<T>(vector: &Vec4<T>, index: usize) -> &T {
    match index {
        0 => &vector.x,
        1 => &vector.y,
        2 => &vector.z,
        3 => &vector.w,
        _ => panic!("vector component {index} out of range"),
    }
}

pub fn component_mut<T>(vector: &mut Vec4<T>, index: usize) -> &mut T {
    match index {
        0 => &mut vector.x,
        1 => &mut vector.y,
        2 => &mut vector.z,
        3 => &mut vector.w,
        _ => panic!("vector component {index} out of range"),
    }
}
