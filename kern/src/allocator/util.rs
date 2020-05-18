/// Align `addr` downwards to the nearest multiple of `align`.
///
/// The returned usize is always <= `addr.`
///
/// # Panics
///
/// Panics if `align` is not a power of 2.
pub fn align_down(addr: usize, align: usize) -> usize {
    if align & align - 1 != 0 {
        panic!("Not a power of 2");
    }

    return addr - (addr % align);
}

/// Align `addr` upwards to the nearest multiple of `align`.
///
/// The returned `usize` is always >= `addr.`
///
/// # Panics
///
/// Panics if `align` is not a power of 2
/// or aligning up overflows the address.
pub fn align_up(addr: usize, align: usize) -> usize {
    if align & align - 1 != 0 {
        panic!("Not a power of 2");
    }

    if addr % align == 0 {
        return addr;
    }

    match addr.checked_add(align - (addr % align)) {
        Some(res) => res,
        None => panic!("Aligning overflowed")
    }
}

/// Checks if address is aligned to given alignment
pub fn is_aligned(addr: usize, align: usize) -> bool {
    return addr % align == 0;
}
