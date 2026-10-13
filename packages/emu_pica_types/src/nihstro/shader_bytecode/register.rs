// Copyright 2014 Tony Wasserka
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
//     * Redistributions of source code must retain the above copyright
//       notice, this list of conditions and the following disclaimer.
//     * Redistributions in binary form must reproduce the above copyright
//       notice, this list of conditions and the following disclaimer in the
//       documentation and/or other materials provided with the distribution.
//     * Neither the name of the owner nor the names of its contributors may
//       be used to endorse or promote products derived from this software
//       without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
// "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
// LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
// OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
// SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
// LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
// DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
// THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegisterType {
    Input,
    Output,
    Temporary,
    FloatUniform,
    IntUniform,
    BoolUniform,
    Address,
    ConditionalCode,
    Unknown,
}

pub fn get_register_name(ty: RegisterType) -> &'static str {
    match ty {
        RegisterType::Input => "v",
        RegisterType::Output => "o",
        RegisterType::Temporary => "r",
        RegisterType::FloatUniform => "c",
        RegisterType::IntUniform => "i",
        RegisterType::BoolUniform => "b",
        RegisterType::ConditionalCode => "cc",
        RegisterType::Unknown => "u",
        _ => "",
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SourceRegister(pub u32);

impl SourceRegister {
    pub fn get_register_type(self) -> RegisterType {
        if self.0 < 0x10 {
            RegisterType::Input
        } else if self.0 < 0x20 {
            RegisterType::Temporary
        } else {
            RegisterType::FloatUniform
        }
    }

    pub fn get_index(self) -> i32 {
        match self.get_register_type() {
            RegisterType::Input => self.0 as i32,
            RegisterType::Temporary => self.0 as i32 - 0x10,
            _ => self.0 as i32 - 0x20,
        }
    }

    pub fn from_type_and_index(ty: RegisterType, index: i32) -> Self {
        let mut reg = Self(0);
        if ty == RegisterType::Input {
            reg.0 = index as u32;
        } else if ty == RegisterType::Temporary {
            reg.0 = (index + 0x10) as u32;
        } else if ty == RegisterType::FloatUniform {
            reg.0 = (index + 0x20) as u32;
        } else {
            // TODO: Should throw an exception or something.
        }
        reg
    }

    pub fn make_input(index: i32) -> Self {
        Self::from_type_and_index(RegisterType::Input, index)
    }

    pub fn make_temporary(index: i32) -> Self {
        Self::from_type_and_index(RegisterType::Temporary, index)
    }

    pub fn make_float(index: i32) -> Self {
        Self::from_type_and_index(RegisterType::FloatUniform, index)
    }

    pub fn get_name(self) -> String {
        format!(
            "{}{}",
            get_register_name(self.get_register_type()),
            self.get_index()
        )
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DestRegister(pub u32);

impl DestRegister {
    pub fn get_register_type(self) -> RegisterType {
        if self.0 < 0x10 {
            RegisterType::Output
        } else {
            RegisterType::Temporary
        }
    }

    pub fn get_index(self) -> i32 {
        match self.get_register_type() {
            RegisterType::Output => self.0 as i32,
            RegisterType::Temporary => self.0 as i32 - 0x10,
            // if (GetRegisterType() == RegisterType::FloatUniform)
            // TODO: This will lead to negative returned values...
            _ => self.0 as i32 - 0x20,
        }
    }

    pub fn from_type_and_index(ty: RegisterType, index: i32) -> Self {
        let mut reg = Self(0);
        if ty == RegisterType::Output {
            reg.0 = index as u32;
        } else if ty == RegisterType::Temporary {
            reg.0 = (index + 0x10) as u32;
        } else if ty == RegisterType::FloatUniform {
            // TODO: Wait what? These shouldn't be writable..
            reg.0 = (index + 0x20) as u32;
        } else {
            // TODO: Should throw an exception or something.
        }
        reg
    }

    pub fn make_output(index: i32) -> Self {
        Self::from_type_and_index(RegisterType::Output, index)
    }

    pub fn make_temporary(index: i32) -> Self {
        Self::from_type_and_index(RegisterType::Temporary, index)
    }

    pub fn get_name(self) -> String {
        format!(
            "{}{}",
            get_register_name(self.get_register_type()),
            self.get_index()
        )
    }
}
