pub mod machine_build;

use ckb_vm::Bytes;
use ckb_vm::SupportMachine;

#[test]
#[cfg_attr(miri, ignore)]
pub fn test_mop_wide_multiply() {
    let code = machine_build::aot_v1_mop_code("tests/programs/mop_wide_multiply");
    let mut machine_aot =
        machine_build::aot_v1_mop("tests/programs/mop_wide_multiply", vec![], &code);
    let ret_aot = machine_aot.run();
    assert!(ret_aot.is_ok());
    assert_eq!(ret_aot.unwrap(), 0);
    assert_eq!(machine_aot.machine.cycles(), 9192427);
}

#[test]
#[cfg_attr(miri, ignore)]
pub fn test_mop_wide_divide() {
    let code = machine_build::aot_v1_mop_code("tests/programs/mop_wide_divide");
    let mut machine_aot =
        machine_build::aot_v1_mop("tests/programs/mop_wide_divide", vec![], &code);
    let ret_aot = machine_aot.run();
    assert!(ret_aot.is_ok());
    assert_eq!(ret_aot.unwrap(), 0);
    assert_eq!(machine_aot.machine.cycles(), 6106583);
}

#[test]
pub fn test_mop_far_jump() {
    let code = machine_build::aot_v1_mop_code("tests/programs/mop_far_jump");
    let mut machine_aot = machine_build::aot_v1_mop("tests/programs/mop_far_jump", vec![], &code);
    let ret_aot = machine_aot.run();
    assert!(ret_aot.is_ok());
    assert_eq!(ret_aot.unwrap(), 0);
    assert_eq!(machine_aot.machine.cycles(), 5);
}

#[test]
pub fn test_mop_secp256k1() {
    let args = vec![
        Bytes::from("033f8cf9c4d51a33206a6c1c6b27d2cc5129daa19dbd1fc148d395284f6b26411f"),
        Bytes::from("304402203679d909f43f073c7c1dcf8468a485090589079ee834e6eed92fea9b09b06a2402201e46f1075afa18f306715e7db87493e7b7e779569aa13c64ab3d09980b3560a3"),
        Bytes::from("foo"),
        Bytes::from("bar"),
    ];
    let code = machine_build::aot_v1_mop_code("benches/data/secp256k1_bench");
    let mut machine_aot =
        machine_build::aot_v1_mop("benches/data/secp256k1_bench", args.clone(), &code);
    let ret_aot = machine_aot.run();
    assert!(ret_aot.is_ok());
    assert_eq!(ret_aot.unwrap(), 0);
    assert_eq!(machine_aot.machine.cycles(), 611871);
}

#[test]
pub fn test_mop_adc() {
    let code = machine_build::aot_v1_mop_code("tests/programs/mop_adc");
    let mut machine_aot = machine_build::aot_v1_mop("tests/programs/mop_adc", vec![], &code);
    let ret_aot = machine_aot.run();
    assert!(ret_aot.is_ok());
    assert_eq!(ret_aot.unwrap(), 0);
    assert_eq!(machine_aot.machine.cycles(), 61);
}

#[test]
pub fn test_mop_sbb() {
    let code = machine_build::aot_v1_mop_code("tests/programs/mop_sbb");
    let mut machine_aot = machine_build::aot_v1_mop("tests/programs/mop_sbb", vec![], &code);
    let ret_aot = machine_aot.run();
    assert!(ret_aot.is_ok());
    assert_eq!(ret_aot.unwrap(), 0);
    assert_eq!(machine_aot.machine.cycles(), 27);
}

#[test]
pub fn test_mop_random_adc_sbb() {
    let code = machine_build::aot_v1_mop_code("tests/programs/mop_random_adc_sbb");
    let mut machine_aot =
        machine_build::aot_v1_mop("tests/programs/mop_random_adc_sbb", vec![], &code);
    let ret_aot = machine_aot.run();
    assert!(ret_aot.is_ok());
    assert_eq!(ret_aot.unwrap(), 0);
    assert_eq!(machine_aot.machine.cycles(), 6755);
}

#[test]
pub fn test_mop_ld_signextend_32_overflow_bug() {
    let code = machine_build::aot_v1_mop_code("tests/programs/mop_ld_signextend_32_overflow_bug");
    let mut machine_aot = machine_build::aot_v1_mop(
        "tests/programs/mop_ld_signextend_32_overflow_bug",
        vec![],
        &code,
    );
    let ret_aot = machine_aot.run();
    assert!(ret_aot.is_ok());
    assert_eq!(ret_aot.unwrap(), 0);
}

#[test]
pub fn test_mop_wide_mul_zero() {
    let code = machine_build::aot_v1_mop_code("tests/programs/mop_wide_mul_zero");
    let mut machine_aot =
        machine_build::aot_v1_mop("tests/programs/mop_wide_mul_zero", vec![], &code);
    let ret_aot = machine_aot.run();
    assert!(ret_aot.is_ok());
    assert_eq!(ret_aot.unwrap(), 0);
}

#[test]
pub fn test_mop_wide_div_zero() {
    let code = machine_build::aot_v1_mop_code("tests/programs/mop_wide_div_zero");
    let mut machine_aot =
        machine_build::aot_v1_mop("tests/programs/mop_wide_div_zero", vec![], &code);
    let ret_aot = machine_aot.run();
    assert!(ret_aot.is_ok());
    assert_eq!(ret_aot.unwrap(), 0);
}
