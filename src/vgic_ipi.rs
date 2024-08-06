
use crate::vgic::vgic_int_yield_owner;
use crate::vgic::vgic_int_get_owner;
use crate::GicHypervisorInterface;
use crate::utils::bit_extract;
extern crate alloc;
use crate::fake::*;

use crate::vgic_traits::VcpuTrait;
use crate::vgic_traits::PcpuTrait;
use crate::vgic_traits::VmTrait;


pub fn vgic_ipi_handler(msg: IpiMessage) {
    if let IpiInnerMsg::Initc(intc) = msg.ipi_message {
        let vm_id = intc.vm_id;
        let int_id = intc.int_id;
        let val = intc.val;
        let array = current_cpu().vcpu_array;
        let trgt_vcpu = match array.pop_vcpu_through_vmid(vm_id) {
            None => {
                // error!("Core {} received vgic msg from unknown VM {}", current_cpu().id, vm_id);
                return;
            }
            Some(vcpu) => vcpu,
        };
        // TODO: restore_vcpu_gic(current_cpu().active_vcpu.clone(), trgt_vcpu.clone());

        let vm = match trgt_vcpu.vm() {
            None => {
                panic!("vgic_ipi_handler: vm is None");
            }
            Some(x) => x,
        };
        let vgic = vm.get_vgic();

        if vm_id != vm.id() {
            // error!("VM {} received vgic msg from another vm {}", vm.id(), vm_id);
            return;
        }
        match intc.event {
            InitcEvent::VgicdGichEn => {
                let hcr = GicHypervisorInterface::hcr();
                if val != 0 {
                    GicHypervisorInterface::set_hcr(hcr | 0b1);
                } else {
                    GicHypervisorInterface::set_hcr(hcr & !0b1);
                }
            }
            InitcEvent::VgicdSetEn => {
                vgic.set_enable(trgt_vcpu, int_id as usize, val != 0);
            }
            InitcEvent::VgicdSetPend => {
                vgic.set_pend(trgt_vcpu, int_id as usize, val != 0);
            }
            InitcEvent::VgicdSetPrio => {
                vgic.set_prio(trgt_vcpu, int_id as usize, val);
            }
            InitcEvent::VgicdSetTrgt => {
                vgic.set_trgt(trgt_vcpu, int_id as usize, val);
            }
            InitcEvent::VgicdRoute => {
                if let Some(interrupt) = vgic.get_int(trgt_vcpu, bit_extract(int_id as usize, 0, 10)) {
                    let interrupt_lock = interrupt.lock.lock();
                    if vgic_int_get_owner(trgt_vcpu, interrupt) {
                        if (interrupt.targets() & (1 << current_cpu().id())) != 0 {
                            vgic.add_lr(trgt_vcpu, interrupt);
                        }
                        vgic_int_yield_owner(trgt_vcpu, interrupt);
                    }
                    drop(interrupt_lock);
                }
            }
            _ => {
                // error!("vgic_ipi_handler: core {} received unknown event", current_cpu().id)
            }
        }
        //TODO: save_vcpu_gic(current_cpu().active_vcpu.clone(), trgt_vcpu);
    } else {
        // error!("vgic_ipi_handler: illegal ipi");
    }
}
