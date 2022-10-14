use ckb_vm::machine::asm::{AotCode, AsmCoreMachine, AsmMachine};
use ckb_vm::machine::VERSION1;
use ckb_vm::{Bytes, DefaultMachineBuilder, Instruction, ISA_B, ISA_IMC, ISA_MOP};
use ckb_vm_aot::AotCompilingMachine;

pub fn instruction_cycle_func(_: Instruction) -> u64 {
    1
}

#[allow(dead_code)]
pub fn aot_v1_imcb_code(path: &str) -> AotCode {
    let buffer: Bytes = std::fs::read(path).unwrap().into();
    let mut aot_machine = AotCompilingMachine::load(
        &buffer,
        Some(Box::new(instruction_cycle_func)),
        ISA_IMC | ISA_B,
        VERSION1,
    )
    .unwrap();
    aot_machine.compile().unwrap()
}

#[allow(dead_code)]
pub fn aot_v1_imcb<'a>(path: &str, code: &'a AotCode) -> AsmMachine<'a> {
    let buffer: Bytes = std::fs::read(path).unwrap().into();

    let asm_core = AsmCoreMachine::new(ISA_IMC | ISA_B, VERSION1, u64::max_value());
    let core = DefaultMachineBuilder::<Box<AsmCoreMachine>>::new(asm_core)
        .instruction_cycle_func(&instruction_cycle_func)
        .build();
    let mut machine = AsmMachine::new(core, Some(code));
    machine
        .load_program(&buffer, &vec![Bytes::from("main")])
        .unwrap();
    machine
}

#[allow(dead_code)]
pub fn aot_v1_mop_code(path: &str) -> AotCode {
    let buffer: Bytes = std::fs::read(path).unwrap().into();
    let mut aot_machine = AotCompilingMachine::load(
        &buffer,
        Some(Box::new(instruction_cycle_func)),
        ISA_IMC | ISA_B | ISA_MOP,
        VERSION1,
    )
    .unwrap();
    aot_machine.compile().unwrap()
}

#[allow(dead_code)]
pub fn aot_v1_mop<'a>(path: &str, args: Vec<Bytes>, code: &'a AotCode) -> AsmMachine<'a> {
    let buffer: Bytes = std::fs::read(path).unwrap().into();

    let asm_core = AsmCoreMachine::new(ISA_IMC | ISA_B | ISA_MOP, VERSION1, u64::max_value());
    let core = DefaultMachineBuilder::<Box<AsmCoreMachine>>::new(asm_core)
        .instruction_cycle_func(&instruction_cycle_func)
        .build();
    let mut argv = vec![Bytes::from("main")];
    argv.extend_from_slice(&args);
    let mut machine = AsmMachine::new(core, Some(code));
    machine.load_program(&buffer, &argv).unwrap();
    machine
}
