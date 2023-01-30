use crate::emitter::Emitter;
use crate::error::AotError;
use ckb_vm::decoder::build_decoder;
use ckb_vm::instructions::ast::Value;
use ckb_vm::instructions::{blank_instruction, execute_instruction, extract_opcode};
use ckb_vm::instructions::{
    execute, instruction_length, is_basic_block_end_instruction, is_slowpath_instruction,
    Instruction,
};
use ckb_vm::machine::asm::{ckb_vm_asm_labels, ckb_vm_x64_execute, AsmCoreMachine};
use ckb_vm::machine::{elf_adaptor, DefaultMachine, SupportMachine, VERSION0, VERSION1};
use ckb_vm::{
    Bytes, CoreMachine, DefaultCoreMachine, Error, FlatMemory, InstructionCycleFunc, Machine,
    Memory, Register, RISCV_MAX_MEMORY,
};
use ckb_vm_definitions::{
    asm::{
        calculate_slot, Trace, RET_CYCLES_OVERFLOW, RET_DECODE_TRACE, RET_DYNAMIC_JUMP, RET_EBREAK,
        RET_ECALL, RET_INVALID_PERMISSION, RET_MAX_CYCLES_EXCEEDED, RET_OUT_OF_BOUND, RET_SLOWPATH,
        TRACE_ITEM_LENGTH,
    },
    instructions::OP_CUSTOM_TRACE_END,
    ISA_MOP,
};
use memmap::{Mmap, MmapMut};
use scroll::Pread;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

const MAXIMUM_INSTRUCTIONS_PER_BLOCK: usize = 1024;
const MAXIMUM_LABELS: usize = 65535;
const MAXIMUM_SECTIONS: usize = 1024;
const MAXIMUM_DUMMY_SECTIONS: usize = 64;

const ADDRESS_WRITE_ONLY_FLAG: u64 = 0x8000_0000_0000_0000;
const ADDRESS_LABEL_FLAG: u64 = 0x4000_0000_0000_0000;
const MAXIMUM_ENCODED_ADDRESS: u64 = 0x8000_0000;

#[derive(Debug, Clone)]
pub enum Write {
    Memory {
        address: Value,
        size: u8,
        value: Value,
    },
    Register {
        index: usize,
        value: Value,
    },
    Pc {
        value: Value,
    },
    Ecall,
    Ebreak,
    Slowpath,
}

fn init_registers() -> [Value; 32] {
    [
        Value::Imm(0),
        Value::Register(1),
        Value::Register(2),
        Value::Register(3),
        Value::Register(4),
        Value::Register(5),
        Value::Register(6),
        Value::Register(7),
        Value::Register(8),
        Value::Register(9),
        Value::Register(10),
        Value::Register(11),
        Value::Register(12),
        Value::Register(13),
        Value::Register(14),
        Value::Register(15),
        Value::Register(16),
        Value::Register(17),
        Value::Register(18),
        Value::Register(19),
        Value::Register(20),
        Value::Register(21),
        Value::Register(22),
        Value::Register(23),
        Value::Register(24),
        Value::Register(25),
        Value::Register(26),
        Value::Register(27),
        Value::Register(28),
        Value::Register(29),
        Value::Register(30),
        Value::Register(31),
    ]
}

struct LabelGatheringMachine {
    registers: [Value; 32],
    pc: Value,
    next_pc: Value,
    labels_to_test: Vec<u64>,
    isa: u8,
    version: u32,

    // A memory segment which contains code loaded from ELF
    memory: FlatMemory<u64>,
    labels: HashSet<u64>,
    sections: Vec<(u64, u64)>,
    dummy_sections: HashMap<u64, u64>,
}

impl LabelGatheringMachine {
    pub fn load(program: &Bytes, isa: u8, version: u32) -> Result<Self, Error> {
        let section_headers: Vec<elf_adaptor::SectionHeader> = if version < VERSION1 {
            use goblin_v023::container::Ctx;
            use goblin_v023::elf::{Header, SectionHeader};

            let header = program.pread::<Header>(0)?;
            let container = header.container().map_err(|_e| Error::ElfBits)?;
            let endianness = header.endianness().map_err(|_e| Error::ElfBits)?;
            if <Self as CoreMachine>::REG::BITS != if container.is_big() { 64 } else { 32 } {
                return Err(Error::ElfBits);
            }
            let ctx = Ctx::new(container, endianness);
            SectionHeader::parse(
                program,
                header.e_shoff as usize,
                header.e_shnum as usize,
                ctx,
            )?
            .iter()
            .map(elf_adaptor::SectionHeader::from_v0)
            .collect()
        } else {
            use goblin_v040::container::Ctx;
            use goblin_v040::elf::{Header, SectionHeader};

            let header = program.pread::<Header>(0)?;
            let container = header.container().map_err(|_e| Error::ElfBits)?;
            let endianness = header.endianness().map_err(|_e| Error::ElfBits)?;
            if <Self as CoreMachine>::REG::BITS != if container.is_big() { 64 } else { 32 } {
                return Err(Error::ElfBits);
            }
            let ctx = Ctx::new(container, endianness);
            SectionHeader::parse(
                program,
                header.e_shoff as usize,
                header.e_shnum as usize,
                ctx,
            )?
            .iter()
            .map(elf_adaptor::SectionHeader::from_v1)
            .collect()
        };
        if section_headers.len() > MAXIMUM_SECTIONS {
            return Err(Error::External(
                AotError::LimitReachedMaximumSections.to_string(),
            ));
        }
        let mut sections: Vec<(u64, u64)> = section_headers
            .iter()
            .filter_map(|section_header| {
                if section_header.sh_flags & u64::from(elf_adaptor::SHF_EXECINSTR) != 0 {
                    Some((
                        section_header.sh_addr,
                        section_header.sh_addr.wrapping_add(section_header.sh_size),
                    ))
                } else {
                    None
                }
            })
            .rev()
            .collect();
        // Test there's no empty section
        if sections.iter().any(|(s, e)| s >= e) {
            return Err(Error::External(AotError::SectionIsEmpty.to_string()));
        }
        // Test no section overlaps with one another. We first sort section
        // list by start, then we test if each end is equal or less than
        // the next start.
        sections.sort_by_key(|section| section.0);
        if sections.windows(2).any(|w| w[0].1 > w[1].0) {
            return Err(Error::External(AotError::SectionOverlaps.to_string()));
        }
        // DefaultCoreMachine is only used here for loading ELF binaries
        // into memory.
        let mut inner = DefaultCoreMachine::new(isa, version, 0);
        inner.load_elf(program, false)?;

        Ok(Self {
            isa,
            version,
            registers: init_registers(),
            pc: Value::from_u64(0),
            next_pc: Value::from_u64(0),
            labels: HashSet::default(),
            labels_to_test: Vec::new(),
            memory: inner.take_memory(),
            sections,
            dummy_sections: HashMap::default(),
        })
    }

    fn read_pc(&self) -> Result<u64, Error> {
        match &self.pc {
            Value::Imm(pc) => Ok(*pc),
            _ => Err(Error::Unexpected(String::from("Unexpected value type"))),
        }
    }

    pub fn gather(&mut self) -> Result<(), Error> {
        let mut decoder = build_decoder::<u64>(self.isa(), self.version());
        for i in 0..self.sections.len() {
            let (section_start, section_end) = self.sections[i];
            self.pc = Value::from_u64(section_start);
            let mut start_of_basic_block = true;
            while self.read_pc()? < section_end {
                let pc = self.read_pc()?;
                match decoder.decode(&mut self.memory, pc) {
                    Ok(instruction) => {
                        if start_of_basic_block {
                            self.labels.insert(pc);
                        }
                        start_of_basic_block = is_basic_block_end_instruction(instruction);
                        let next_pc = pc + u64::from(instruction_length(instruction));
                        self.update_pc(Value::from_u64(next_pc));
                        execute(instruction, self)?;
                        for label in self.labels_to_test.drain(..) {
                            if label != next_pc && label < section_end && label >= section_start {
                                self.labels.insert(label);
                            }
                        }
                        if self.labels.len() > MAXIMUM_LABELS {
                            return Err(Error::External(
                                AotError::LimitReachedMaximumLabels.to_string(),
                            ));
                        }
                        self.pc = Value::from_u64(next_pc);
                    }
                    Err(Error::InvalidInstruction {
                        pc: _,
                        instruction: i,
                    }) if i == 0 => {
                        // Due to alignment or other reasons, the code might
                        // certain invalid instructions in the executable
                        // sections, for a normal VM instance that's executing
                        // instructions, this is usually fine since the invalid
                        // regions might never be touched. But for an AOT
                        // solution, this won't work since we have to
                        // pre-process the whole text section without knowing
                        // which part would be touched. The solution here, is
                        // to skip sections containing invalid instructions,
                        // keep a note of them, and ignore them in the code
                        // generation phase. One caveat here,
                        // is that a malicious program might choose to include
                        // invalid instructions everywhere, hence creating
                        // numerous sections hoping to bring the program down.
                        // To tackle that, we will add an upper bound on the
                        // number of dummy sections allowed here. That
                        // allow us to signal correct error and revert back
                        // to assembly VM for those quirky programs.
                        if !start_of_basic_block {
                            return Err(Error::External(
                                AotError::OutOfBoundDueToNotStartOfBasicBlock.to_string(),
                            ));
                        }
                        let mut dummy_end = pc + 2;
                        while dummy_end < section_end && self.memory.execute_load16(dummy_end)? == 0
                        {
                            dummy_end += 2;
                        }
                        // We checked no sections are overlapped, so dummy
                        // sections won't overlap with each other as well.
                        self.dummy_sections.insert(pc, dummy_end);
                        if self.dummy_sections.len() > MAXIMUM_DUMMY_SECTIONS {
                            return Err(Error::External(
                                AotError::LimitReachedMaximumDummySections.to_string(),
                            ));
                        }
                        self.pc = Value::from_u64(dummy_end);
                    }
                    Err(e) => return Err(e),
                }
            }
            // A section must end a basic block, otherwise we would run into
            // out of bound error;
            if !start_of_basic_block {
                return Err(Error::External(
                    AotError::OutOfBoundDueToNotStartOfBasicBlock.to_string(),
                ));
            }
            debug_assert!(!self.labels.contains(&section_end));
        }
        // Remove all labels pointed to dummy sections, since we won't generate
        // code for dummy sections
        for (dummy_start, dummy_end) in &self.dummy_sections {
            self.labels
                .retain(|label| *label < *dummy_start || *label >= *dummy_end);
        }
        Ok(())
    }
}

impl CoreMachine for LabelGatheringMachine {
    type REG = Value;
    type MEM = Self;

    fn pc(&self) -> &Value {
        &self.pc
    }

    fn update_pc(&mut self, pc: Self::REG) {
        self.next_pc = pc;
    }

    fn commit_pc(&mut self) {
        match self.next_pc.clone() {
            Value::Imm(pc) => self.labels_to_test.push(pc),
            Value::Cond(_, t, f) => {
                if let (Value::Imm(t), Value::Imm(f)) = (&*t, &*f) {
                    self.labels_to_test.push(*t);
                    self.labels_to_test.push(*f);
                }
            }
            _ => (),
        }
    }

    fn memory(&self) -> &Self {
        self
    }

    fn memory_mut(&mut self) -> &mut Self {
        self
    }

    fn registers(&self) -> &[Value] {
        &self.registers
    }

    fn set_register(&mut self, _idx: usize, _value: Value) {
        // This is a NOP since we only care about PC writes
    }

    fn isa(&self) -> u8 {
        self.isa
    }

    fn version(&self) -> u32 {
        self.version
    }
}

impl Machine for LabelGatheringMachine {
    fn ecall(&mut self) -> Result<(), Error> {
        // This is a basic block end instruction, main loop will record the
        // address after this instruction.
        Ok(())
    }

    fn ebreak(&mut self) -> Result<(), Error> {
        // This is a basic block end instruction, main loop will record the
        // address after this instruction.
        Ok(())
    }
}

impl Memory for LabelGatheringMachine {
    type REG = Value;

    fn init_pages(
        &mut self,
        _addr: u64,
        _size: u64,
        _flags: u8,
        _source: Option<Bytes>,
        _offset_from_addr: u64,
    ) -> Result<(), Error> {
        Err(Error::Unimplemented)
    }

    fn fetch_flag(&mut self, _page: u64) -> Result<u8, Error> {
        Err(Error::Unimplemented)
    }

    fn set_flag(&mut self, _page: u64, _flag: u8) -> Result<(), Error> {
        Err(Error::Unimplemented)
    }

    fn clear_flag(&mut self, _page: u64, _flag: u8) -> Result<(), Error> {
        Err(Error::Unimplemented)
    }

    fn store_byte(&mut self, _addr: u64, _size: u64, _value: u8) -> Result<(), Error> {
        Err(Error::Unimplemented)
    }

    fn store_bytes(&mut self, _addr: u64, _value: &[u8]) -> Result<(), Error> {
        Err(Error::Unimplemented)
    }

    fn execute_load16(&mut self, _addr: u64) -> Result<u16, Error> {
        Err(Error::Unimplemented)
    }

    fn execute_load32(&mut self, _addr: u64) -> Result<u32, Error> {
        Err(Error::Unimplemented)
    }

    fn load8(&mut self, addr: &Value) -> Result<Value, Error> {
        Ok(Value::Load(Rc::new(addr.clone()), 1))
    }

    fn load16(&mut self, addr: &Value) -> Result<Value, Error> {
        Ok(Value::Load(Rc::new(addr.clone()), 2))
    }

    fn load32(&mut self, addr: &Value) -> Result<Value, Error> {
        Ok(Value::Load(Rc::new(addr.clone()), 4))
    }

    fn load64(&mut self, addr: &Value) -> Result<Value, Error> {
        Ok(Value::Load(Rc::new(addr.clone()), 8))
    }

    fn store8(&mut self, _addr: &Value, _value: &Value) -> Result<(), Error> {
        Ok(())
    }

    fn store16(&mut self, _addr: &Value, _value: &Value) -> Result<(), Error> {
        Ok(())
    }

    fn store32(&mut self, _addr: &Value, _value: &Value) -> Result<(), Error> {
        Ok(())
    }

    fn store64(&mut self, _addr: &Value, _value: &Value) -> Result<(), Error> {
        Ok(())
    }
}

pub struct AotCode {
    pub code: Mmap,
    /// Labels that map RISC-V addresses to offsets into the compiled x86_64
    /// assembly code. This can be used as entrypoints to start executing in
    /// AOT code.
    pub labels: HashMap<u64, u32>,
}

impl AotCode {
    pub fn base_address(&self) -> u64 {
        self.code.as_ptr() as u64
    }
}

pub struct AotCompilingMachine {
    isa: u8,
    version: u32,
    registers: [Value; 32],
    pc: Value,
    next_pc: Value,
    emitter: Emitter,
    memory: FlatMemory<u64>,
    sections: Vec<(u64, u64)>,
    dummy_sections: HashMap<u64, u64>,
    addresses_to_labels: HashMap<u64, u32>,
    writes: Vec<Write>,
    next_pc_write: Option<Value>,
    instruction_cycle_func: Option<Box<InstructionCycleFunc>>,
}

impl AotCompilingMachine {
    pub fn load(
        program: &Bytes,
        instruction_cycle_func: Option<Box<InstructionCycleFunc>>,
        isa: u8,
        version: u32,
    ) -> Result<Self, Error> {
        // First we need to gather labels
        let mut label_gathering_machine = LabelGatheringMachine::load(program, isa, version)?;
        label_gathering_machine.gather()?;

        let mut labels: Vec<u64> = label_gathering_machine.labels.iter().cloned().collect();
        labels.sort_unstable();
        let addresses_to_labels = labels
            .iter()
            .enumerate()
            .map(|(i, address)| (*address, i as u32))
            .collect();

        Ok(Self {
            isa,
            version,
            registers: init_registers(),
            pc: Value::from_u64(0),
            next_pc: Value::from_u64(0),
            emitter: Emitter::new(labels.len(), version)?,
            addresses_to_labels,
            memory: label_gathering_machine.memory,
            sections: label_gathering_machine.sections,
            dummy_sections: label_gathering_machine.dummy_sections,
            writes: vec![],
            next_pc_write: None,
            instruction_cycle_func,
        })
    }

    fn read_pc(&self) -> Result<u64, Error> {
        match &self.pc {
            Value::Imm(pc) => Ok(*pc),
            _ => Err(Error::Unexpected(String::from("Unexpected value type"))),
        }
    }

    fn take_and_clear_writes(&mut self) -> Vec<Write> {
        std::mem::take(&mut self.writes)
    }

    fn emit_block(&mut self, instructions: &[Instruction]) -> Result<(), Error> {
        let mut cycles = 0;
        // A block is split into 2 parts:
        //
        // * initial_writes contains writes for all sequential operations,
        // those can be processed as normal in sequential order.
        // * last_writes contains writes generated for the last operations,
        // in case of a branch instruction, this might contains a normal
        // register write, and a PC update. To correctly handle JALR, those
        // 2 operations need to happen atomically. Hence later we can ses
        // when version 1 or above is enabled, last_writes are submitted
        // together to emit correct native code.
        let mut initial_writes = vec![];

        for instruction in instructions.iter() {
            cycles += self
                .instruction_cycle_func
                .as_ref()
                .map(|f| f(*instruction))
                .unwrap_or(0);
        }
        self.emitter.emit_add_cycles(cycles)?;

        for (i, instruction) in instructions.iter().enumerate() {
            if i == instructions.len() - 1 {
                initial_writes = self.take_and_clear_writes();
            }
            let pc = self.read_pc()?;
            let length = instruction_length(*instruction);
            if is_slowpath_instruction(*instruction) {
                self.writes.push(Write::Slowpath);
            } else {
                execute(*instruction, self)?;
            }
            self.pc = Value::from_u64(pc + u64::from(length));
        }
        let pc = self.read_pc()?;
        // Emit succeeding PC write only
        if pc >= RISCV_MAX_MEMORY as u64 {
            return Err(Error::MemOutOfBound);
        }
        self.emitter.emit(&Write::Pc {
            value: Value::Imm(pc | ADDRESS_WRITE_ONLY_FLAG),
        })?;
        for write in initial_writes {
            self.emitter.emit(&write)?;
        }
        let mut last_writes = self.take_and_clear_writes();
        if let Some(value) = self.next_pc_write.take() {
            last_writes.push(Write::Pc {
                value: self.optimize_pc_value(value)?,
            });
        }
        // Atomic writes only accept normal register writes and PC writes.
        let all_normal_writes = last_writes
            .iter()
            .all(|write| matches!(write, Write::Register { .. } | Write::Pc { .. }));
        if self.version >= VERSION1 && last_writes.len() > 1 && all_normal_writes {
            self.emitter.emit_writes(&last_writes)?;
        } else {
            for write in last_writes {
                self.emitter.emit(&write)?;
            }
        }
        Ok(())
    }

    pub fn compile(&mut self) -> Result<AotCode, Error> {
        let mut decoder = build_decoder::<u64>(self.isa(), self.version());
        let mut instructions = [Instruction::default(); MAXIMUM_INSTRUCTIONS_PER_BLOCK];
        for i in 0..self.sections.len() {
            let (section_start, section_end) = self.sections[i];
            self.pc = Value::from_u64(section_start);
            loop {
                let pc = self.read_pc()?;
                if pc >= section_end {
                    break;
                }
                if let Some(dummy_end) = self.dummy_sections.get(&pc) {
                    self.pc = Value::from_u64(*dummy_end);
                    continue;
                }
                if let Some(label) = self.addresses_to_labels.get(&pc) {
                    self.emitter.emit_label(*label)?;
                }
                let mut count = 0;
                let mut current_pc = pc;
                while count < MAXIMUM_INSTRUCTIONS_PER_BLOCK && current_pc < section_end {
                    let instruction = decoder.decode(&mut self.memory, current_pc)?;
                    instructions[count] = instruction;
                    count += 1;
                    current_pc += u64::from(instruction_length(instruction));
                    if is_basic_block_end_instruction(instruction)
                        || self.addresses_to_labels.contains_key(&current_pc)
                    {
                        break;
                    }
                }
                self.emit_block(&instructions[0..count])?;
            }
        }
        let encoded_size = self.emitter.link()?;
        let mut buffer_mut = MmapMut::map_anon(encoded_size)?;
        self.emitter.encode(&mut buffer_mut[..])?;
        let code = buffer_mut.make_exec()?;
        let mut labels = HashMap::default();
        for (address, label) in &self.addresses_to_labels {
            let offset = self.emitter.get_label_offset(*label)?;
            labels.insert(*address, offset);
        }
        Ok(AotCode { code, labels })
    }

    // This method inspects PC value, and if any immediate encoded in the PC
    // Value matches a label, we will encode the real label directly in the
    // address for fast path jumps.
    fn optimize_pc_value(&self, value: Value) -> Result<Value, Error> {
        match value {
            Value::Imm(v) => Ok(Value::Imm(self.optimize_pc(v)?)),
            Value::Cond(c, t, f) => Ok(match (&*t, &*f) {
                (Value::Imm(t), Value::Imm(f)) => Value::Cond(
                    c,
                    Rc::new(Value::Imm(self.optimize_pc(*t)?)),
                    Rc::new(Value::Imm(self.optimize_pc(*f)?)),
                ),
                _ => Value::Cond(c, t, f),
            }),
            _ => Ok(value),
        }
    }

    fn optimize_pc(&self, pc: u64) -> Result<u64, Error> {
        if pc >= RISCV_MAX_MEMORY as u64 {
            return Err(Error::MemOutOfBound);
        }
        if pc < MAXIMUM_ENCODED_ADDRESS {
            if let Some(label) = self.addresses_to_labels.get(&pc) {
                return Ok(pc | (u64::from(*label) << 32) | ADDRESS_LABEL_FLAG);
            }
        }
        Ok(pc)
    }
}

impl CoreMachine for AotCompilingMachine {
    type REG = Value;
    type MEM = Self;

    fn pc(&self) -> &Value {
        &self.pc
    }

    fn update_pc(&mut self, pc: Self::REG) {
        self.next_pc = pc;
    }

    fn commit_pc(&mut self) {
        self.next_pc_write = Some(self.next_pc.clone())
    }

    fn memory(&self) -> &Self {
        self
    }

    fn memory_mut(&mut self) -> &mut Self {
        self
    }

    fn registers(&self) -> &[Value] {
        &self.registers
    }

    fn set_register(&mut self, index: usize, value: Value) {
        self.writes.push(Write::Register { index, value });
    }

    fn isa(&self) -> u8 {
        self.isa
    }

    fn version(&self) -> u32 {
        self.version
    }
}

impl Machine for AotCompilingMachine {
    fn ecall(&mut self) -> Result<(), Error> {
        self.writes.push(Write::Ecall);
        Ok(())
    }

    fn ebreak(&mut self) -> Result<(), Error> {
        self.writes.push(Write::Ebreak);
        Ok(())
    }
}

impl Memory for AotCompilingMachine {
    type REG = Value;

    fn init_pages(
        &mut self,
        _addr: u64,
        _size: u64,
        _flags: u8,
        _source: Option<Bytes>,
        _offset_from_addr: u64,
    ) -> Result<(), Error> {
        Err(Error::Unimplemented)
    }

    fn fetch_flag(&mut self, _page: u64) -> Result<u8, Error> {
        Err(Error::Unimplemented)
    }

    fn set_flag(&mut self, _page: u64, _flag: u8) -> Result<(), Error> {
        Err(Error::Unimplemented)
    }

    fn clear_flag(&mut self, _page: u64, _flag: u8) -> Result<(), Error> {
        Err(Error::Unimplemented)
    }

    fn store_byte(&mut self, _addr: u64, _size: u64, _value: u8) -> Result<(), Error> {
        Err(Error::Unimplemented)
    }

    fn store_bytes(&mut self, _addr: u64, _value: &[u8]) -> Result<(), Error> {
        Err(Error::Unimplemented)
    }

    fn execute_load16(&mut self, _addr: u64) -> Result<u16, Error> {
        Err(Error::Unimplemented)
    }

    fn execute_load32(&mut self, _addr: u64) -> Result<u32, Error> {
        Err(Error::Unimplemented)
    }

    fn load8(&mut self, addr: &Value) -> Result<Value, Error> {
        Ok(Value::Load(Rc::new(addr.clone()), 1))
    }

    fn load16(&mut self, addr: &Value) -> Result<Value, Error> {
        Ok(Value::Load(Rc::new(addr.clone()), 2))
    }

    fn load32(&mut self, addr: &Value) -> Result<Value, Error> {
        Ok(Value::Load(Rc::new(addr.clone()), 4))
    }

    fn load64(&mut self, addr: &Value) -> Result<Value, Error> {
        Ok(Value::Load(Rc::new(addr.clone()), 8))
    }

    fn store8(&mut self, addr: &Value, value: &Value) -> Result<(), Error> {
        self.writes.push(Write::Memory {
            address: addr.clone(),
            size: 1,
            value: value.clone(),
        });
        Ok(())
    }

    fn store16(&mut self, addr: &Value, value: &Value) -> Result<(), Error> {
        self.writes.push(Write::Memory {
            address: addr.clone(),
            size: 2,
            value: value.clone(),
        });
        Ok(())
    }

    fn store32(&mut self, addr: &Value, value: &Value) -> Result<(), Error> {
        self.writes.push(Write::Memory {
            address: addr.clone(),
            size: 4,
            value: value.clone(),
        });
        Ok(())
    }

    fn store64(&mut self, addr: &Value, value: &Value) -> Result<(), Error> {
        self.writes.push(Write::Memory {
            address: addr.clone(),
            size: 8,
            value: value.clone(),
        });
        Ok(())
    }
}

pub struct AotMachine<'a> {
    pub machine: DefaultMachine<Box<AsmCoreMachine>>,
    pub aot_code: Option<&'a AotCode>,
}

impl<'a> AotMachine<'a> {
    pub fn new(
        machine: DefaultMachine<Box<AsmCoreMachine>>,
        aot_code: Option<&'a AotCode>,
    ) -> Self {
        Self { machine, aot_code }
    }

    pub fn set_max_cycles(&mut self, cycles: u64) {
        self.machine.set_cycles(cycles)
    }

    pub fn load_program(&mut self, program: &Bytes, args: &[Bytes]) -> Result<u64, Error> {
        self.machine.load_program(program, args)
    }

    pub fn run(&mut self) -> Result<i8, Error> {
        if self.machine.isa() & ISA_MOP != 0 && self.machine.version() == VERSION0 {
            return Err(Error::InvalidVersion);
        }
        let mut decoder = build_decoder::<u64>(self.machine.isa(), self.machine.version());
        self.machine.set_running(true);
        while self.machine.running() {
            if self.machine.reset_signal() {
                decoder.reset_instructions_cache();
                self.aot_code = None;
            }
            let result = if let Some(aot_code) = &self.aot_code {
                if let Some(offset) = aot_code.labels.get(self.machine.pc()) {
                    let base_address = aot_code.base_address();
                    let offset_address = base_address + u64::from(*offset);
                    let f = unsafe {
                        std::mem::transmute::<u64, fn(*mut AsmCoreMachine, u64) -> u8>(base_address)
                    };
                    f(&mut (**self.machine.inner_mut()), offset_address)
                } else {
                    unsafe { ckb_vm_x64_execute(&mut (**self.machine.inner_mut())) }
                }
            } else {
                unsafe { ckb_vm_x64_execute(&mut (**self.machine.inner_mut())) }
            };
            match result {
                RET_DECODE_TRACE => {
                    let pc = *self.machine.pc();
                    let slot = calculate_slot(pc);
                    let mut trace = Trace::default();
                    let mut current_pc = pc;
                    let mut i = 0;
                    while i < TRACE_ITEM_LENGTH {
                        let instruction = decoder.decode(self.machine.memory_mut(), current_pc)?;
                        let end_instruction = is_basic_block_end_instruction(instruction);
                        current_pc += u64::from(instruction_length(instruction));
                        trace.instructions[i] = instruction;
                        trace.cycles += self.machine.instruction_cycle_func()(instruction);
                        let opcode = extract_opcode(instruction);
                        // Here we are calculating the absolute address used in direct threading
                        // from label offsets.
                        trace.thread[i] = unsafe {
                            u64::from(
                                *(ckb_vm_asm_labels as *const u32).offset(opcode as u8 as isize),
                            ) + (ckb_vm_asm_labels as *const u32 as u64)
                        };
                        i += 1;
                        if end_instruction {
                            break;
                        }
                    }
                    trace.instructions[i] = blank_instruction(OP_CUSTOM_TRACE_END);
                    trace.thread[i] = unsafe {
                        u64::from(
                            *(ckb_vm_asm_labels as *const u32).offset(OP_CUSTOM_TRACE_END as isize),
                        ) + (ckb_vm_asm_labels as *const u32 as u64)
                    };
                    trace.address = pc;
                    trace.length = (current_pc - pc) as u8;
                    self.machine.inner_mut().traces[slot] = trace;
                }
                RET_ECALL => self.machine.ecall()?,
                RET_EBREAK => self.machine.ebreak()?,
                RET_DYNAMIC_JUMP => (),
                RET_MAX_CYCLES_EXCEEDED => return Err(Error::CyclesExceeded),
                RET_CYCLES_OVERFLOW => return Err(Error::CyclesOverflow),
                RET_OUT_OF_BOUND => return Err(Error::MemOutOfBound),
                RET_INVALID_PERMISSION => return Err(Error::MemWriteOnExecutablePage),
                RET_SLOWPATH => {
                    let pc = *self.machine.pc() - 4;
                    let instruction = decoder.decode(self.machine.memory_mut(), pc)?;
                    execute_instruction(instruction, &mut self.machine)?;
                }
                _ => return Err(Error::Asm(result)),
            }
        }
        Ok(self.machine.exit_code())
    }
}
