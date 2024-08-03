
use core::borrow::BorrowMut;

use spin::Mutex;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref VGG: Mutex<Option<VgicGlobal>> = Mutex::new(None);
}

#[derive(Clone, Copy)]
pub struct VgicGlobal {
    // GIC_LRS_NUM
    pub nr_lr:         u32,
    pub mainten_irq:   u32,
    pub max_gic_vcpus: u32,

    pub typer: u32,
    pub iidr : u32
}

impl VgicGlobal {
    pub fn new(__vgg: VgicGlobal) {
        let mut vgg = VGG.lock().unwrap();
        vgg = __vgg;
    }
}
