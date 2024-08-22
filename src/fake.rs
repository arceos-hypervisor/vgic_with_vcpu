
// use serde::{
//     de::{self, Visitor},
//     Deserialize, Deserializer, Serialize,
// };
use core::{fmt::Error, ops::Range};
extern crate alloc;
use alloc::sync::{Arc, Weak};

#[derive(Copy, Clone, Debug)]
pub enum IrqState {
    IrqSInactive,
    IrqSPend,
    IrqSActive,
    IrqSPendActive,
}

impl IrqState {
    pub fn num_to_state(num: usize) -> IrqState {
        match num {
            0 => IrqState::IrqSInactive,
            1 => IrqState::IrqSPend,
            2 => IrqState::IrqSActive,
            3 => IrqState::IrqSPendActive,
            _ => panic!("num_to_state: illegal irq state"),
        }
    }

    pub fn to_num(self) -> usize {
        match self {
            IrqState::IrqSInactive => 0,
            IrqState::IrqSPend => 1,
            IrqState::IrqSActive => 2,
            IrqState::IrqSPendActive => 3,
        }
    }
}



/* ============================================================================ */
/* ============================================================================ */
/* ========== VM =========== */
/* ============================================================================ */
/* ============================================================================ */

use alloc::vec::Vec;
use crate::vgic::Vgic;
use crate::vgic_traits::*;

#[derive(Clone)]
pub struct Vm  {
    pub id: usize,
    pub vcpu_list: Vec<Vcpu>,
    pub emu_devs: Vec<Arc<Vgic<Vcpu>>>,
}

// unsafe impl Sync for Vm {}
// unsafe impl Send for Vm {}

impl Vm {
    pub fn new(id: usize) -> Self{
        Vm { id: id, vcpu_list: Vec::new(), emu_devs: Vec::new() }
    }

    /* 下面四个函数 targetr 和 sgi 要用 */
}

/* 实现trait */
impl VmTrait<Vcpu> for Vm {

    fn if_id(&self) -> usize { self.id }
    // fn if_vcpu_list(&self) -> &[Vcpu] { &self.vcpu_list }
    // fn if_vcpu(&self, id :usize) -> Option<&Vcpu> {
    //     match self.if_vcpu_list().get(id) {
    //         Some(vcpu) => {
    //             assert_eq!(id, vcpu.id());
    //             Some(vcpu)
    //         }
    //         None => {
    //             log::error!(
    //                 "vcpu idx {} is to large than vcpu_list len {}",
    //                 id,
    //                 self.if_vcpu_list().len()
    //             );
    //             None
    //         }
    //     }
    // }
    
    fn if_has_interrupt(&self, _id: usize) -> bool {true}
    fn if_emu_has_interrupt(&self, _id: usize) -> bool {true}
    // fn get_vgic(&self) -> &Vgic<Vcpu> {  &self.emu_devs[0] }
    // pub fn cpu_num(&self) -> usize { self.vcpu_list.len() }
}

/* ============================================================================ */
/* ============================================================================ */
/* ================= VCPU ================ */
/* ============================================================================ */
/* ============================================================================ */

#[derive(Clone, Debug)] pub struct Vcpu {
    pub id      : usize,
    pub phys_id : usize,
    pub vm_id   : usize,

    /* ipi use  */
    /* VgicInt::owner_vm use */
    pub vm      : Weak<Vm>,
}

/* 实现trait */
impl VcpuTrait for  Vcpu {
    // fn vm(&self) -> Option<Arc<Vm>> { self.vm.upgrade() }
    
    fn if_id(&self) -> usize { self.id }
    fn if_vm_id(&self) ->usize { self.vm_id }
    fn if_phys_id(&self) ->usize { self.phys_id }

    fn if_get_gpr(&self, idx: usize) -> usize {0}
    fn if_set_gpr(&mut self, idx: usize, val: usize) {}
}

/* ============================================================================ */
/* ============================================================================ */
/* ========= Current cpu (pcpu) ============ */
/* ============================================================================ */
/* ============================================================================ */
#[derive(Clone)]
pub struct VcpuArray {
    array: [Option<Vcpu>; 64],
    len: usize,
}

impl VcpuArray {
    pub const fn new() -> Self {
        Self {
            array: [const { None }; 64],
            len: 0,
        }
    }

    #[inline]
    pub fn pop_vcpu_through_vmid(&self, vm_id: usize) -> Option<&Vcpu> {
        match self.array.get(vm_id) {
            Some(vcpu) => vcpu.as_ref(),
            None => None,
        }
    }
}

#[derive(Clone)]
pub struct Pcpu  {
    /* ipi */
    /* maintence */
    /* xxx_access */

    pub active_vcpu  : Option<Vcpu>,
    
    /* only ipi use */
    pub vcpu_array   : VcpuArray,
}

pub fn current_cpu() -> Pcpu {
    Pcpu {
        active_vcpu: None,
        vcpu_array: VcpuArray::new(),
    }
}


/* 实现trait */
impl PcpuTrait<Vcpu>  for Pcpu {
    fn id(&self) -> usize { 0 }
}

/* nothing */
pub fn active_vm_id() -> usize {
    let vm = active_vm().unwrap();
    vm.if_id()
}
/* nothing */
pub fn active_vm() -> Option<Arc<Vm>> {
    None
    // match current_cpu().active_vcpu.as_ref() {
    //     None => None,
    //     Some(active_vcpu) => active_vcpu.vm(),
    // } 
}


/* only ipi emu_sgiregs_access use */
pub fn active_vcpu_id() -> usize {0}
/* only ipi emu_sgiregs_access use */
pub fn active_vm_ncpu() -> usize {0}


/* ============================================================================ */
/* ============================================================================ */
/* ============================================================================ */
/*   =============================================  Nothing =================   */
/* ============================================================================ */
/* ============================================================================ */
/* ============================================================================ */


#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EmuDeviceType {
    // Variants representing different emulator device types.
    EmuDeviceTConsole = 0,
    EmuDeviceTGicd = 1,
}


/* ================ IPI relevant =============== */

#[derive(Copy, Clone, Debug)] pub enum InitcEvent {
    VgicdGichEn,
    VgicdSetEn,
    VgicdSetAct,
    VgicdSetPend,
    VgicdSetPrio,
    VgicdSetTrgt,
    VgicdSetCfg,
    VgicdRoute,
    Vgicdinject,
    None,
}

#[derive(Copy, Clone)] pub struct IpiInitcMessage {
    pub event: InitcEvent,
    pub vm_id: usize,
    pub int_id: u16,
    pub val: u8,
}

#[derive(Clone)] pub enum IpiInnerMsg {
    Initc(IpiInitcMessage),
    None,
}

pub enum IpiType {None}

pub struct IpiMessage {
    pub ipi_type: IpiType,
    pub ipi_message: IpiInnerMsg,
}

pub use emu_dev::EmuContext;

