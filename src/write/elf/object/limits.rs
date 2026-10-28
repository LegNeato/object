use crate::write::{Error, Result};

fn unsigned(field: &str, value: u64, maximum: u64) -> Result<()> {
    if value > maximum {
        return Err(Error(format!(
            "ELF32 {field} {value:#x} exceeds {maximum:#x}"
        )));
    }
    Ok(())
}

pub(super) fn section(size: u64, alignment: u64, flags: u64) -> Result<()> {
    let maximum = u64::from(u32::MAX);
    unsigned("section size", size, maximum)?;
    unsigned("section alignment", alignment, maximum)?;
    unsigned("section flags", flags, maximum)
}

pub(super) fn symbol(value: u64, size: u64) -> Result<()> {
    let maximum = u64::from(u32::MAX);
    unsigned("symbol value", value, maximum)?;
    unsigned("symbol size", size, maximum)
}

pub(super) fn relocation(offset: u64, r_type: u32, is_rela: bool, addend: i64) -> Result<()> {
    unsigned("relocation offset", offset, u64::from(u32::MAX))?;
    unsigned("relocation type", u64::from(r_type), u64::from(u8::MAX))?;
    if is_rela && i32::try_from(addend).is_err() {
        return Err(Error(format!(
            "ELF32 relocation addend {addend} is outside {}..={}",
            i32::MIN,
            i32::MAX
        )));
    }
    Ok(())
}

pub(super) fn relocation_symbol(index: u32) -> Result<()> {
    // ELF32 r_info reserves its low byte for the relocation type.
    unsigned(
        "relocation symbol index",
        u64::from(index),
        u64::from(u32::MAX >> u8::BITS),
    )
}

#[cfg(test)]
mod tests;
