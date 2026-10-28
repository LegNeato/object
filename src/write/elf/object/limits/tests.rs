use super::*;
use alloc::string::ToString;

#[test]
fn unsigned_fields() {
    let maximum = u64::from(u32::MAX);
    assert!(section(maximum, maximum, maximum).is_ok());
    assert!(symbol(maximum, maximum).is_ok());
    for (size, alignment, flags, field) in [
        (maximum + 1, 1, 0, "section size"),
        (0, maximum + 1, 0, "section alignment"),
        (0, 1, maximum + 1, "section flags"),
    ] {
        let error = section(size, alignment, flags).unwrap_err();
        assert!(error.0.contains(field));
        assert!(error.0.contains("0x100000000"));
    }
    for (value, size, field) in [
        (maximum + 1, 0, "symbol value"),
        (0, maximum + 1, "symbol size"),
    ] {
        let error = symbol(value, size).unwrap_err();
        assert!(error.0.contains(field));
        assert!(error.0.contains("0x100000000"));
    }
}

#[test]
fn relocation_fields() {
    let maximum = u64::from(u32::MAX);
    for addend in [i64::from(i32::MIN), 0, i64::from(i32::MAX)] {
        assert!(relocation(maximum, u32::from(u8::MAX), true, addend).is_ok());
    }
    assert!(relocation(maximum + 1, 0, true, 0).is_err());
    assert!(relocation(0, u32::from(u8::MAX) + 1, true, 0).is_err());
    for addend in [
        i64::MIN,
        i64::from(i32::MIN) - 1,
        i64::from(i32::MAX) + 1,
        i64::MAX,
    ] {
        let error = relocation(0, 0, true, addend).unwrap_err();
        assert!(error.0.contains(&addend.to_string()));
        assert!(relocation(0, 0, false, addend).is_ok());
    }
}

#[test]
fn relocation_symbol_indices() {
    let maximum = u32::MAX >> u8::BITS;
    for index in [0, 1, maximum - 1, maximum] {
        assert!(relocation_symbol(index).is_ok());
    }
    for index in [maximum + 1, u32::MAX] {
        let error = relocation_symbol(index).unwrap_err();
        assert!(error.0.contains(&format!("{index:#x}")));
    }
}
