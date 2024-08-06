

use crate::vint::*;
extern crate alloc;
use alloc::vec::Vec;
use alloc::collections::VecDeque;

use core::ptr::NonNull;
use core::cell::RefCell;
use crate::consts::*;

use crate::fake::*;

#[derive(Clone, Copy, Default)]
pub struct Sgis {
    pub pend: u8,
    pub act : u8,
}
use crate::VcpuTrait;

pub struct VgicCpuPriv<V>
where
    V: VcpuTrait<Vm>,
{
    pub interrupts: Vec<VgicInt<V>>,
    pub inner_mut: RefCell<VgicCpuPrivMut<V>>,
}

pub struct VgicCpuPrivMut <V: VcpuTrait<Vm>> {
    pub curr_lrs: [u16; GIC_LIST_REGS_NUM],
    pub sgis: [Sgis; GIC_SGIS_NUM],

    pub pend_list: VecDeque<NonNull<VgicInt<V>>>,
    pub act_list: VecDeque<NonNull<VgicInt<V>>>,
}

impl <V: VcpuTrait<Vm>> VgicCpuPrivMut <V> {
    fn queue_remove(list: &mut VecDeque<NonNull<VgicInt<V>>>, interrupt: &VgicInt<V>) {
        // SAFETY: All VgicInt are allocated when initializing, so it's safe to convert them to NonNull
        list.iter()
            .position(|x| unsafe { x.as_ref().id() } == interrupt.id())
            .map(|i| list.remove(i));
    }

    pub fn pend_list_push(&mut self, interrupt: &VgicInt<V>) {
        // SAFETY: All VgicInt are allocated when initializing, so it's safe to convert them to NonNull
        self.pend_list
            .push_back(unsafe { NonNull::new_unchecked(interrupt as *const _ as *mut _) });
    }

    pub fn pend_list_remove(&mut self, interrupt: &VgicInt<V>) {
        Self::queue_remove(&mut self.pend_list, interrupt);
    }

    pub fn act_list_push(&mut self, interrupt: &VgicInt<V>) {
        // SAFETY: All VgicInt are allocated when initializing, so it's safe to convert them to NonNull
        self.act_list
            .push_back(unsafe { NonNull::new_unchecked(interrupt as *const _ as *mut _) });
    }

    pub fn act_list_remove(&mut self, interrupt: &VgicInt<V>) {
        Self::queue_remove(&mut self.act_list, interrupt);
    }
}

// SAFETY: VgicCpuPriv is only accessed on one core
unsafe impl Send for VgicCpuPriv <Vcpu> {}
unsafe impl Sync for VgicCpuPriv <Vcpu> {}

impl<V: VcpuTrait<Vm>> VgicCpuPriv<V> {
    pub fn default() -> VgicCpuPriv<V> {
        VgicCpuPriv {
            interrupts: Vec::new(),
            inner_mut: RefCell::new(
                VgicCpuPrivMut {
                curr_lrs: [0; GIC_LIST_REGS_NUM],
                sgis: [Sgis::default(); GIC_SGIS_NUM],
                pend_list: VecDeque::new(),
                act_list: VecDeque::new(),
            }),
        }
    }
}
