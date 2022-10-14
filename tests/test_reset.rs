use ckb_vm::machine::asm::{AsmCoreMachine, AsmMachine};
use ckb_vm::machine::{DefaultMachineBuilder, VERSION1};
use ckb_vm::Bytes;
use ckb_vm::{
    registers::A7, Error, Register, SupportMachine, Syscalls, DEFAULT_STACK_SIZE, ISA_IMC, ISA_MOP,
    RISCV_MAX_MEMORY,
};
use ckb_vm_aot::AotCompilingMachine;

mod machine_build;

pub struct CustomSyscall {}

impl<Mac: SupportMachine> Syscalls<Mac> for CustomSyscall {
    fn initialize(&mut self, _: &mut Mac) -> Result<(), Error> {
        Ok(())
    }

    fn ecall(&mut self, machine: &mut Mac) -> Result<bool, Error> {
        let code = &machine.registers()[A7];
        if code.to_i32() != 1111 {
            return Ok(false);
        }
        let cycles = machine.cycles();
        machine.reset(machine.max_cycles());
        machine.set_cycles(cycles);
        let code_data = std::fs::read("tests/programs/reset_callee").unwrap();
        let code = Bytes::from(code_data);
        machine.load_elf(&code, true).unwrap();
        machine.initialize_stack(
            &[],
            (RISCV_MAX_MEMORY - DEFAULT_STACK_SIZE) as u64,
            DEFAULT_STACK_SIZE as u64,
        )?;
        Ok(true)
    }
}

#[test]
pub fn test_reset_aot() {
    let code_data = std::fs::read("tests/programs/reset_caller").unwrap();
    let code = Bytes::from(code_data);

    let mut aot_machine = AotCompilingMachine::load(
        &code,
        Some(Box::new(machine_build::instruction_cycle_func)),
        ISA_IMC | ISA_MOP,
        VERSION1,
    )
    .unwrap();
    let code = aot_machine.compile().unwrap();

    let buffer: Bytes = std::fs::read("tests/programs/reset_caller").unwrap().into();

    let asm_core = AsmCoreMachine::new(ISA_IMC | ISA_MOP, VERSION1, u64::max_value());
    let core = DefaultMachineBuilder::<Box<AsmCoreMachine>>::new(asm_core)
        .instruction_cycle_func(&machine_build::instruction_cycle_func)
        .syscall(Box::new(CustomSyscall {}))
        .build();
    let mut machine = AsmMachine::new(core, Some(&code));
    machine.load_program(&buffer, &vec![]).unwrap();

    let result = machine.run();
    let cycles = machine.machine.cycles();
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0);
    assert_eq!(cycles, 775);
}
