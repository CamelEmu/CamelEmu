use std::collections::HashMap;

use crate::id::Id;
use crate::operand::Operand;

pub fn words_in_string(string: &str) -> usize {
    string.len() / size_of::<u32>() + 1
}

#[derive(Debug, Default)]
pub struct Stream {
    words: Vec<u32>,
    insert_index: usize,
    op_index: usize,
}

impl Stream {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reserve(&mut self, num_words: usize) {
        if self.insert_index + num_words <= self.words.len() {
            return;
        }
        self.words.resize(self.insert_index + num_words, 0);
    }

    pub fn words(&self) -> &[u32] {
        &self.words[..self.insert_index]
    }

    pub fn local_address(&self) -> u32 {
        self.words.len() as u32
    }

    pub fn value(&self, index: u32) -> u32 {
        self.words[index as usize]
    }

    pub fn set_value(&mut self, index: u32, value: u32) {
        self.words[index as usize] = value;
    }

    pub fn put_word(&mut self, value: u32) {
        self.words[self.insert_index] = value;
        self.insert_index += 1;
    }

    pub fn put_string(&mut self, string: &str) {
        let bytes = string.as_bytes();
        let read = |offset: usize| {
            bytes
                .get(offset)
                .map_or(0u32, |&byte| byte as i8 as i32 as u32)
        };
        for index in (0..bytes.len()).step_by(size_of::<u32>()) {
            let word =
                read(index) | read(index + 1) << 8 | read(index + 2) << 16 | read(index + 3) << 24;
            self.put_word(word);
        }
        if bytes.len().is_multiple_of(size_of::<u32>()) {
            self.put_word(0);
        }
    }

    fn begin(&mut self, op: spirv::Op) {
        self.op_index = self.insert_index;
        self.put_word(op as u32);
    }

    fn begin_id(&mut self, bound: &mut u32, op: spirv::Op, result_type: Option<Id>) {
        self.op_index = self.insert_index;
        self.put_word(op as u32);
        if let Some(result_type) = result_type {
            debug_assert!(result_type.value != 0);
            self.put_word(result_type.value);
        }
        *bound += 1;
        self.put_word(*bound);
    }

    fn put_operands(&mut self, operands: &[&dyn Operand]) {
        for operand in operands {
            operand.write(self);
        }
    }

    fn end(&mut self, bound: u32) -> Id {
        let num_words = self.insert_index - self.op_index;
        self.words[self.op_index] |= (num_words as u32) << 16;
        Id { value: bound }
    }

    pub fn emit(
        &mut self,
        bound: &mut u32,
        reserve: usize,
        op: spirv::Op,
        operands: &[&dyn Operand],
    ) -> Id {
        self.reserve(reserve);
        self.begin(op);
        self.put_operands(operands);
        self.end(*bound)
    }

    pub fn emit_id(
        &mut self,
        bound: &mut u32,
        reserve: usize,
        op: spirv::Op,
        operands: &[&dyn Operand],
    ) -> Id {
        self.reserve(reserve);
        self.begin_id(bound, op, None);
        self.put_operands(operands);
        self.end(*bound)
    }

    pub fn emit_typed(
        &mut self,
        bound: &mut u32,
        reserve: usize,
        op: spirv::Op,
        result_type: Id,
        operands: &[&dyn Operand],
    ) -> Id {
        self.reserve(reserve);
        self.begin_id(bound, op, Some(result_type));
        self.put_operands(operands);
        self.end(*bound)
    }
}

#[derive(Debug, Default)]
pub struct Declarations {
    stream: Stream,
    existing_declarations: HashMap<Vec<u32>, u32>,
    id_index: usize,
}

impl Declarations {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn words(&self) -> &[u32] {
        self.stream.words()
    }

    pub fn emit_id(
        &mut self,
        bound: &mut u32,
        reserve: usize,
        op: spirv::Op,
        operands: &[&dyn Operand],
    ) -> Id {
        self.id_index = 1;
        self.stream.reserve(reserve);
        self.stream.begin_id(bound, op, None);
        self.stream.put_operands(operands);
        self.finish(bound)
    }

    pub fn emit_typed(
        &mut self,
        bound: &mut u32,
        reserve: usize,
        op: spirv::Op,
        result_type: Id,
        operands: &[&dyn Operand],
    ) -> Id {
        self.id_index = 2;
        self.stream.reserve(reserve);
        self.stream.begin_id(bound, op, Some(result_type));
        self.stream.put_operands(operands);
        self.finish(bound)
    }

    fn finish(&mut self, bound: &mut u32) -> Id {
        let mut declaration =
            self.stream.words[self.stream.op_index..self.stream.insert_index].to_vec();
        let id = std::mem::replace(&mut declaration[self.id_index], 0);

        if let Some(&existing) = self.existing_declarations.get(&declaration) {
            self.stream.insert_index = self.stream.op_index;
            *bound -= 1;
            return Id { value: existing };
        }
        self.existing_declarations.insert(declaration, id);
        self.stream.end(*bound)
    }
}
