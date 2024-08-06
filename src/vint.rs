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
where V: VcpuTrait<Vm> 
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
pub struct VgicIntInnerMut<V: VcpuTrait<Vm>> {
    pub owner: Option<V>,
    pub in_lr: bool,
    pub lr       : u16,
    enabled  : bool,
    pub state: IrqState,
    prio     : u8,
    targets  : u8,
    cfg      : u8,

    pub in_pend: bool,
    pub in_act: bool,
}




impl<T: VcpuTrait<Vm>> VgicIntInnerMut<T> {
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

    fn owner_vm(&self) -> Arc<Vm> {
        let owner = self.owner.as_ref().unwrap();
        owner.vm().unwrap()
    }
}

impl<V: VcpuTrait<Vm> + Clone > VgicInt<V> {

    pub fn set_owner(&self, owner: V) {
        let mut vgic_int = self.inner.lock();
        vgic_int.owner = Some(owner);
    }

    pub fn owner(&self) -> Option<V> {
        let vgic_int = self.inner.lock();
        vgic_int.owner.as_ref().cloned()
    }
    
}

impl<V: VcpuTrait<Vm> > VgicInt<V> {

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
        vgic_int.owner.as_ref().map(|owner| owner.phys_id())
    }

    pub fn owner_id(&self) -> Option<usize> {
        let vgic_int = self.inner.lock();
        match &vgic_int.owner {
            Some(owner) => Some(owner.id()),
            None => {
                // warn!("owner_id is None");
                None
            }
        }
    }

    fn owner_vm_id(&self) -> Option<usize> {
        let vgic_int = self.inner.lock();
        vgic_int.owner.as_ref().map(|owner| owner.vm_id())
    }

    pub fn owner_vm(&self) -> Arc<Vm> {
        let vgic_int = self.inner.lock();
        vgic_int.owner_vm()
    }

    pub fn locked_helper<F>(&self, f: F)
    where
        F: FnOnce(&mut VgicIntInnerMut<V>),
    {
        f(&mut self.inner.lock());
    }
}

