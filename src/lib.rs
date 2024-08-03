
#![no_std]
#![feature(const_ptr_as_ref)]
#![feature(const_option)]
#![feature(const_nonnull_new)]

mod config;
mod gich;
mod vint;
mod vint_private;
mod vigc;
mod utils;
mod consts;
mod vgic_state;

mod fake;

use gich::*;



// static STATE: AtomicUsize = AtomicUsize::new(0);

fn vgic_init(gich_base: * mut u8) 
{
    GicHypervisorInterface::init_base(gich_base);
    GicHypervisorInterface::init();
}