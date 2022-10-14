pub mod machine_build;

#[test]
pub fn test_clzw_bug() {
    let code = machine_build::aot_v1_imcb_code("tests/programs/clzw_bug");
    let mut machine_aot = machine_build::aot_v1_imcb("tests/programs/clzw_bug", &code);
    let ret_aot = machine_aot.run();
    assert!(ret_aot.is_ok());
    assert_eq!(ret_aot.unwrap(), 0);
}

#[test]
pub fn test_sbinvi_aot_load_imm_bug() {
    let code = machine_build::aot_v1_imcb_code("tests/programs/sbinvi_aot_load_imm_bug");
    let mut machine_aot =
        machine_build::aot_v1_imcb("tests/programs/sbinvi_aot_load_imm_bug", &code);
    let ret_aot = machine_aot.run();
    assert!(ret_aot.is_ok());
    assert_eq!(ret_aot.unwrap(), 0);
}

#[test]
pub fn test_rorw_in_end_of_aot_block() {
    let code = machine_build::aot_v1_imcb_code("tests/programs/rorw_in_end_of_aot_block");
    let mut machine_aot =
        machine_build::aot_v1_imcb("tests/programs/rorw_in_end_of_aot_block", &code);
    let ret_aot = machine_aot.run();
    assert!(ret_aot.is_ok());
    assert_eq!(ret_aot.unwrap(), 0);
}

#[test]
pub fn test_pcnt() {
    let code = machine_build::aot_v1_imcb_code("tests/programs/pcnt");
    let mut machine_aot = machine_build::aot_v1_imcb("tests/programs/pcnt", &code);
    let ret_aot = machine_aot.run();
    assert!(ret_aot.is_ok());
    assert_eq!(ret_aot.unwrap(), 0);
}

#[test]
pub fn test_clmul_bug() {
    let code = machine_build::aot_v1_imcb_code("tests/programs/clmul_bug");
    let mut machine_aot = machine_build::aot_v1_imcb("tests/programs/clmul_bug", &code);
    let ret_aot = machine_aot.run();
    assert!(ret_aot.is_ok());
    assert_eq!(ret_aot.unwrap(), 0);
}

#[test]
pub fn test_orc_bug() {
    let code = machine_build::aot_v1_imcb_code("tests/programs/orc_bug");
    let mut machine_aot = machine_build::aot_v1_imcb("tests/programs/orc_bug", &code);
    let ret_aot = machine_aot.run();
    assert!(ret_aot.is_ok());
    assert_eq!(ret_aot.unwrap(), 0);
}
