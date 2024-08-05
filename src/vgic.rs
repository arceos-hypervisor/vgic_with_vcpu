
/* I want to move this to arm_vgic crate */
// use crate::GicHypervisorInterface::

/* Some config, just use axconfig or umhv Vmconfig */
// use crate::config::VmEmulatedDeviceConfig;
// use crate::board::{Platform, PlatOperation};

/* Encapsulated data abort info */
// use crate::device::EmuContext;
// use crate::device::{EmuDev, EmuDeviceType};


/* "current_cpu"                             use percpu(crate) */
/* "active_vcpu_id"                          used only in emu_sgiregs_access() */
/* "restore_vcpu_gic" and "vgic_ipi_handler" used only in  vgic_ipi_handler() */
// use crate::kernel::{active_vcpu_id, current_cpu, restore_vcpu_gic, save_vcpu_gic};

/* current_cpu -> current_vcpu -> active_vm */
/* "active_vm_ncpu" used in emu_sgiregs_access, return vcpu num */
// use crate::kernel::{active_vm, active_vm_id, active_vm_ncpu};



/* these funcs need gic or other moudle do */
// use crate::kernel::{IpiInitcMessage, IpiInnerMsg, IpiMessage, IpiType, InitcEvent, ipi_intra_broadcast_msg, ipi_send_msg,};

/* I want use id ... as possible */
// use crate::kernel::{Vcpu, Vm};

/* we just use some types and 'static GICD'
 * 'static GICD' shouldn't use here (like GICD.setenable ...)
 * The related functions should be implemented by the GIC
 * */
// use super::gic::*;


/* ========================== */
/*  rewriting is as follows:  */
/* ========================== */


extern crate alloc;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};
use core::ops::Range;

use crate::utils::{bit_extract, bit_get, bit_set};
use core::sync::atomic::AtomicUsize;
use crate::consts::*;
use crate::vint_private;
use crate::GicHypervisorInterface;
use crate::vint::*;
use crate::config;

use crate::fake::*;
use arm_gic::gic_v2::GicDistributor;

use crate::vgic_traits::PcpuTrait;
use crate::vgic_traits::VcpuTrait;


pub struct Vgicd {
    // ctlr will be written among different cores, so we use AtomicU32 to guarantee thread safety
    ctlr        : AtomicU32,
    // Others will be read only and only be written when initializings
    typer       : u32,
    iidr        : u32,
    pub interrupts  : Vec<VgicInt>,
}

impl Vgicd {
    fn new(cpu_num: usize) -> Vgicd {
        let vgg = config::VGG.lock().unwrap();
        Vgicd {
            ctlr: AtomicU32::new(0b10),
            typer: (vgg.typer & GICD_TYPER_CPUNUM_MSK as u32)  |
                   (((cpu_num - 1) << GICD_TYPER_CPUNUM_OFF) & GICD_TYPER_CPUNUM_MSK) as u32,
            iidr:  vgg.iidr,
            interrupts: Vec::new(),
        }
    }
}



pub struct Vgic {
    pub address_range: Range<usize>,
    pub vgicd: Vgicd,
    pub cpu_priv: Vec<vint_private::VgicCpuPriv>,  // 0..32
}

impl Vgic {
    pub fn new(base: usize, length: usize, cpu_num: usize) -> Vgic {
        Vgic {
            address_range: base..base + length,
            vgicd: Vgicd::new(cpu_num),
            cpu_priv: Vec::new(),
        }
    }

    // vcpu_id
    // 操作vcpu的cpu_priv的 pend list 和 act list
    pub fn update_int_list(&self, vcpu_id: usize, interrupt: &VgicInt) {
        // let vcpu_id = vcpu.id();
        // Every vcpu has its own cpu_priv, so we can use vcpu.id() to index cpu_priv
        let mut cpu_priv = self.cpu_priv[vcpu_id].inner_mut.borrow_mut();

        interrupt.locked_helper(|int| {
            let state = int.state.to_num();

            if state & IrqState::IrqSPend.to_num() != 0 && !int.in_pend {
                cpu_priv.pend_list_push(interrupt);
                int.in_pend = true;
            } else if state & IrqState::IrqSPend.to_num() == 0 {
                cpu_priv.pend_list_remove(interrupt);
                int.in_pend = false;
            }

            if state & IrqState::IrqSActive.to_num() != 0 && !int.in_act {
                cpu_priv.act_list_push(interrupt);
                int.in_act = true;
            } else if state & IrqState::IrqSActive.to_num() == 0 {
                cpu_priv.act_list_remove(interrupt);
                int.in_act = false;
            }
        });
    }

    pub fn set_vgicd_ctlr(&self, ctlr: u32) {
        self.vgicd.ctlr.store(ctlr, Ordering::Relaxed);
    }

    pub fn vgicd_ctlr(&self) -> u32 {
        self.vgicd.ctlr.load(Ordering::Relaxed)
    }

    pub fn vgicd_typer(&self) -> u32 {
        self.vgicd.typer
    }

    pub fn vgicd_iidr(&self) -> u32 {
        self.vgicd.iidr
    }

    fn cpu_priv_interrupt(&self, cpu_id: usize, idx: usize) -> Option<&VgicInt> {
        self.cpu_priv[cpu_id].interrupts.get(idx)
    }

    fn cpu_priv_curr_lrs(&self, cpu_id: usize, idx: usize) -> u16 {
        let cpu_priv = self.cpu_priv[cpu_id].inner_mut.borrow();
        cpu_priv.curr_lrs[idx]
    }

    fn cpu_priv_sgis_pend(&self, cpu_id: usize, idx: usize) -> u8 {
        let cpu_priv = self.cpu_priv[cpu_id].inner_mut.borrow();
        cpu_priv.sgis[idx].pend
    }

    fn cpu_priv_sgis_act(&self, cpu_id: usize, idx: usize) -> u8 {
        let cpu_priv = self.cpu_priv[cpu_id].inner_mut.borrow();
        cpu_priv.sgis[idx].act
    }

    fn set_cpu_priv_curr_lrs(&self, cpu_id: usize, idx: usize, val: u16) {
        let mut cpu_priv = self.cpu_priv[cpu_id].inner_mut.borrow_mut();
        cpu_priv.curr_lrs[idx] = val;
    }

    fn set_cpu_priv_sgis_pend(&self, cpu_id: usize, idx: usize, pend: u8) {
        let mut cpu_priv = self.cpu_priv[cpu_id].inner_mut.borrow_mut();
        cpu_priv.sgis[idx].pend = pend;
    }

    fn set_cpu_priv_sgis_act(&self, cpu_id: usize, idx: usize, act: u8) {
        let mut cpu_priv = self.cpu_priv[cpu_id].inner_mut.borrow_mut();
        cpu_priv.sgis[idx].act = act;
    }

    fn vgicd_interrupt(&self, idx: usize) -> Option<&VgicInt> {
        self.vgicd.interrupts.get(idx)
    }

}


impl Vgic {
    /// can use vcpu_id
    pub fn get_int<T: VcpuTrait<Vm>>(&self, vcpu: &T, int_id: usize) -> Option<&VgicInt> {
        if int_id < GIC_PRIVINT_NUM {
            let vcpu_id = vcpu.id();
            self.cpu_priv_interrupt(vcpu_id, int_id)
        } else if (GIC_PRIVINT_NUM..GIC_INTS_MAX).contains(&int_id) {
            self.vgicd_interrupt(int_id - GIC_PRIVINT_NUM)
        } else {
            None
        }
    }

    // vcpu_id
    fn remove_lr<T: VcpuTrait<Vm>>(&self, vcpu: &T, interrupt: &VgicInt) -> bool {
        if !vgic_owns(vcpu, interrupt) { // 查看中断是否属于该 vcpu 
            return false;
        }
        let int_lr = interrupt.lr();
        let int_id = interrupt.id() as usize;
        let vcpu_id = vcpu.id();

        if !interrupt.in_lr() {  // 不在lr中返回false
            return false;
        }

        let mut lr_val = 0;
        if let Some(lr) = gich_get_lr(interrupt) {
            GicHypervisorInterface::set_lr(int_lr as usize, 0);
            lr_val = lr;
        }

        interrupt.set_in_lr(false);

        let lr_state = (lr_val >> 28) & 0b11;
        if lr_state != 0 {
            interrupt.set_state(IrqState::num_to_state(lr_state as usize));
            if int_id < GIC_SGIS_NUM {
                if interrupt.state().to_num() & 2 != 0 {
                    self.set_cpu_priv_sgis_act(vcpu_id, int_id, ((lr_val >> 10) & 0b111) as u8);
                } else if interrupt.state().to_num() & 1 != 0 {
                    let pend = self.cpu_priv_sgis_pend(vcpu_id, int_id);
                    self.set_cpu_priv_sgis_pend(vcpu_id, int_id, pend | (1 << ((lr_val >> 10) & 0b111) as u8));
                }
            }

            self.update_int_list(vcpu.id(), interrupt);

            if (interrupt.state().to_num() & 1 != 0) && interrupt.enabled() {
                let hcr = GicHypervisorInterface::hcr();
                /*
                NPIE, bit [3]
                No Pending Interrupt Enable. Enables the signaling of a maintenance interrupt while no pending interrupts are present in the List registers:

                NPIE	Meaning
                0
                Maintenance interrupt disabled.

                1
                Maintenance interrupt signaled while the List registers contain no interrupts in the pending state.

                When this register has an architecturally-defined reset value, this field resets to 0.
                */
                GicHypervisorInterface::set_hcr(hcr | (1 << 3));
            }
            return true;
        }
        false
    }

    pub fn add_lr<T: VcpuTrait<Vm>>(&self, vcpu: &T, interrupt: &VgicInt) -> bool {
        if !interrupt.enabled() || interrupt.in_lr() {
            return false;
        }

        let gic_lrs = gic_lrs();
        let mut lr_ind = None;

        for i in 0..gic_lrs {
            if (GicHypervisorInterface::elrsr(i / 32) & (1 << (i % 32))) != 0 {
                lr_ind = Some(i);
                break;
            }
        }

        if lr_ind.is_none() {
            let mut pend_found = 0;
            let mut act_found = 0;
            let mut min_prio_act = 0;
            let mut min_prio_pend = 0;
            let mut act_ind = None;
            let mut pend_ind = None;

            for i in 0..gic_lrs {
                let lr = GicHypervisorInterface::lr(i);
                let lr_prio = (lr >> 23) & 0b11111;
                let lr_state = (lr >> 28) & 0b11;

                if lr_state & 2 != 0 {
                    if lr_prio > min_prio_act {
                        min_prio_act = lr_prio;
                        act_ind = Some(i);
                    }
                    act_found += 1;
                } else if lr_state & 1 != 0 {
                    if lr_prio > min_prio_pend {
                        min_prio_pend = lr_prio;
                        pend_ind = Some(i);
                    }
                    pend_found += 1;
                }
            }

            if pend_found > 1 {
                lr_ind = pend_ind;
            } else if act_found > 1 {
                lr_ind = act_ind;
            }

            if let Some(idx) = lr_ind {
                let spilled_int = self.get_int(vcpu, GicHypervisorInterface::lr(idx) as usize & 0b1111111111).unwrap();
                if spilled_int.id() != interrupt.id() {
                    let spilled_int_lock = spilled_int.lock.lock();
                    self.remove_lr(vcpu, spilled_int);
                    vgic_int_yield_owner(vcpu, spilled_int);
                    drop(spilled_int_lock);
                } else {
                    self.remove_lr(vcpu, spilled_int);
                    vgic_int_yield_owner(vcpu, spilled_int);
                }
            }
        }

        match lr_ind {
            Some(idx) => {
                self.write_lr(vcpu, interrupt, idx);
                return true;
            }
            None => {
                // turn on maintenance interrupts
                if vgic_get_state(interrupt) & 1 != 0 {
                    let hcr = GicHypervisorInterface::hcr();
                    GicHypervisorInterface::set_hcr(hcr | (1 << 3));
                }
            }
        }

        false
    }

    pub fn write_lr<T: VcpuTrait<Vm>>(&self, vcpu: &T, interrupt: &VgicInt, lr_ind: usize) {
        let vcpu_id = vcpu.id();
        let int_id = interrupt.id() as usize;
        let int_prio = interrupt.prio();

        let prev_int_id = self.cpu_priv_curr_lrs(vcpu_id, lr_ind) as usize;
        if prev_int_id != int_id {
            if let Some(prev_interrupt) = self.get_int(vcpu, prev_int_id) {
                let prev_interrupt_lock = prev_interrupt.lock.lock();
                if vgic_owns(vcpu, prev_interrupt) && prev_interrupt.in_lr() && prev_interrupt.lr() == lr_ind as u16 {
                    prev_interrupt.set_in_lr(false);
                    let prev_id = prev_interrupt.id() as usize;
                    if !gic_is_priv(prev_id) {
                        vgic_int_yield_owner(vcpu, prev_interrupt);
                    }
                }
                drop(prev_interrupt_lock);
            }
        }

        let state = vgic_get_state(interrupt);
        let mut lr = (int_id & 0b1111111111) | (((int_prio as usize >> 3) & 0b11111) << 23);

        if vgic_int_is_hw(interrupt) {
            lr |= 1 << 31;
            lr |= (0b1111111111 & int_id) << 10;
            if state == 3 {
                lr |= (2 & 0b11) << 28;
            } else {
                lr |= (state & 0b11) << 28;
            }
            if GicDistributor::state(int_id) != 2 {
                GicDistributor::set_state(int_id, 2);
            }
        } else if int_id < GIC_SGIS_NUM {
            if (state & 2) != 0 {
                lr |= ((self.cpu_priv_sgis_act(vcpu_id, int_id) as usize) << 10) & (0b111 << 10);
                lr |= (2 & 0b11) << 28;
            } else {
                let mut idx = GIC_TARGETS_MAX - 1;
                while idx as isize >= 0 {
                    if (self.cpu_priv_sgis_pend(vcpu_id, int_id) & (1 << idx)) != 0 {
                        lr |= (idx & 0b111) << 10;
                        let pend = self.cpu_priv_sgis_pend(vcpu_id, int_id);
                        self.set_cpu_priv_sgis_pend(vcpu_id, int_id, pend & !(1 << idx));

                        lr |= (1 & 0b11) << 28;
                        break;
                    }
                    idx -= 1;
                }
            }

            if self.cpu_priv_sgis_pend(vcpu_id, int_id) != 0 {
                lr |= 1 << 19;
            }
        } else {
            if !gic_is_priv(int_id) && !vgic_int_is_hw(interrupt) {
                lr |= 1 << 19;
            }

            lr |= (state & 0b11) << 28;
        }

        interrupt.locked_helper(|int| {
            int.state = IrqState::IrqSInactive;
            int.in_lr = true;
            int.lr = lr_ind as u16;
        });
        self.set_cpu_priv_curr_lrs(vcpu_id, lr_ind, int_id as u16);
        GicHypervisorInterface::set_lr(lr_ind, lr as u32);

        self.update_int_list(vcpu.id(), interrupt);
    }

    fn route<T: VcpuTrait<Vm>>(&self, vcpu: &T, interrupt: &VgicInt) {
        let cpu_id = current_cpu().id();
        if let IrqState::IrqSInactive = interrupt.state() {
            return;
        }

        if !interrupt.enabled() {
            return;
        }

        let int_targets = interrupt.targets();
        if (int_targets & (1 << cpu_id)) != 0 {
            self.add_lr(vcpu, interrupt);
        }

        if !interrupt.in_lr() && (int_targets & !(1 << cpu_id)) != 0 {
            let vcpu_vm_id = vcpu.vm_id();

            let ipi_msg = IpiInitcMessage {
                event: InitcEvent::VgicdRoute,
                vm_id: vcpu_vm_id,
                int_id: interrupt.id(),
                val: 0,
            };
            vgic_int_yield_owner(vcpu, interrupt);

            //TODO:  ipi_intra_broadcast_msg(&active_vm().unwrap(), IpiType::IpiTIntc, IpiInnerMsg::Initc(ipi_msg));
        }
    }

    pub fn set_enable<T: VcpuTrait<Vm>>(&self, vcpu: &T, int_id: usize, en: bool) {
        if int_id < GIC_SGIS_NUM {
            return;
        }
        match self.get_int(vcpu, int_id) {
            Some(interrupt) => {
                let interrupt_lock = interrupt.lock.lock();
                if vgic_int_get_owner(vcpu, interrupt) {
                    if interrupt.enabled() ^ en {
                        interrupt.set_enabled(en);
                        if !interrupt.enabled() {
                            self.remove_lr(vcpu, interrupt);
                        } else {
                            self.route(vcpu, interrupt);
                        }
                        if interrupt.hw() {
                            GicDistributor::set_enable(interrupt.id() as usize, en);
                        }
                    }
                    vgic_int_yield_owner(vcpu, interrupt);
                } else {
                    let int_phys_id = interrupt.owner_phys_id().unwrap();
                    let vcpu_vm_id = vcpu.vm_id();
                    let ipi_msg = IpiInitcMessage {
                        event: InitcEvent::VgicdSetEn,
                        vm_id: vcpu_vm_id,
                        int_id: interrupt.id(),
                        val: en as u8,
                    };

                    // TODO
                    /*
                    if !ipi_send_msg(int_phys_id, IpiType::IpiTIntc, IpiInnerMsg::Initc(ipi_msg)) {
                        // error!(
                        //     "vgicd_set_enable: Failed to send ipi message, target {} type {}",
                        //     int_phys_id, 0
                        // );
                    }
                    */
                }
                drop(interrupt_lock);
            }
            None => {
                // error!("vgicd_set_enable: interrupt {} is illegal", int_id);
            }
        }
    }

    /* nothing */
    pub fn get_enable<T: VcpuTrait<Vm>>(&self, vcpu: &T, int_id: usize) -> bool {
        self.get_int(vcpu, int_id).unwrap().enabled()
    }

    pub fn set_pend<T: VcpuTrait<Vm>>(&self, vcpu: &T, int_id: usize, pend: bool) {
        // TODO: sgi_get_pend ?
        if bit_extract(int_id, 0, 10) < GIC_SGIS_NUM {
            self.sgi_set_pend(vcpu, int_id, pend);
            return;
        }

        if let Some(interrupt) = self.get_int(vcpu, bit_extract(int_id, 0, 10)) {
            let interrupt_lock = interrupt.lock.lock();
            if vgic_int_get_owner(vcpu, interrupt) {
                self.remove_lr(vcpu, interrupt);

                let state = interrupt.state().to_num();
                if pend && ((state & 1) == 0) {
                    interrupt.set_state(IrqState::num_to_state(state | 1));
                } else if !pend && (state & 1) != 0 {
                    interrupt.set_state(IrqState::num_to_state(state & !1));
                }
                self.update_int_list(vcpu.id(), interrupt);

                let state = interrupt.state().to_num();
                if interrupt.hw() {
                    let vgic_int_id = interrupt.id() as usize;
                    GicDistributor::set_state(vgic_int_id, if state == 1 { 2 } else { state })
                }
                self.route(vcpu, interrupt);
                vgic_int_yield_owner(vcpu, interrupt);
                drop(interrupt_lock);
            } else {
                let vm_id = vcpu.vm_id();

                let m = IpiInitcMessage {
                    event: InitcEvent::VgicdSetPend,
                    vm_id,
                    int_id: interrupt.id(),
                    val: pend as u8,
                };
                match interrupt.owner() {
                    Some(owner) => {
                        let phys_id = owner.phys_id();

                        drop(interrupt_lock);
                        // TODO 
                        /*
                        if !ipi_send_msg(phys_id, IpiType::IpiTIntc, IpiInnerMsg::Initc(m)) {
                            // error!(
                            //     "vgicd_set_pend: Failed to send ipi message, target {} type {}",
                            //     phys_id, 0
                            // );
                        }
                        */
                    }
                    None => {
                        panic!(
                            "set_pend: Core {} int {} has no owner",
                            current_cpu().id(),
                            interrupt.id()
                        );
                    }
                }
            }
        }
    }

    pub fn set_active<T: VcpuTrait<Vm>>(&self, vcpu: &T, int_id: usize, act: bool) {
        if let Some(interrupt) = self.get_int(vcpu, bit_extract(int_id, 0, 10)) {
            let interrupt_lock = interrupt.lock.lock();
            if vgic_int_get_owner(vcpu, interrupt) {
                self.remove_lr(vcpu, interrupt);
                let state = interrupt.state().to_num();
                if act && ((state & 2) == 0) {
                    interrupt.set_state(IrqState::num_to_state(state | 2));
                } else if !act && (state & 2) != 0 {
                    interrupt.set_state(IrqState::num_to_state(state & !2));
                }
                self.update_int_list(vcpu.id(), interrupt);

                let state = interrupt.state().to_num();
                if interrupt.hw() {
                    let vgic_int_id = interrupt.id() as usize;
                    GicDistributor::set_state(vgic_int_id, if state == 1 { 2 } else { state })
                }
                self.route(vcpu, interrupt);
                vgic_int_yield_owner(vcpu, interrupt);
            } else {
                let vm_id = vcpu.vm_id();

                let m = IpiInitcMessage {
                    event: InitcEvent::VgicdSetPend,
                    vm_id,
                    int_id: interrupt.id(),
                    val: act as u8,
                };
                let phys_id = interrupt.owner_phys_id().unwrap();
                //TODO 
                /*
                if !ipi_send_msg(phys_id, IpiType::IpiTIntc, IpiInnerMsg::Initc(m)) {
                    // error!(
                    //     "vgicd_set_active: Failed to send ipi message, target {} type {}",
                    //     phys_id, 0
                    // );
                }
                */
            }
            drop(interrupt_lock);
        }
    }

    pub fn set_icfgr<T: VcpuTrait<Vm>>(&self, vcpu: &T, int_id: usize, cfg: u8) {
        if let Some(interrupt) = self.get_int(vcpu, int_id) {
            let interrupt_lock = interrupt.lock.lock();
            if vgic_int_get_owner(vcpu, interrupt) {
                interrupt.set_cfg(cfg);
                if interrupt.hw() {
                    GicDistributor::set_icfgr(interrupt.id() as usize, cfg);
                }
                vgic_int_yield_owner(vcpu, interrupt);
            } else {
                let m = IpiInitcMessage {
                    event: InitcEvent::VgicdSetCfg,
                    vm_id: vcpu.vm_id(),
                    int_id: interrupt.id(),
                    val: cfg,
                };

                //TODO 
                /*
                if !ipi_send_msg(
                    interrupt.owner_phys_id().unwrap(),
                    IpiType::IpiTIntc,
                    IpiInnerMsg::Initc(m),
                ) {
                    // error!(
                    //     "set_icfgr: Failed to send ipi message, target {} type {}",
                    //     interrupt.owner_phys_id().unwrap(),
                    //     0
                    // );
                }
                */
            }
            drop(interrupt_lock);
        } else {
            unimplemented!();
        }
    }

    pub fn get_icfgr<T: VcpuTrait<Vm>>(&self, vcpu: &T, int_id: usize) -> u8 {
        if let Some(interrupt) = self.get_int(vcpu, int_id) {
            interrupt.cfg()
        } else {
            unimplemented!();
        }
    }

    fn sgi_set_pend<T: VcpuTrait<Vm>>(&self, vcpu: &T, int_id: usize, pend: bool) {
        if bit_extract(int_id, 0, 10) > GIC_SGIS_NUM {
            return;
        }

        let source = bit_extract(int_id, 10, 5);

        if let Some(interrupt) = self.get_int(vcpu, bit_extract(int_id, 0, 10)) {
            let interrupt_lock = interrupt.lock.lock();
            self.remove_lr(vcpu, interrupt);
            let vcpu_id = vcpu.id();

            let vgic_int_id = interrupt.id() as usize;
            let pendstate = self.cpu_priv_sgis_pend(vcpu_id, vgic_int_id);
            let new_pendstate = if pend {
                pendstate | (1 << source) as u8
            } else {
                pendstate & !(1 << source) as u8
            };
            // state changed ,the two state isn`t equal
            if (pendstate ^ new_pendstate) != 0 {
                self.set_cpu_priv_sgis_pend(vcpu_id, vgic_int_id, new_pendstate);
                let state = interrupt.state().to_num();
                if new_pendstate != 0 {
                    interrupt.set_state(IrqState::num_to_state(state | 1));
                } else {
                    interrupt.set_state(IrqState::num_to_state(state & !1));
                }

                self.update_int_list(vcpu.id(), interrupt);

                match interrupt.state() {
                    IrqState::IrqSInactive => {
                        // debug!("inactive");
                    }
                    _ => {
                        self.add_lr(vcpu, interrupt);
                    }
                }
            }
            drop(interrupt_lock);
        } else {
            // error!("sgi_set_pend: interrupt {} is None", bit_extract(int_id, 0, 10));
        }
    }

    pub fn set_prio<T: VcpuTrait<Vm>>(&self, vcpu: &T, int_id: usize, mut prio: u8) {
        if let Some(interrupt) = self.get_int(vcpu, int_id) {
            prio &= 0xf0; // gic-400 only allows 4 priority bits in non-secure state

            let interrupt_lock = interrupt.lock.lock();
            if vgic_int_get_owner(vcpu, interrupt) {
                if interrupt.prio() != prio {
                    self.remove_lr(vcpu, interrupt);
                    let prev_prio = interrupt.prio();
                    interrupt.set_prio(prio);
                    if prio <= prev_prio {
                        self.route(vcpu, interrupt);
                    }
                    if interrupt.hw() {
                        GicDistributor::set_priority(interrupt.id() as usize, prio);
                    }
                }
                vgic_int_yield_owner(vcpu, interrupt);
            } else {
                let vm_id = vcpu.vm_id();

                let m = IpiInitcMessage {
                    event: InitcEvent::VgicdSetPrio,
                    vm_id,
                    int_id: interrupt.id(),
                    val: prio,
                };
                // TODO
                /*
                if !ipi_send_msg(
                    interrupt.owner_phys_id().unwrap(),
                    IpiType::IpiTIntc,
                    IpiInnerMsg::Initc(m),
                ) {
                    // error!(
                    //     "set_prio: Failed to send ipi message, target {} type {}",
                    //     interrupt.owner_phys_id().unwrap(),
                    //     0
                    // );
                }
                */
            }
            drop(interrupt_lock);
        }
    }

    /* nothing  */
    pub fn get_prio<T: VcpuTrait<Vm>>(&self, vcpu: &T, int_id: usize) -> u8 {
        self.get_int(vcpu, int_id).unwrap().prio()
    }

    pub fn set_trgt<T: VcpuTrait<Vm>>(&self, vcpu: &T, int_id: usize, trgt: u8) {
        if let Some(interrupt) = self.get_int(vcpu, int_id) {
            let interrupt_lock = interrupt.lock.lock();
            if vgic_int_get_owner(vcpu, interrupt) {
                if interrupt.targets() != trgt {
                    interrupt.set_targets(trgt);
                    let mut ptrgt = 0;
                    for cpuid in 0..8 {
                        if bit_get(trgt as usize, cpuid) != 0 {
                            ptrgt = bit_set(ptrgt, cpuid_to_cpuif(cpuid))
                        }
                    }
                    if interrupt.hw() {
                        GicDistributor::set_trgt(interrupt.id() as usize, ptrgt as u8);
                    }
                    if vgic_get_state(interrupt) != 0 {
                        self.route(vcpu, interrupt);
                    }
                }
                vgic_int_yield_owner(vcpu, interrupt);
            } else {
                let vm_id = vcpu.vm_id();
                let m = IpiInitcMessage {
                    event: InitcEvent::VgicdSetTrgt,
                    vm_id,
                    int_id: interrupt.id(),
                    val: trgt,
                };
                // TODO
                /*
                if !ipi_send_msg(
                    interrupt.owner_phys_id().unwrap(),
                    IpiType::IpiTIntc,
                    IpiInnerMsg::Initc(m),
                ) {
                    // error!(
                    //     "set_trgt: Failed to send ipi message, target {} type {}",
                    //     interrupt.owner_phys_id().unwrap(),
                    //     0
                    // );
                }
                */
            }
            drop(interrupt_lock);
        }
    }

    /* nothing */
    pub fn get_trgt<T: VcpuTrait<Vm>>(&self, vcpu: &T, int_id: usize) -> u8 {
        self.get_int(vcpu, int_id).unwrap().targets()
    }

    pub fn inject<T: VcpuTrait<Vm>>(&self, vcpu: &T, int_id: usize) {
        if let Some(interrupt) = self.get_int(vcpu, bit_extract(int_id, 0, 10)) {
            if interrupt.hw() {
                let interrupt_lock = interrupt.lock.lock();
                interrupt.locked_helper(|interrupt| {
                    interrupt.owner = Some(vcpu.clone());
                    interrupt.state = IrqState::IrqSPend;
                    interrupt.in_lr = false;
                });
                self.update_int_list(vcpu.id(), interrupt);
                self.route(vcpu, interrupt);
                drop(interrupt_lock);
            } else {
                self.set_pend(vcpu, int_id, true);
            }
        }
    }

}




/// Maps CPU ID to CPU interface number for QEMU
fn cpuid_to_cpuif(cpuid: usize) -> usize {
    // PLAT_DESC.cpu_desc.core_list[cpuid].mpidr
    cpuid
}

pub fn vgic_target_translate(vm: &Vm, trgt: u32, v2p: bool) -> u32 {
    let from = trgt.to_le_bytes();

    let mut result = 0;
    for (idx, val) in from
        .map(|x| {
            if v2p {
                vm.vcpu_to_pcpu_mask(x as usize, 8) as u32
            } else {
                vm.pcpu_to_vcpu_mask(x as usize, 8) as u32
            }
        })
        .iter()
        .enumerate()
    {
        result |= *val << (8 * idx);
        if idx >= 4 {
            panic!("illegal idx, from len {}", from.len());
        }
    }
    result
}

// vcpu_id, pcpu_id
// 只考虑 spi 
// 中断的所有者是当前 vcpu 返回真
fn vgic_owns<T: VcpuTrait<Vm>>(vcpu: &T, interrupt: &VgicInt) -> bool {
    // sgi ppi 
    if gic_is_priv(interrupt.id() as usize) {
        return true;
    }

    let vcpu_id = vcpu.id();
    let pcpu_id = vcpu.phys_id();
    match interrupt.owner() {
        Some(owner) => { 
            let owner_vcpu_id = owner.id();
            let owner_pcpu_id = owner.phys_id();
            owner_vcpu_id == vcpu_id && owner_pcpu_id == pcpu_id
        }
        None => false,
    }
}


/// 1、这个int没有owner的话，设置当前vcpu为他的主人  返回真
/// 2、这个int有owner，返回 owner_vm_id == vcpu_vm_id && owner_vcpu_id == vcpu_id 
pub fn vgic_int_get_owner<T: VcpuTrait<Vm>>(vcpu: &T, interrupt: &VgicInt) -> bool {
    let vcpu_id = vcpu.id();
    let vcpu_vm_id = vcpu.vm_id();

    match interrupt.owner() {
        Some(owner) => {
            let owner_vcpu_id = owner.id();
            let owner_vm_id = owner.vm_id();

            owner_vm_id == vcpu_vm_id && owner_vcpu_id == vcpu_id
        }
        None => {
            interrupt.set_owner(vcpu.clone());
            true
        }
    }
}

pub fn vgic_get_state(interrupt: &VgicInt) -> usize {
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

    let vm = interrupt.owner_vm();
    let vgic = vm.vgic();
    let vcpu_id = interrupt.owner_id().unwrap();

    if vgic.cpu_priv_sgis_pend(vcpu_id, interrupt.id() as usize) != 0 {
        state |= 1;
    }

    state
}

// vcpu_id, pcpu_id
pub fn vgic_int_yield_owner<T: VcpuTrait<Vm>>(vcpu: &T, interrupt: &VgicInt) {
    if !vgic_owns(vcpu, interrupt) || interrupt.in_lr() || gic_is_priv(interrupt.id() as usize) {
        return;
    }

    if vgic_get_state(interrupt) & 2 == 0 {
        interrupt.clear_owner();
    }
}

pub fn vgic_int_is_hw(interrupt: &VgicInt) -> bool {
    interrupt.id() as usize >= GIC_SGIS_NUM && interrupt.hw()
}

fn gich_get_lr(interrupt: &VgicInt) -> Option<u32> {
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

/* Do this in config */

pub static GIC_LRS_NUM: AtomicUsize = AtomicUsize::new(0);

pub fn gic_lrs() -> usize {
    GIC_LRS_NUM.load(Ordering::Relaxed)
}

pub fn set_gic_lrs(lrs: usize) {
    GIC_LRS_NUM.store(lrs, Ordering::Relaxed);
}