use core::iter::Chain;
use core::ops::{Deref, DerefMut};
use core::slice::Iter;

use alloc::boxed::Box;
use alloc::fmt;
use core::alloc::{GlobalAlloc, Layout};

use crate::allocator;
use crate::param::*;
use crate::vm::{PhysicalAddr, VirtualAddr};
use crate::ALLOCATOR;
use crate::console::kprintln;

use aarch64::vmsa::*;
use shim::const_assert_size;

#[repr(C)]
pub struct Page([u8; PAGE_SIZE]);
const_assert_size!(Page, PAGE_SIZE);

impl Page {
    pub const SIZE: usize = PAGE_SIZE;
    pub const ALIGN: usize = PAGE_SIZE;

    fn layout() -> Layout {
        unsafe { Layout::from_size_align_unchecked(Self::SIZE, Self::ALIGN) }
    }
}

#[repr(C)]
#[repr(align(65536))]
pub struct L2PageTable {
    pub entries: [RawL2Entry; 8192],
}
const_assert_size!(L2PageTable, PAGE_SIZE);

impl L2PageTable {
    /// Returns a new `L2PageTable`
    fn new() -> L2PageTable {
        return L2PageTable {
            entries: [RawL2Entry::new(0); 8192],
        }
    }

    /// Returns a `PhysicalAddr` of the pagetable.
    pub fn as_ptr(&self) -> PhysicalAddr {
        let pointer = self as *const L2PageTable as usize;
        return PhysicalAddr::from(pointer);
    }
}

#[derive(Copy, Clone)]
pub struct L3Entry(RawL3Entry);

impl L3Entry {
    /// Returns a new `L3Entry`.
    fn new() -> L3Entry {
        return L3Entry(RawL3Entry::new(0));
    }

    /// Returns `true` if the L3Entry is valid and `false` otherwise.
    fn is_valid(&self) -> bool {
        return self.0.get_value(RawL3Entry::VALID) == 1;
    }

    /// Extracts `ADDR` field of the L3Entry and returns as a `PhysicalAddr`
    /// if valid. Otherwise, return `None`.
    fn get_page_addr(&self) -> Option<PhysicalAddr> {
        if !self.is_valid() {
            return None;
        }
    
        return Some(PhysicalAddr::from(
            self.0.get_value(RawL3Entry::ADDR)
        ));
    }
}

#[repr(C)]
#[repr(align(65536))]
pub struct L3PageTable {
    pub entries: [L3Entry; 8192],
}
const_assert_size!(L3PageTable, PAGE_SIZE);

impl L3PageTable {
    /// Returns a new `L3PageTable`.
    fn new() -> L3PageTable {
        return L3PageTable {
            entries: [L3Entry::new(); 8192],
        };
    }

    /// Returns a `PhysicalAddr` of the pagetable.
    pub fn as_ptr(&self) -> PhysicalAddr {
        let pointer = self as *const L3PageTable as usize;
        return PhysicalAddr::from(pointer);
    }
}

#[repr(C)]
#[repr(align(65536))]
pub struct PageTable {
    pub l2: L2PageTable,
    pub l3: [L3PageTable; 2],
}

impl PageTable {
    /// Returns a new `Box` containing `PageTable`.
    /// Entries in L2PageTable should be initialized properly before return.
    fn new(perm: u64) -> Box<PageTable> {
        let mut pt = Box::new(PageTable {
            l2: L2PageTable::new(),
            l3: [L3PageTable::new(), L3PageTable::new()],
        });
        
        // Initialize L2PageTable entries "properly"?
        for i in 0..2 {
            pt.l2.entries[i].set_value(0b1, RawL2Entry::VALID);
            pt.l2.entries[i].set_value(0b1, RawL2Entry::TYPE);
            pt.l2.entries[i].set_value(0b000, RawL2Entry::ATTR);
            // NS is unused
            pt.l2.entries[i].set_value(perm, RawL2Entry::AP);
            pt.l2.entries[i].set_value(0b11, RawL2Entry::SH);
            pt.l2.entries[i].set_value(0b1, RawL2Entry::AF);
            pt.l2.entries[i].set_masked(pt.l3[i].as_ptr().as_u64(), RawL2Entry::ADDR);
        }

        return pt;
    }

    /// Returns the (L2index, L3index) extracted from the given virtual address.
    /// Since we are only supporting 1GB virtual memory in this system, L2index
    /// should be smaller than 2.
    ///
    /// # Panics
    ///
    /// Panics if the virtual address is not properly aligned to page size.
    /// Panics if extracted L2index exceeds the number of L3PageTable.
    fn locate(va: VirtualAddr) -> (usize, usize) {
        let va_val = va.as_u64();
        if (va_val % PAGE_SIZE as u64 != 0) {
            panic!("Virtual address is not aligned to page size");
        }

        let l2_mask = 0b1; // 1 one
        let l2_i = (va_val & (l2_mask << 29)) >> 29;

        let l3_mask = !(!0 << 13); // 13 ones
        let l3_i = (va_val & (l3_mask << 16)) >> 16;

        return (l2_i as usize, l3_i as usize);
    }

    /// Returns `true` if the L3entry indicated by the given virtual address is valid.
    /// Otherwise, `false` is returned.
    pub fn is_valid(&self, va: VirtualAddr) -> bool {
        let (l2_i, l3_i) = PageTable::locate(va);

        return self.l3[l2_i].entries[l3_i].is_valid();
    }

    /// Returns `true` if the L3entry indicated by the given virtual address is invalid.
    /// Otherwise, `true` is returned.
    pub fn is_invalid(&self, va: VirtualAddr) -> bool {
        return !self.is_valid(va);
    }

    /// Set the given RawL3Entry `entry` to the L3Entry indicated by the given virtual
    /// address.
    pub fn set_entry(&mut self, va: VirtualAddr, entry: RawL3Entry) -> &mut Self {
        let (l2_i, l3_i) = PageTable::locate(va); 
        self.l3[l2_i].entries[l3_i].0 = entry;

        return self;
    }

    /// Returns a base address of the pagetable. The returned `PhysicalAddr` value
    /// will point the start address of the L2PageTable.
    pub fn get_baddr(&self) -> PhysicalAddr {
        return PhysicalAddr::from(&self.l2 as *const L2PageTable);
    }
}

impl<'a> IntoIterator for &'a PageTable {
    type Item = &'a L3Entry;
    type IntoIter = Chain<Iter<'a, L3Entry>, Iter<'a, L3Entry>>;

    /// Returns and iterator of the L3 page table, with indices 0 and 1 
    /// chained together
    fn into_iter(self) -> Self::IntoIter {
        return self.l3[0].entries.iter().chain(self.l3[1].entries.iter());
    }
    
}

pub struct KernPageTable(Box<PageTable>);

impl KernPageTable {
    /// Returns a new `KernPageTable`. `KernPageTable` should have a `Pagetable`
    /// created with `KERN_RW` permission.
    ///
    /// Set L3entry of ARM physical address starting at 0x00000000 for RAM and
    /// physical address range from `IO_BASE` to `IO_BASE_END` for peripherals.
    /// Each L3 entry should have correct value for lower attributes[10:0] as well
    /// as address[47:16]. Refer to the definition of `RawL3Entry` in `vmsa.rs` for
    /// more details.
    pub fn new() -> KernPageTable {
        let mut pt = PageTable::new(0b00);

        let starting_address = 0;

        let (_, ending_address) = allocator::memory_map().expect("Expected start and end address");

        let mut curr_address = starting_address;

        while curr_address <= IO_BASE_END - PAGE_SIZE {
            if curr_address <= ending_address - PAGE_SIZE || curr_address >= IO_BASE {
                let mut entry = RawL3Entry::new(0);
                entry.set_value(0b1, RawL3Entry::VALID);
                entry.set_value(0b1, RawL3Entry::TYPE);

                if curr_address <= ending_address - PAGE_SIZE {
                    entry.set_value(0b000, RawL3Entry::ATTR);
                    entry.set_value(0b11, RawL3Entry::SH);
                } else if curr_address >= IO_BASE {
                    entry.set_value(0b001, RawL3Entry::ATTR);
                    entry.set_value(0b10, RawL3Entry::SH);
                }

                entry.set_value(0b00, RawL3Entry::AP);
                entry.set_value(0b1, RawL3Entry::AF);
                entry.set_masked(curr_address as u64, RawL3Entry::ADDR);

                pt.set_entry(VirtualAddr::from(curr_address), entry);
            }
            curr_address += PAGE_SIZE;
        }

        return KernPageTable(pt);
    }
}

pub enum PagePerm {
    RW,
    RO,
    RWX,
}

pub struct UserPageTable(Box<PageTable>);

impl UserPageTable {
    /// Returns a new `UserPageTable` containing a `PageTable` created with
    /// `USER_RW` permission.
    pub fn new() -> UserPageTable {
        let pt = PageTable::new(0b01);

        return UserPageTable(pt);
    }

    /// Allocates a page and set an L3 entry translates given virtual address to the
    /// physical address of the allocated page. Returns the allocated page.
    ///
    /// # Panics
    /// Panics if the virtual address is lower than `USER_IMG_BASE`.
    /// Panics if the virtual address has already been allocated.
    /// Panics if allocator fails to allocate a page.
    ///
    /// TODO. use Result<T> and make it failurable
    /// TODO. use perm properly
    pub fn alloc(&mut self, va: VirtualAddr, _perm: PagePerm) -> &mut [u8] {
        let va_val = va.as_usize();

        if va_val < USER_IMG_BASE {
            panic!("Cannot access that memory as a user!");
        }

        let mut page;

        unsafe {
            page = ALLOCATOR.alloc(Page::layout());
        }

        if page == core::ptr::null_mut() {
            panic!("Allocating the page table failed!");
        }

        let page_address = page as u64;

        let mut entry = RawL3Entry::new(0);

        // Set attributes
        entry.set_value(0b1, RawL3Entry::VALID);
        entry.set_value(0b1, RawL3Entry::TYPE);
        entry.set_value(0b000, RawL3Entry::ATTR);
        entry.set_value(0b01, RawL3Entry::AP);
        entry.set_value(0b11, RawL3Entry::SH);
        entry.set_value(0b1, RawL3Entry::AF);
        entry.set_masked(page_address, RawL3Entry::ADDR);

        // Set entry in page table
        self.set_entry(VirtualAddr::from(va_val - USER_IMG_BASE), entry);

        unsafe {
            return core::slice::from_raw_parts_mut(page, PAGE_SIZE);
        }
    }
}

impl Deref for KernPageTable {
    type Target = PageTable;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Deref for UserPageTable {
    type Target = PageTable;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for KernPageTable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl DerefMut for UserPageTable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// FIXME: Implement `Drop` for `UserPageTable`.
impl Drop for UserPageTable {
    fn drop(&mut self) {
        for entry in self.into_iter() {
            if entry.is_valid() {
                let mut address = entry.get_page_addr().expect("Expected address");
                let physical_pointer = address.as_mut_ptr();
                unsafe {
                    ALLOCATOR.dealloc(physical_pointer, Page::layout());
                }
            }
        }
    }
}

// FIXME: Implement `fmt::Debug` as you need.
impl fmt::Debug for UserPageTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UserPageTable")
    }
}
