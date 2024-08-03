

use tock_registers::interfaces::{Readable, Writeable};
use tock_registers::register_structs;
use tock_registers::registers::{ReadOnly, ReadWrite};
use core::ptr::NonNull;

use crate::consts::*;
use crate::utils::device_ref::*;

register_structs! {
    #[allow(non_snake_case)]
    pub GicHypervisorInterface {
        (0x0000 => HCR: ReadWrite<u32>),
        (0x0004 => VTR: ReadOnly<u32>),
        (0x0008 => VMCR: ReadWrite<u32>),
        (0x000c => reserve0),
        (0x0010 => MISR: ReadOnly<u32>),
        (0x0014 => reserve1),
        (0x0020 => EISR: [ReadOnly<u32>; GIC_LIST_REGS_NUM / 32]),
        (0x0028 => reserve2),
        (0x0030 => ELRSR: [ReadOnly<u32>; GIC_LIST_REGS_NUM / 32]),
        (0x0038 => reserve3),
        (0x00f0 => APR: ReadWrite<u32>),
        (0x00f4 => reserve4),
        (0x0100 => LR: [ReadWrite<u32>; GIC_LIST_REGS_NUM]),
        (0x0200 => reserve5),
        (0x1000 => @END),
    }
}

unsafe impl Sync for GicHypervisorInterface {}

static mut GICH: DeviceRef<GicHypervisorInterface> = unsafe { DeviceRef::new() };


impl GicHypervisorInterface {


    pub fn init_base(base: *mut u8) {
        unsafe { GICH.dev_init(base as * const GicHypervisorInterface); }
    }

    pub fn init() {
        for i in 0..Self::gich_lrs_num() {
            Self::set_lr(i, 0)
        }
    
        let hcr_prev = Self::hcr();
        Self::set_hcr(hcr_prev | GICH_HCR_LRENPIE_BIT as u32);
    }

    pub fn gich_lrs_num() -> usize {
        let mut vtr;
        unsafe { vtr = GICH.VTR.get(); }
        ((vtr & 0b11111) + 1) as usize
    }

    pub fn hcr() -> u32 {
        unsafe{ GICH.HCR.get() }
    }

    pub fn set_hcr(hcr: u32) {
        unsafe{ GICH.HCR.set(hcr); }
    }

    pub fn elrsr(elsr_idx: usize) -> u32 {
        unsafe{ GICH.ELRSR[elsr_idx].get() }
    }

    pub fn eisr(eisr_idx: usize) -> u32 {
        unsafe{ GICH.EISR[eisr_idx].get() }
    }

    pub fn lr(lr_idx: usize) -> u32 {
        unsafe{ GICH.LR[lr_idx].get() }
    }

    pub fn misr() -> u32 {
        unsafe{ GICH.MISR.get() }
    }

    pub fn apr() -> u32 {
        unsafe{ GICH.APR.get() }
    }

    pub fn set_apr(val: u32) {
        unsafe{ GICH.APR.set(val) }
    }

    pub fn set_lr(lr_idx: usize, val: u32) {
        unsafe{ GICH.LR[lr_idx].set(val) }
    }
}

