use std::collections::BTreeSet;

use crate::id::Id;
use crate::operand::Literal;
use crate::stream::{Declarations, Stream, words_in_string};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ModuleSettings {
    pub generator_magic_number: u32,
}

#[derive(Debug)]
pub struct Module {
    pub(crate) settings: ModuleSettings,
    pub(crate) version: u32,
    pub(crate) bound: u32,

    extensions: BTreeSet<String>,
    capabilities: BTreeSet<u32>,
    pub(crate) glsl_std_450: Option<Id>,

    addressing_model: spirv::AddressingModel,
    memory_model: spirv::MemoryModel,

    pub(crate) ext_inst_imports: Stream,
    entry_points: Stream,
    execution_modes: Stream,
    pub(crate) debug: Stream,
    pub(crate) annotations: Stream,
    pub(crate) declarations: Declarations,
    global_variables: Stream,
    pub(crate) code: Stream,
    pub(crate) deferred_phi_nodes: Vec<u32>,
}

impl Module {
    pub fn new(version: u32) -> Self {
        Self {
            settings: ModuleSettings::default(),
            version,
            bound: 0,
            extensions: BTreeSet::new(),
            capabilities: BTreeSet::new(),
            glsl_std_450: None,
            addressing_model: spirv::AddressingModel::Logical,
            memory_model: spirv::MemoryModel::GLSL450,
            ext_inst_imports: Stream::new(),
            entry_points: Stream::new(),
            execution_modes: Stream::new(),
            debug: Stream::new(),
            annotations: Stream::new(),
            declarations: Declarations::new(),
            global_variables: Stream::new(),
            code: Stream::new(),
            deferred_phi_nodes: Vec::new(),
        }
    }

    pub fn assemble(&self) -> Vec<u32> {
        let mut words = vec![
            spirv::MAGIC_NUMBER,
            self.version,
            self.settings.generator_magic_number,
            self.bound + 1,
            0,
        ];

        words.reserve(self.capabilities.len() * 2);
        for &capability in &self.capabilities {
            words.extend([make_word0(spirv::Op::Capability, 2), capability]);
        }

        for extension_name in &self.extensions {
            let string_words = words_in_string(extension_name);
            words.push(make_word0(spirv::Op::Extension, string_words + 1));
            let mut string = Stream::new();
            string.reserve(string_words);
            string.put_string(extension_name);
            words.extend_from_slice(string.words());
        }

        words.extend_from_slice(self.ext_inst_imports.words());

        words.extend([
            make_word0(spirv::Op::MemoryModel, 3),
            self.addressing_model as u32,
            self.memory_model as u32,
        ]);

        words.extend_from_slice(self.entry_points.words());
        words.extend_from_slice(self.execution_modes.words());
        words.extend_from_slice(self.debug.words());
        words.extend_from_slice(self.annotations.words());
        words.extend_from_slice(self.declarations.words());
        words.extend_from_slice(self.global_variables.words());
        words.extend_from_slice(self.code.words());

        words
    }

    pub fn patch_deferred_phi(&mut self, mut func: impl FnMut(usize) -> Id) {
        for &phi_index in &self.deferred_phi_nodes {
            let first_word = self.code.value(phi_index);
            debug_assert!(first_word & 0xffff == spirv::Op::Phi as u32);
            let num_words = first_word >> 16;
            let num_args = (num_words - 3) / 2;
            let mut cursor = phi_index + 3;
            for arg in 0..num_args {
                self.code.set_value(cursor, func(arg as usize).value);
                cursor += 2;
            }
        }
    }

    pub fn add_extension(&mut self, extension_name: impl Into<String>) {
        self.extensions.insert(extension_name.into());
    }

    pub fn add_capability(&mut self, capability: spirv::Capability) {
        self.capabilities.insert(capability as u32);
    }

    pub fn set_memory_model(
        &mut self,
        addressing_model: spirv::AddressingModel,
        memory_model: spirv::MemoryModel,
    ) {
        self.addressing_model = addressing_model;
        self.memory_model = memory_model;
    }

    pub fn add_entry_point(
        &mut self,
        execution_model: spirv::ExecutionModel,
        entry_point: Id,
        name: &str,
        interfaces: &[Id],
    ) {
        self.entry_points.emit(
            &mut self.bound,
            4 + words_in_string(name) + interfaces.len(),
            spirv::Op::EntryPoint,
            &[&execution_model, &entry_point, &name, &interfaces],
        );
    }

    pub fn add_execution_mode(
        &mut self,
        entry_point: Id,
        mode: spirv::ExecutionMode,
        literals: &[&dyn Literal],
    ) {
        self.execution_modes.emit(
            &mut self.bound,
            3 + literals.len(),
            spirv::Op::ExecutionMode,
            &[&entry_point, &mode, &literals],
        );
    }

    pub fn add_label(&mut self, label: Id) -> Id {
        debug_assert!(label.value != 0);
        self.code.reserve(2);
        self.code
            .emit(&mut self.bound, 0, spirv::Op::Label, &[&label]);
        label
    }

    pub fn add_local_variable(
        &mut self,
        result_type: Id,
        storage_class: spirv::StorageClass,
        initializer: Option<Id>,
    ) -> Id {
        self.code.emit_typed(
            &mut self.bound,
            5,
            spirv::Op::Variable,
            result_type,
            &[&storage_class, &initializer],
        )
    }

    pub fn add_global_variable(
        &mut self,
        result_type: Id,
        storage_class: spirv::StorageClass,
        initializer: Option<Id>,
    ) -> Id {
        self.global_variables.emit_typed(
            &mut self.bound,
            5,
            spirv::Op::Variable,
            result_type,
            &[&storage_class, &initializer],
        )
    }

    pub(crate) fn get_glsl_std450(&mut self) -> Id {
        if self.glsl_std_450.is_none() {
            let id = self.ext_inst_imports.emit_id(
                &mut self.bound,
                3 + 4,
                spirv::Op::ExtInstImport,
                &[&"GLSL.std.450"],
            );
            self.glsl_std_450 = Some(id);
        }
        self.glsl_std_450.unwrap_or_default()
    }
}

fn make_word0(op: spirv::Op, word_count: usize) -> u32 {
    op as u32 | (word_count as u32) << 16
}
