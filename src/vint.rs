//! This file define The Vinterrupt
//! 


extern crate alloc;
use alloc::sync::Arc;

use core::cell::Cell;
use spin::Mutex;

use crate::consts::*;
use crate::fake::*;

use crate::vgic_traits::PcpuTrait;
use crate::vgic_traits::VcpuTrait;

pub struct VgicInt<V>
    where 
    V: VcpuTrait
{
    inner_const: VgicIntInnerConst,
    inner: Mutex<VgicIntInnerMut<V>>,
    pub lock: Mutex<()>,
}

// SAFETY: VgicIntInnerConst hw is only set when initializing
unsafe impl Sync for VgicIntInnerConst {}


struct VgicIntInnerConst {
    id: u16,
    hw: Cell<bool>,
}
pub struct VgicIntInnerMut<V: VcpuTrait> {
    pub owner: Option<V>,
    pub in_lr: bool,
    pub lr   : u16,
    enabled  : bool,
    pub state: IrqState,
    prio     : u8,
    targets  : u8,
    cfg      : u8,

    pub in_pend: bool,
    pub in_act : bool,
}


impl<T: VcpuTrait> VgicIntInnerMut<T> {
    fn new() -> Self {
        Self {
            owner: None,
            in_lr: false,
            lr: 0,
            enabled: false,
            state: IrqState::IrqSInactive,
            prio: 0xff,
            targets: 0,
            cfg: 0,
            in_pend: false,
            in_act: false,
        }
    }

    fn priv_new(owner: T, targets: usize, enabled: bool) -> Self {
        Self {
            owner: Some(owner),
            in_lr: false,
            lr: 0,
            enabled,
            state: IrqState::IrqSInactive,
            prio: 0xff,
            targets: targets as u8,
            cfg: 0,
            in_pend: false,
            in_act: false,
        }
    }

    // fn owner_vm(&self) -> Arc<Vm> {
    //     let owner = self.owner.as_ref().unwrap();
    //     owner.vm().unwrap()
    // }
}

impl<V: VcpuTrait + Clone > VgicInt<V> {

    pub fn set_owner(&self, owner: V) {
        let mut vgic_int = self.inner.lock();
        vgic_int.owner = Some(owner);
    }

    pub fn owner(&self) -> Option<V> {
        let vgic_int = self.inner.lock();
        vgic_int.owner.as_ref().cloned()
    }
    
}

impl<V: VcpuTrait > VgicInt<V> {

    pub fn new(id: usize) -> Self {
        Self {
            inner_const: VgicIntInnerConst {
                id: (id + GIC_PRIVINT_NUM) as u16,
                hw: Cell::new(false),
            },
            inner: Mutex::new(VgicIntInnerMut::new()),
            lock: Mutex::new(()),
        }
    }

    pub fn priv_new(id: usize, owner: V, targets: usize, enabled: bool) -> Self {
        Self {
            inner_const: VgicIntInnerConst {
                id: id as u16,
                hw: Cell::new(false),
            },
            inner: Mutex::new(VgicIntInnerMut::priv_new(owner, targets, enabled)),
            lock: Mutex::new(()),
        }
    }

    fn set_in_pend_state(&self, is_pend: bool) {
        let mut vgic_int = self.inner.lock();
        vgic_int.in_pend = is_pend;
    }

    fn set_in_act_state(&self, is_act: bool) {
        let mut vgic_int = self.inner.lock();
        vgic_int.in_act = is_act;
    }

    pub fn in_pend(&self) -> bool {
        let vgic_int = self.inner.lock();
        vgic_int.in_pend
    }

    pub fn in_act(&self) -> bool {
        let vgic_int = self.inner.lock();
        vgic_int.in_act
    }

    pub fn set_enabled(&self, enabled: bool) {
        let mut vgic_int = self.inner.lock();
        vgic_int.enabled = enabled;
    }

    fn set_lr(&self, lr: u16) {
        let mut vgic_int = self.inner.lock();
        vgic_int.lr = lr;
    }

    pub fn set_targets(&self, targets: u8) {
        let mut vgic_int = self.inner.lock();
        vgic_int.targets = targets;
    }

    pub fn set_prio(&self, prio: u8) {
        let mut vgic_int = self.inner.lock();
        vgic_int.prio = prio;
    }

    pub fn set_in_lr(&self, in_lr: bool) {
        let mut vgic_int = self.inner.lock();
        vgic_int.in_lr = in_lr;
    }

    pub fn set_state(&self, state: IrqState) {
        let mut vgic_int = self.inner.lock();
        vgic_int.state = state;
    }

    pub fn clear_owner(&self) {
        let mut vgic_int = self.inner.lock();
        // println!("clear owner get lock");
        vgic_int.owner = None;
    }

    pub fn set_hw(&self, hw: bool) {
        self.inner_const.hw.set(hw);
    }

    pub fn set_cfg(&self, cfg: u8) {
        let mut vgic_int = self.inner.lock();
        vgic_int.cfg = cfg;
    }

    pub fn lr(&self) -> u16 {
        let vgic_int = self.inner.lock();
        vgic_int.lr
    }

    pub fn in_lr(&self) -> bool {
        let vgic_int = self.inner.lock();
        vgic_int.in_lr
    }

    #[inline]
    pub fn id(&self) -> u16 {
        self.inner_const.id
    }

    pub fn enabled(&self) -> bool {
        let vgic_int = self.inner.lock();
        vgic_int.enabled
    }

    pub fn prio(&self) -> u8 {
        let vgic_int = self.inner.lock();
        vgic_int.prio
    }

    pub fn targets(&self) -> u8 {
        let vgic_int = self.inner.lock();
        vgic_int.targets
    }

    #[inline]
    pub fn hw(&self) -> bool {
        self.inner_const.hw.get()
    }

    pub fn state(&self) -> IrqState {
        let vgic_int = self.inner.lock();
        vgic_int.state
    }

    pub fn cfg(&self) -> u8 {
        let vgic_int = self.inner.lock();
        vgic_int.cfg
    }

    pub fn owner_phys_id(&self) -> Option<usize> {
        let vgic_int = self.inner.lock();
        vgic_int.owner.as_ref().map(|owner| owner.if_phys_id())
    }

    pub fn owner_id(&self) -> Option<usize> {
        let vgic_int = self.inner.lock();
        match &vgic_int.owner {
            Some(owner) => Some(owner.if_id()),
            None => {
                // warn!("owner_id is None");
                None
            }
        }
    }

    fn owner_vm_id(&self) -> Option<usize> {
        let vgic_int = self.inner.lock();
        vgic_int.owner.as_ref().map(|owner| owner.if_vm_id())
    }

    // pub fn owner_vm(&self) -> Arc<Vm> {
    //     let vgic_int = self.inner.lock();
    //     vgic_int.owner_vm()
    // }

    pub fn locked_helper<F>(&self, f: F)
    where
        F: FnOnce(&mut VgicIntInnerMut<V>),
    {
        f(&mut self.inner.lock());
    }
}



use crate::GicHypervisorInterface;
use crate::vgic_traits::VmTrait;



// 只考虑 spi 
pub fn vgic_int_owns<V: VcpuTrait + Clone>(vcpu: &V, interrupt: &VgicInt<V>) -> bool {
    // sgi ppi 
    if gic_is_priv(interrupt.id() as usize) {
        return true;
    }

    let vcpu_id = vcpu.if_id();
    let pcpu_id = vcpu.if_phys_id();
    match interrupt.owner() {
        Some(owner) => { 
            let owner_vcpu_id = owner.if_id();
            let owner_pcpu_id = owner.if_phys_id();
            owner_vcpu_id == vcpu_id && owner_pcpu_id == pcpu_id
        }
        None => false,
    }
}

// vcpu_id, pcpu_id
pub fn vgic_int_yield_owner<V: VcpuTrait + Clone>(vcpu: &V, interrupt: &VgicInt<V>) {
    if !vgic_int_owns(vcpu, interrupt) || interrupt.in_lr() || gic_is_priv(interrupt.id() as usize) {
        return;
    }

    if vgic_get_state(interrupt) & 2 == 0 {
        interrupt.clear_owner();
    }
}

/// 1、这个int没有owner的话，设置当前vcpu为他的主人  返回真
/// 2、这个int有owner，返回 owner_vm_id == vcpu_vm_id && owner_vcpu_id == vcpu_id 
pub fn vgic_int_get_owner<V: VcpuTrait + Clone>(vcpu: &V, interrupt: &VgicInt<V>) -> bool {
    let vcpu_id = vcpu.if_id();
    let vcpu_vm_id = vcpu.if_vm_id();

    match interrupt.owner() {
        Some(owner) => {
            let owner_vcpu_id = owner.if_id();
            let owner_vm_id = owner.if_vm_id();

            owner_vm_id == vcpu_vm_id && owner_vcpu_id == vcpu_id
        }
        None => {
            interrupt.set_owner(vcpu.clone());
            true
        }
    }
}

pub fn vgic_get_state<V: VcpuTrait + Clone>(interrupt: &VgicInt<V>) -> usize {
    let mut state = interrupt.state().to_num();

    if interrupt.in_lr() && interrupt.owner_phys_id().unwrap() == current_cpu().id() {
        let lr_option = gich_get_lr(interrupt);
        if let Some(lr_val) = lr_option {
            state = lr_val as usize;
        }
    }

    if interrupt.id() as usize >= GIC_SGIS_NUM {
        return state;
    }
    if interrupt.owner().is_none() {
        return state;
    }

    /*
    let vm = interrupt.owner_vm();
    let vgic = vm.get_vgic();
    let vcpu_id = interrupt.owner_id().unwrap();

    if vgic.cpu_priv_sgis_pend(vcpu_id, interrupt.id() as usize) != 0 {
        state |= 1;
    }
    */

    state
}

pub fn gich_get_lr<V: VcpuTrait>(interrupt: &VgicInt<V>) -> Option<u32> {
    let cpu_id = current_cpu().id();
    let phys_id = interrupt.owner_phys_id().unwrap();

    if !interrupt.in_lr() || phys_id != cpu_id {
        return None;
    }

    let lr_val = GicHypervisorInterface::lr(interrupt.lr() as usize);
    if (lr_val & 0b1111111111 == interrupt.id() as u32) && (lr_val >> 28 & 0b11 != 0) {
        return Some(lr_val);
    }
    None
}