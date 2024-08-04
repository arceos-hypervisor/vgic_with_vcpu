
#![no_std]
#![feature(const_ptr_as_ref)]
#![feature(const_option)]
#![feature(const_nonnull_new)]

mod config;
mod gich;
mod vint;
mod vint_private;
mod vgic;
mod vgic_maintence;
mod vgic_reg_access;
mod utils;
mod consts;
mod vgic_state;

mod fake;

use fake::*;
use gich::*;

extern crate alloc;
use alloc::vec::Vec;

use crate::config::VgicGlobal;
use arm_gic::gic_v2::GicDistributor;


// static STATE: AtomicUsize = AtomicUsize::new(0);

pub fn vgic_init(gich_base: * mut u8) 
{
    GicHypervisorInterface::init_base(gich_base);
    GicHypervisorInterface::init();
    use crate::vgic::emu_intc_init;
    use crate::vgic::vgic_set_hw_int;
    let vgg = VgicGlobal {
        nr_lr: 1,
        typer: GicDistributor::get_typer(),
        iidr: GicDistributor::get_iidr(),
        mainten_irq: 32,
        max_gic_vcpus: 32,
    };
    VgicGlobal::new(vgg);
    let mut vcpu = Vec::new();
    emu_intc_init(1, 1, &vcpu).unwrap();

    let vm = Vm {
        id: 0,
        vcpu_list: vcpu,
    };
    vgic_set_hw_int(&vm, 64);
}