use std::collections::VecDeque;

use emu_pica_types::Vec4;
use emu_pica_types::nihstro::shader_bytecode::FlowControl;

use super::Interpreter;

#[derive(Clone, Copy)]
pub(super) struct IfStackElement {
    pub else_address: u32,
    pub end_address: u32,
}

#[derive(Clone, Copy)]
pub(super) struct CallStackElement {
    pub end_address: u32,
    pub return_address: u32,
}

#[allow(non_snake_case)]
#[derive(Clone, Copy)]
pub(super) struct LoopStackElement {
    pub entry_address: u32,
    pub end_address: u32,
    pub loop_downcounter: u8,
    pub address_increment: u8,
    pub previous_aL: u8,
}

/// A stack of a fixed capacity that drops its oldest element when it is full, as
/// `boost::circular_buffer` does.
pub(super) struct CircularBuffer<T> {
    elements: VecDeque<T>,
    capacity: usize,
}

impl<T> CircularBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            elements: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push_back(&mut self, element: T) {
        if self.elements.len() == self.capacity {
            self.elements.pop_front();
        }
        self.elements.push_back(element);
    }

    pub fn pop_back(&mut self) {
        self.elements.pop_back();
    }

    pub fn back(&self) -> Option<&T> {
        self.elements.back()
    }

    pub fn back_mut(&mut self) -> Option<&mut T> {
        self.elements.back_mut()
    }

    pub fn len(&self) -> usize {
        self.elements.len()
    }
}

impl Interpreter<'_> {
    pub(super) fn do_if(&mut self, flow_control: FlowControl, condition: bool) {
        if condition {
            self.if_stack.push_back(IfStackElement {
                else_address: flow_control.dest_offset(),
                end_address: flow_control.dest_offset() + flow_control.num_instructions(),
            });
        } else {
            self.program_counter = flow_control.dest_offset().wrapping_sub(1);
        }
    }

    pub(super) fn do_call(&mut self, flow_control: FlowControl) {
        self.call_stack.push_back(CallStackElement {
            end_address: flow_control.dest_offset() + flow_control.num_instructions(),
            return_address: self.program_counter.wrapping_add(1),
        });
        self.program_counter = flow_control.dest_offset().wrapping_sub(1);
    }

    #[allow(non_snake_case)]
    pub(super) fn do_loop(&mut self, flow_control: FlowControl, loop_param: Vec4<u8>) {
        let previous_aL = self.state.address_registers[2] as u8;
        self.loop_stack.push_back(LoopStackElement {
            entry_address: self.program_counter.wrapping_add(1),
            end_address: flow_control.dest_offset() + 1,
            loop_downcounter: loop_param.x,
            address_increment: loop_param.z,
            previous_aL,
        });
        self.state.address_registers[2] = i32::from(loop_param.y);
    }
}
