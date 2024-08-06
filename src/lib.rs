
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
mod vgic_ipi;
mod vgic_traits;
mod utils;
mod consts;
mod vgic_state;

mod fake;

use fake::*;
use gich::*;

extern crate alloc;
use alloc::vec::Vec;
use vgic::Vgic;

use crate::config::VgicGlobal;
use arm_gic::gic_v2::GicDistributor;
use crate::vgic_maintence::gic_maintenance_handler;
// use crate::vgic::vgic_set_hw_int;
// use crate::vgic::emu_intc_init;
use crate::vint::*;
use crate::consts::*;
use alloc::sync::Arc;

use crate::vgic_traits::VcpuTrait;
use crate::vgic_traits::VmTrait;

use lazy_static::lazy_static;
use axsync::Mutex;
lazy_static! {
    pub static ref VM0: Mutex<Vm> = Mutex::new(Vm::new(0));
    pub static ref VM1: Mutex<Vm> = Mutex::new(Vm::new(1));
}
use alloc::sync::Weak;


// init intc for a vm
pub fn emu_intc_init(base_ipa: usize, length: usize, vcpu_list: &[Vcpu]) -> Result<Arc<Vgic<Vcpu>>, ()> {

    let vcpu_num = vcpu_list.len();
    let mut vgic = Vgic::new(base_ipa, length, vcpu_num);

    for i in 0..GIC_SPI_MAX {
        vgic.vgicd.interrupts.push(VgicInt::new(i));
    }

    for vcpu in vcpu_list {
        let mut cpu_priv = vint_private::VgicCpuPriv::default();
        for int_idx in 0..GIC_PRIVINT_NUM {
            
            let phys_id = vcpu.phys_id();

            cpu_priv.interrupts.push(VgicInt::priv_new(
                int_idx,
                vcpu.clone(),
                1 << phys_id,
                int_idx < GIC_SGIS_NUM,
            ));
        }

        vgic.cpu_priv.push(cpu_priv);
    }

    Ok(Arc::new(vgic))
}

pub fn vgic_set_hw_int(vm: &Vm, int_id: usize) {
    // soft
    if int_id < GIC_SGIS_NUM {
        return;
    }

    let vgic = vm.get_vgic();

    // ppi
    if int_id < GIC_PRIVINT_NUM {
        for i in 0..vm.vcpu_list().len() {
            if let Some(interrupt) = vgic.get_int(&vm.vcpu(i).unwrap(), int_id) {
                let interrupt_lock = interrupt.lock.lock();
                interrupt.set_hw(true);
                drop(interrupt_lock);
            }
        }
    // spi
    } else if let Some(interrupt) = vgic.get_int(&vm.vcpu(0).unwrap(), int_id) {
        let interrupt_lock = interrupt.lock.lock();
        interrupt.set_hw(true);
        drop(interrupt_lock);
    }
}



// static STATE: AtomicUsize = AtomicUsize::new(0);
pub fn vgic_init(gich_base: * mut u8) 
{
    GicHypervisorInterface::init_base(gich_base);
    GicHypervisorInterface::init();
    
    let vgg = VgicGlobal {
        nr_lr: 1,
        typer: GicDistributor::get_typer(),
        iidr: GicDistributor::get_iidr(),
        mainten_irq: 32,
        max_gic_vcpus: 32,
    };
    VgicGlobal::new(vgg);

    let mut vcpu = Vec::new();
    vcpu.push(Vcpu{id:0, phys_id:0, vm_id:0, vm: Weak::new()});
    let vgic_dev = emu_intc_init(1, 1, &vcpu).unwrap();


    VM0.lock().vcpu_list = vcpu.clone();
    VM0.lock().emu_devs.push(vgic_dev);

    let emu_ctx = EmuContext{address:0, width:0, write:true, sign_ext:true, reg:0, reg_width:4};
    VM0.lock().get_vgic().handler(&emu_ctx, &vcpu[0]);


    vgic_set_hw_int(&VM0.lock(), 64);


    gic_maintenance_handler();
}




// 引用外部
/*
1、arm_gic  GICC GICD GICH
2、percpu   得到当前pcpu
*/

// 对外接口
/*
1、外部告知vgic：vm中的 vcpu，vcpu的 pcpu的id， 以及活跃的vm
2、state 保存恢复接口3
3、ipi 通信接口
*/