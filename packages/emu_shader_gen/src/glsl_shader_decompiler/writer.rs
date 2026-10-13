use emu_pica_types::nihstro::shader_bytecode::{Selector, SwizzlePattern};

#[derive(Default)]
pub(super) struct ShaderWriter {
    pub scope: i32,
    shader_source: String,
}

impl ShaderWriter {
    pub fn add_line(&mut self, text: impl AsRef<str>) {
        self.add_expression(text.as_ref());
        self.add_new_line();
    }

    pub fn add_new_line(&mut self) {
        debug_assert!(self.scope >= 0);
        self.shader_source.push('\n');
    }

    pub fn move_result(self) -> String {
        self.shader_source
    }

    fn add_expression(&mut self, text: &str) {
        if !text.is_empty() {
            self.shader_source
                .push_str(&" ".repeat(self.scope as usize * 4));
        }
        self.shader_source.push_str(text);
    }
}

/// An adaptor for getting swizzle pattern string from nihstro interfaces.
fn get_selector_src(getter: impl Fn(usize) -> Selector) -> String {
    (0..4)
        .map(|i| match getter(i) {
            Selector::X => 'x',
            Selector::Y => 'y',
            Selector::Z => 'z',
            Selector::W => 'w',
            _ => unreachable!(),
        })
        .collect()
}

pub(super) fn get_selector_src1(pattern: &SwizzlePattern) -> String {
    get_selector_src(|i| pattern.get_selector_src1(i))
}

pub(super) fn get_selector_src2(pattern: &SwizzlePattern) -> String {
    get_selector_src(|i| pattern.get_selector_src2(i))
}

pub(super) fn get_selector_src3(pattern: &SwizzlePattern) -> String {
    get_selector_src(|i| pattern.get_selector_src3(i))
}
