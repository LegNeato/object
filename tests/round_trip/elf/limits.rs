use super::*;
use object::{RelocationFlags, SectionFlags};

fn new_object(architecture: Architecture) -> write::Object<'static> {
    write::Object::new(BinaryFormat::Elf, architecture, Endianness::Little)
}

fn symbol(name: &[u8], value: u64, size: u64) -> write::Symbol {
    write::Symbol {
        name: name.to_vec(),
        value,
        size,
        kind: SymbolKind::Data,
        scope: SymbolScope::Dynamic,
        weak: false,
        section: write::SymbolSection::Absolute,
        flags: SymbolFlags::None,
    }
}

#[derive(Default)]
struct Output {
    reserved: Option<u64>,
    written: u64,
}

impl write::WritableBuffer for Output {
    fn reserve(&mut self, size: u64) -> Result<(), ()> {
        assert!(self.reserved.replace(size).is_none());
        Ok(())
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        self.written += bytes.len() as u64;
    }

    fn write_zeros(&mut self, size: u64) {
        self.written += size;
    }
}

fn rejected(object: &write::Object<'_>, field: &str, value: &str) {
    let mut output = Output::default();
    let error = object.emit(&mut output).unwrap_err().to_string();
    assert!(error.contains(field), "{error}");
    assert!(error.contains(value), "{error}");
    assert_eq!(output.reserved, None);
    assert_eq!(output.written, 0);
    let mut stream = Vec::new();
    assert!(object.write_stream(&mut stream).is_err());
    assert!(stream.is_empty());
}

#[test]
fn bss_sections() {
    let maximum = u64::from(u32::MAX);
    for (sizes, alignment, valid) in [
        (vec![0], 1, true),
        (vec![maximum], 1, true),
        (vec![maximum + 1], 1, false),
        (vec![1 << 31, 1 << 31], 1, false),
        (vec![(1 << 31) - 1, 1 << 31], 8, false),
    ] {
        let mut object = new_object(Architecture::Xtensa);
        let section = object.section_id(write::StandardSection::UninitializedData);
        for (index, size) in sizes.iter().copied().enumerate() {
            let symbol = object.add_symbol(symbol(format!("data{index}").as_bytes(), 0, 0));
            object.add_symbol_bss(symbol, section, size, alignment);
        }
        if valid {
            let bytes = object.write().unwrap();
            let file = read::File::parse(&*bytes).unwrap();
            assert_eq!(file.section_by_name(".bss").unwrap().size(), sizes[0]);
            assert_eq!(file.symbol_by_name("data0").unwrap().size(), sizes[0]);
        } else {
            rejected(&object, "section size", "0x100000000");
        }
    }
}

#[test]
fn separate_bss_sections() {
    let mut object = new_object(Architecture::Xtensa);
    for name in [b"first", b"second" as &[u8]] {
        let section = object.add_section(Vec::new(), name.to_vec(), SectionKind::UninitializedData);
        object.append_section_bss(section, 1 << 31, 1);
    }
    let bytes = object.write().unwrap();
    let file = read::File::parse(&*bytes).unwrap();
    for name in ["first", "second"] {
        assert_eq!(file.section_by_name(name).unwrap().size(), 1 << 31);
    }
}

#[test]
fn section_alignment_and_flags() {
    let mut aligned = new_object(Architecture::Xtensa);
    let section = aligned.section_id(write::StandardSection::UninitializedData);
    aligned.append_section_bss(section, 0, 1 << 32);
    rejected(&aligned, "section alignment", "0x100000000");

    let mut flagged = new_object(Architecture::Xtensa);
    let section = flagged.section_id(write::StandardSection::UninitializedData);
    *flagged.section_flags_mut(section) = SectionFlags::Elf {
        sh_type: elf::SHT_NOBITS,
        sh_flags: elf::SectionFlags(1 << 32),
    };
    rejected(&flagged, "section flags", "0x100000000");
    if let SectionFlags::Elf { sh_flags, .. } = flagged.section_flags_mut(section) {
        *sh_flags = elf::SectionFlags(u64::from(u32::MAX));
    }
    let bytes = flagged.write().unwrap();
    let file = read::File::parse(&*bytes).unwrap();
    assert!(matches!(
        file.section_by_name(".bss").unwrap().flags(),
        SectionFlags::Elf { sh_flags, .. } if sh_flags.0 == u64::from(u32::MAX)
    ));
}

#[test]
fn symbol_fields_and_mutations() {
    let maximum = u64::from(u32::MAX);
    let mut object = new_object(Architecture::Xtensa);
    let id = object.add_symbol(symbol(b"absolute", maximum, maximum));
    let bytes = object.write().unwrap();
    let file = read::File::parse(&*bytes).unwrap();
    let parsed = file.symbol_by_name("absolute").unwrap();
    assert_eq!((parsed.address(), parsed.size()), (maximum, maximum));
    object.symbol_mut(id).value = maximum + 1;
    rejected(&object, "symbol value", "0x100000000");
    object.symbol_mut(id).value = 0;
    object.symbol_mut(id).size = maximum + 1;
    rejected(&object, "symbol size", "0x100000000");
    object.symbol_mut(id).size = 0;
    assert!(object.write().is_ok());
}

fn relocated(
    architecture: Architecture,
    offset: u64,
    r_type: u32,
    addend: i64,
) -> write::Object<'static> {
    let mut object = new_object(architecture);
    let section = object.section_id(write::StandardSection::Data);
    object.append_section_data(section, &[0; 8], 1);
    let id = object.add_symbol(symbol(b"target", 0, 0));
    object
        .add_relocation(
            section,
            write::Relocation {
                offset,
                symbol: id,
                addend,
                flags: RelocationFlags::Elf {
                    r_type: elf::RelocationType(r_type),
                },
            },
        )
        .unwrap();
    object
}

#[test]
fn relocation_fields() {
    for (offset, r_type, addend, field, value) in [
        (1 << 32, 1, 0, "relocation offset", "0x100000000"),
        (0, 256, 0, "relocation type", "0x100"),
        (
            0,
            1,
            i64::from(i32::MIN) - 1,
            "relocation addend",
            "-2147483649",
        ),
        (
            0,
            1,
            i64::from(i32::MAX) + 1,
            "relocation addend",
            "2147483648",
        ),
    ] {
        rejected(
            &relocated(Architecture::Xtensa, offset, r_type, addend),
            field,
            value,
        );
    }
    for addend in [i64::from(i32::MIN), 0, i64::from(i32::MAX)] {
        let object = relocated(Architecture::Xtensa, u64::from(u32::MAX), 255, addend);
        let bytes = object.write().unwrap();
        let file = read::File::parse(&*bytes).unwrap();
        let section = file.section_by_name(".data").unwrap();
        let (offset, relocation) = section.relocations().next().unwrap();
        assert_eq!(offset, u64::from(u32::MAX));
        assert_eq!(relocation.addend(), addend);
        assert!(matches!(relocation.flags(), RelocationFlags::Elf { r_type } if r_type.0 == 255));
    }
}

#[test]
fn implicit_addends_keep_their_existing_storage() {
    let object = relocated(Architecture::I386, 0, elf::R_386_32.0, i64::from(u32::MAX));
    let bytes = object.write().unwrap();
    let file = read::File::parse(&*bytes).unwrap();
    let section = file.section_by_name(".data").unwrap();
    assert_eq!(&section.data().unwrap()[..4], &u32::MAX.to_le_bytes());
    let (_, relocation) = section.relocations().next().unwrap();
    assert!(relocation.has_implicit_addend());
}

#[test]
fn relocation_indices_follow_symbol_order() {
    let mut object = relocated(Architecture::Xtensa, 0, elf::R_XTENSA_32.0, 0);
    let mut local = symbol(b"local", 0, 0);
    local.scope = SymbolScope::Compilation;
    object.add_symbol(local);
    let bytes = object.write().unwrap();
    let file = read::File::parse(&*bytes).unwrap();
    let section = file.section_by_name(".data").unwrap();
    let (_, relocation) = section.relocations().next().unwrap();
    let read::RelocationTarget::Symbol(index) = relocation.target() else {
        panic!("expected a symbol relocation");
    };
    assert_eq!(index.0, 2);
    assert_eq!(
        file.symbol_by_index(index).unwrap().name().unwrap(),
        "target"
    );
}

#[test]
fn elf64_fields_remain_wide() {
    let mut object = relocated(Architecture::X86_64, 1 << 32, 256, i64::MAX);
    let section = object.section_id(write::StandardSection::UninitializedData);
    let id = object.add_symbol(symbol(b"large", 1 << 32, 1 << 32));
    object.add_symbol_bss(id, section, 1 << 32, 1);
    object.symbol_mut(id).value = 1 << 32;
    let bytes = object.write().unwrap();
    let file = read::File::parse(&*bytes).unwrap();
    assert_eq!(file.section_by_name(".bss").unwrap().size(), 1 << 32);
    let parsed = file.symbol_by_name("large").unwrap();
    assert_eq!((parsed.address(), parsed.size()), (1 << 32, 1 << 32));
    let section = file.section_by_name(".data").unwrap();
    let (offset, relocation) = section.relocations().next().unwrap();
    assert_eq!(offset, 1 << 32);
    assert_eq!(relocation.addend(), i64::MAX);
    assert!(matches!(relocation.flags(), RelocationFlags::Elf { r_type } if r_type.0 == 256));

    let mut object = new_object(Architecture::X86_64);
    let section = object.section_id(write::StandardSection::UninitializedData);
    object.append_section_bss(section, 0, 1 << 32);
    *object.section_flags_mut(section) = SectionFlags::Elf {
        sh_type: elf::SHT_NOBITS,
        sh_flags: elf::SectionFlags(1 << 32),
    };
    let mut output = Output::default();
    object.emit(&mut output).unwrap();
    assert_eq!(output.reserved, Some(output.written));
    assert!(output.written > 1 << 32);
}
