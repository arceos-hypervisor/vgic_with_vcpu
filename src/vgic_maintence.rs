
use crate::vgic::Vgic;
use crate::vgic::gic_lrs;
use crate::vgic::vgic_int_yield_owner;
use crate::vgic::vgic_int_get_owner;
use crate::vgic::vgic_int_is_hw;
use crate::vint::*;
use crate::consts::*;
use crate::GicHypervisorInterface;
use arm_gic::gic_v2::GicDistributor;
use crate::utils::{bit_extract, bit_get, bit_set, bitmap_find_nth};

use crate::fake::*;
// for maintenance
impl Vgic {
    // vcpu_id
    // 得到vcpu的cpu_priv的 pend_list.front act_list.front
    pub fn int_list_head(&self, vcpu_id: usize, is_pend: bool) -> Option<&VgicInt> {
        // let vcpu_id = vcpu.id();
        let cpu_priv = self.cpu_priv[vcpu_id].inner_mut.borrow();
        if is_pend {
            // SAFETY: All VgicInt are allocated when initializing, so it's safe to convert them to NonNull
            cpu_priv.pend_list.front().cloned().map(|x| unsafe { x.as_ref() })
        } else {
            // SAFETY: All VgicInt are allocated when initializing, so it's safe to convert them to NonNull
            cpu_priv.act_list.front().cloned().map(|x| unsafe { x.as_ref() })
        }
    }
    
    // maintenance use
    fn handle_trapped_eoir(&self, vcpu: &Vcpu) {
        let gic_lrs = gic_lrs();
        let mut lr_idx_opt = bitmap_find_nth(
            GicHypervisorInterface::eisr(0) as usize | ((GicHypervisorInterface::eisr(1) as usize) << 32),
            0,
            gic_lrs,
            1,
            true,
        );

        while lr_idx_opt.is_some() {
            let lr_idx = lr_idx_opt.unwrap();
            let lr_val = GicHypervisorInterface::lr(lr_idx) as usize;
            GicHypervisorInterface::set_lr(lr_idx, 0);

            match self.get_int(vcpu, bit_extract(lr_val, 0, 10)) {
                Some(interrupt) => {
                    let interrupt_lock = interrupt.lock.lock();
                    interrupt.set_in_lr(false);
                    if (interrupt.id() as usize) < GIC_SGIS_NUM {
                        self.add_lr(vcpu, interrupt);
                    } else {
                        vgic_int_yield_owner(vcpu, interrupt);
                    }
                    drop(interrupt_lock);
                }
                None => {
                    unimplemented!();
                }
            }
            lr_idx_opt = bitmap_find_nth(
                GicHypervisorInterface::eisr(0) as usize | ((GicHypervisorInterface::eisr(1) as usize) << 32),
                0,
                gic_lrs,
                1,
                true,
            );
        }
    }

    // maintenance use
    fn refill_lrs(&self, vcpu: &Vcpu) {
        let gic_lrs = gic_lrs();
        let mut has_pending = false;

        for i in 0..gic_lrs {
            let lr = GicHypervisorInterface::lr(i) as usize;
            if bit_extract(lr, 28, 2) & 1 != 0 {
                has_pending = true;
            }
        }

        let mut lr_idx_opt = bitmap_find_nth(
            GicHypervisorInterface::elrsr(0) as usize | ((GicHypervisorInterface::elrsr(1) as usize) << 32),
            0,
            gic_lrs,
            1,
            true,
        );

        while lr_idx_opt.is_some() {
            let mut interrupt_opt: Option<&VgicInt> = None;
            let mut prev_pend = false;
            let act_head = self.int_list_head(vcpu.id(), false);
            let pend_head = self.int_list_head(vcpu.id(), true);
            if has_pending {
                if let Some(act_int) = act_head {
                    if !act_int.in_lr() {
                        interrupt_opt = Some(act_int);
                    }
                }
            }
            if interrupt_opt.is_none() {
                if let Some(pend_int) = pend_head {
                    if !pend_int.in_lr() {
                        interrupt_opt = Some(pend_int);
                        prev_pend = true;
                    }
                }
            }

            match interrupt_opt {
                Some(interrupt) => {
                    // println!("refill int {}", interrupt.id());
                    vgic_int_get_owner(vcpu, interrupt);
                    self.write_lr(vcpu, interrupt, lr_idx_opt.unwrap());
                    has_pending = has_pending || prev_pend;
                }
                None => {
                    // println!("no int to refill");
                    let hcr = GicHypervisorInterface::hcr();
                    GicHypervisorInterface::set_hcr(hcr & !(1 << 3));
                    break;
                }
            }

            lr_idx_opt = bitmap_find_nth(
                GicHypervisorInterface::elrsr(0) as usize | ((GicHypervisorInterface::elrsr(1) as usize) << 32),
                0,
                gic_lrs,
                1,
                true,
            );
        }
    }

    // maintenance use
    fn eoir_highest_spilled_active(&self, vcpu: &Vcpu) {
            if let Some(int) = self.int_list_head(vcpu.id(), false) {
                int.lock.lock();
                vgic_int_get_owner(vcpu, int);
    
                let state = int.state().to_num();
                int.set_state(IrqState::num_to_state(state & !2));
                self.update_int_list(vcpu.id(), int);
    
                if vgic_int_is_hw(int) {
                    GicDistributor::set_act(int.id() as usize, false);
                } else if int.state().to_num() & 1 != 0 {
                    self.add_lr(vcpu, int);
                }
            }
        }
}

pub fn gic_maintenance_handler() {
    let misr = GicHypervisorInterface::misr();
    let vm = match active_vm() {
        Some(vm) => vm,
        None => {
            panic!("gic_maintenance_handler: current vcpu.vm is None");
        }
    };
    let vgic = vm.vgic();

    if misr & 1 != 0 {
        vgic.handle_trapped_eoir(current_cpu().active_vcpu.as_ref().unwrap());
    }

    if misr & (1 << 3) != 0 {
        vgic.refill_lrs(current_cpu().active_vcpu.as_ref().unwrap());
    }

    if misr & (1 << 2) != 0 {
        let mut hcr = GicHypervisorInterface::hcr();
        while hcr & (0b11111 << 27) != 0 {
            vgic.eoir_highest_spilled_active(current_cpu().active_vcpu.as_ref().unwrap());
            hcr -= 1 << 27;
            GicHypervisorInterface::set_hcr(hcr);
            hcr = GicHypervisorInterface::hcr();
        }
    }
}
