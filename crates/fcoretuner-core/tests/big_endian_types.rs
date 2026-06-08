//! Tests for per-field big-endian types (BU08, BS16, etc.)

use libretune_core::ini::EcuDefinition;
use libretune_core::ini::Endianness;

#[test]
fn test_big_endian_type_detection() {
    let ini_content = r#"
[MegaTune]
    signature = "TestBigEndian"
    queryCommand = "Q"

[TunerStudio]
    endianness = little

[Constants]
    page = 1
    ; Regular types use global endianness (little)
    normalU16 = scalar, U16, 0, "", 1, 0, 0, 65535, 0
    normalS16 = scalar, S16, 2, "", 1, 0, 0, 32767, 0
    
    ; Big-endian override types (BU*, BS*)
    bigEndianU16 = scalar, BU16, 4, "", 1, 0, 0, 65535, 0
    bigEndianS32 = scalar, BS32, 6, "", 1, 0, 0, 2147483647, 0
    bigEndianU08 = scalar, BU08, 10, "", 1, 0, 0, 255, 0
"#;

    let def = EcuDefinition::from_str(ini_content).expect("Should parse successfully");

    // Check global endianness is little (as specified)
    assert_eq!(def.endianness, Endianness::Little);

    // Normal types should have no override
    let normal_u16 = def
        .constants
        .get("normalU16")
        .expect("normalU16 should exist");
    assert!(
        normal_u16.endianness_override.is_none(),
        "normalU16 should use global endianness"
    );

    let normal_s16 = def
        .constants
        .get("normalS16")
        .expect("normalS16 should exist");
    assert!(
        normal_s16.endianness_override.is_none(),
        "normalS16 should use global endianness"
    );

    // Big-endian types should have override
    let big_u16 = def
        .constants
        .get("bigEndianU16")
        .expect("bigEndianU16 should exist");
    assert_eq!(
        big_u16.endianness_override,
        Some(Endianness::Big),
        "BU16 should force big endian"
    );

    let big_s32 = def
        .constants
        .get("bigEndianS32")
        .expect("bigEndianS32 should exist");
    assert_eq!(
        big_s32.endianness_override,
        Some(Endianness::Big),
        "BS32 should force big endian"
    );

    let big_u08 = def
        .constants
        .get("bigEndianU08")
        .expect("bigEndianU08 should exist");
    assert_eq!(
        big_u08.endianness_override,
        Some(Endianness::Big),
        "BU08 should force big endian"
    );
}

#[test]
fn test_all_big_endian_types() {
    // Test all B* type variants
    let ini_content = r#"
[MegaTune]
    signature = "TestAllBigEndian"
    queryCommand = "Q"

[Constants]
    page = 1
    testBU08 = scalar, BU08, 0, "", 1, 0, 0, 255, 0
    testBS08 = scalar, BS08, 1, "", 1, 0, -128, 127, 0
    testBU16 = scalar, BU16, 2, "", 1, 0, 0, 65535, 0
    testBS16 = scalar, BS16, 4, "", 1, 0, -32768, 32767, 0
    testBU32 = scalar, BU32, 6, "", 1, 0, 0, 4294967295, 0
    testBS32 = scalar, BS32, 10, "", 1, 0, -2147483648, 2147483647, 0
    testBF32 = scalar, BF32, 14, "", 1, 0, -1000, 1000, 2
"#;

    let def = EcuDefinition::from_str(ini_content).expect("Should parse successfully");

    // All B* types should have big endian override
    for name in [
        "testBU08", "testBS08", "testBU16", "testBS16", "testBU32", "testBS32", "testBF32",
    ] {
        let constant = def
            .constants
            .get(name)
            .unwrap_or_else(|| panic!("{} should exist", name));
        assert_eq!(
            constant.endianness_override,
            Some(Endianness::Big),
            "{} should have big endian override",
            name
        );
    }
}

#[test]
fn test_mixed_endianness_parsing() {
    // Test that both regular and B* types parse correctly in the same file
    let ini_content = r#"
[MegaTune]
    signature = "MixedEndian"
    queryCommand = "Q"

[TunerStudio]
    endianness = big

[Constants]
    page = 1
    ; With global big endian, normal types still have no override
    normalField = scalar, U16, 0, "", 1, 0, 0, 65535, 0
    ; B* types still set the override (even if global is already big)
    explicitBig = scalar, BU16, 2, "", 1, 0, 0, 65535, 0
"#;

    let def = EcuDefinition::from_str(ini_content).expect("Should parse successfully");

    // Global is big endian
    assert_eq!(def.endianness, Endianness::Big);

    // Normal type has no override
    let normal = def.constants.get("normalField").unwrap();
    assert!(normal.endianness_override.is_none());

    // B* type has explicit override
    let explicit = def.constants.get("explicitBig").unwrap();
    assert_eq!(explicit.endianness_override, Some(Endianness::Big));
}
