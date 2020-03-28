use super::{Instruction, InstructionInfo};
use classfile::OpCode;

pub struct I2S;

impl Instruction for I2S {
    fn run(&self, codes: &[u8], pc: usize) -> (InstructionInfo, usize) {
        let info = InstructionInfo {
            name: OpCode::i2s.into(),
            code: codes[pc],
            icp: 0,
        };

        (info, pc + 1)
    }
}