use libretune_core::ini::{DataType, EcuDefinition, Endianness, OutputChannel};
use libretune_core::realtime::evaluator::Evaluator;

#[test]
fn test_evaluator_dependency_chain() {
    let mut def = EcuDefinition::default();
    // Default endianness is Little
    def.endianness = Endianness::Little;

    // Raw channel: RPM @ offset 0, U16
    let mut rpm = OutputChannel::new("rpm", DataType::U16, 0);
    rpm.scale = 1.0;
    def.output_channels.insert("rpm".to_string(), rpm);

    // Computed channel: rpm_x2 = { rpm * 2 }
    let mut rpm_x2 = OutputChannel::new("rpm_x2", DataType::F32, 0);
    rpm_x2.expression = Some("rpm * 2".to_string());
    rpm_x2.cache_expression();
    def.output_channels.insert("rpm_x2".to_string(), rpm_x2);

    // Computed channel dependent on computed: rpm_x4 = { rpm_x2 * 2 }
    let mut rpm_x4 = OutputChannel::new("rpm_x4", DataType::F32, 0);
    rpm_x4.expression = Some("rpm_x2 * 2".to_string());
    rpm_x4.cache_expression();
    def.output_channels.insert("rpm_x4".to_string(), rpm_x4);

    let eval = Evaluator::new(&def);

    // 1000 decimal = 0x03E8
    // Little endian: E8 03
    let raw_data = vec![0xE8, 0x03, 0x00, 0x00];

    // Process
    let result = eval.process(&raw_data, &def);

    assert_eq!(*result.get("rpm").unwrap(), 1000.0);
    assert_eq!(*result.get("rpm_x2").unwrap(), 2000.0);
    assert_eq!(*result.get("rpm_x4").unwrap(), 4000.0);
}

#[test]
fn test_evaluator_ternary() {
    let mut def = EcuDefinition::default();

    // Raw channel: Flag @ offset 0, U08
    let flag = OutputChannel::new("flag", DataType::U08, 0);
    def.output_channels.insert("flag".to_string(), flag);

    // Computed: status = { flag > 0 ? 100 : 0 }
    let mut status = OutputChannel::new("status", DataType::F32, 0);
    status.expression = Some("flag > 0 ? 100 : 0".to_string());
    status.cache_expression();
    def.output_channels.insert("status".to_string(), status);

    let eval = Evaluator::new(&def);

    // Case 1: Flag = 1
    let raw_data_1 = vec![0x01];
    let result_1 = eval.process(&raw_data_1, &def);
    assert_eq!(*result_1.get("status").unwrap(), 100.0);

    // Case 0: Flag = 0
    let raw_data_0 = vec![0x00];
    let result_0 = eval.process(&raw_data_0, &def);
    assert_eq!(*result_0.get("status").unwrap(), 0.0);
}
