
use crate::vgic::Vgic;
use crate::vgic::vgic_get_state;
use crate::vgic::vgic_target_translate;
use crate::consts::*;
use crate::GicHypervisorInterface;
use arm_gic::gic_v2::GicDistributor;
use crate::utils::{bit_extract, bit_get};

extern crate alloc;
use core::ops::Range;


use crate::fake::*;

pub fn vgicd_emu_access_is_vaild(emu_ctx: &EmuContext) -> bool {
    let offset = emu_ctx.address & 0xfff;
    let offset_prefix = (offset & 0xf80) >> 7;
    match offset_prefix {
        VGICD_REG_OFFSET_PREFIX_CTLR
        | VGICD_REG_OFFSET_PREFIX_ISENABLER
        | VGICD_REG_OFFSET_PREFIX_ISPENDR
        | VGICD_REG_OFFSET_PREFIX_ISACTIVER
        | VGICD_REG_OFFSET_PREFIX_ICENABLER
        | VGICD_REG_OFFSET_PREFIX_ICPENDR
        | VGICD_REG_OFFSET_PREFIX_ICACTIVER
        | VGICD_REG_OFFSET_PREFIX_ICFGR => {
            if emu_ctx.width != 4 || emu_ctx.address & 0x3 != 0 {
                return false;
            }
        }
        VGICD_REG_OFFSET_PREFIX_SGIR => {
            if (emu_ctx.width == 4 && emu_ctx.address & 0x3 != 0) || (emu_ctx.width == 2 && emu_ctx.address & 0x1 != 0)
            {
                return false;
            }
        }
        _ => {
            // TODO: hard code to rebuild (gicd IPRIORITYR and ITARGETSR)
            if (0x400..0xc00).contains(&offset)
                && ((emu_ctx.width == 4 && emu_ctx.address & 0x3 != 0)
                    || (emu_ctx.width == 2 && emu_ctx.address & 0x1 != 0))
            {
                return false;
            }
        }
    }
    true
}

impl Vgic {
    /* set gpr get gpr */
    /* 找到当前活跃 vm ，向其vcpu发送ipi，保证ctrlr同步 */
    fn emu_ctrl_access(&self, emu_ctx: &EmuContext) {
        if emu_ctx.write {
            let prev_ctlr = self.vgicd_ctlr();
            let idx = emu_ctx.reg;
            self.set_vgicd_ctlr(current_cpu().get_gpr(idx) as u32 & 0x1);
            if prev_ctlr ^ self.vgicd_ctlr() != 0 {
                let enable = self.vgicd_ctlr() != 0;
                let hcr = GicHypervisorInterface::hcr();
                if enable {
                    GicHypervisorInterface::set_hcr(hcr | 1);
                } else {
                    GicHypervisorInterface::set_hcr(hcr & !1);
                }

                let m = IpiInitcMessage {
                    event: InitcEvent::VgicdGichEn,
                    vm_id: active_vm_id(),
                    int_id: 0,
                    val: enable as u8,
                };
                //  Ensure all processors are synchronously updated when there is a change in Ctrl state.
                // TODO: ipi_intra_broadcast_msg(&active_vm().unwrap(), IpiType::IpiTIntc, IpiInnerMsg::Initc(m));
            }
        } else {
            let idx = emu_ctx.reg;
            let val = self.vgicd_ctlr() as usize;
            current_cpu().set_gpr(idx, val);
        }
    }

    /* set gpr get gpr */
    fn emu_typer_access(&self, emu_ctx: &EmuContext) {
        if !emu_ctx.write {
            let idx = emu_ctx.reg;
            let val = self.vgicd_typer() as usize;
            current_cpu().set_gpr(idx, val);
        } else {
            // warn!("emu_typer_access: can't write to RO reg");
        }
    }

    /* set gpr get gpr */
    fn emu_iidr_access(&self, emu_ctx: &EmuContext) {
        if !emu_ctx.write {
            let idx = emu_ctx.reg;
            let val = self.vgicd_iidr() as usize;
            current_cpu().set_gpr(idx, val);
        } else {
            // warn!("emu_iidr_access: can't write to RO reg");
        }
    }

    /* set gpr get gpr */
    /*  */
    fn emu_isenabler_access(&self, emu_ctx: &EmuContext) {
        // println!("DEBUG: in emu_isenabler_access");
        let reg_idx = (emu_ctx.address & 0b1111111) / 4;
        let idx = emu_ctx.reg;
        let mut val = if emu_ctx.write { current_cpu().get_gpr(idx) } else { 0 };
        let first_int = reg_idx * 32;
        let vm_id = active_vm_id();
        let vm = match active_vm() {
            Some(vm) => vm,
            None => {
                panic!("emu_isenabler_access: current vcpu.vm is none");
            }
        };
        let mut vm_has_interrupt_flag = false;

        for i in 0..32 {
            if vm.has_interrupt(first_int + i) || vm.emu_has_interrupt(first_int + i) {
                vm_has_interrupt_flag = true;
                break;
            }
        }
        if first_int >= 16 && !vm_has_interrupt_flag {
            // error!(
            //     "emu_isenabler_access: vm[{}] does not have interrupt {}",
            //     vm_id, first_int
            // );
            return;
        }

        if emu_ctx.write {
            for i in 0..32 {
                if bit_get(val, i) != 0 {
                    self.set_enable(current_cpu().active_vcpu.as_ref().unwrap(), first_int + i, true);
                }
            }
        } else {
            for i in 0..32 {
                if self.get_enable(current_cpu().active_vcpu.as_ref().unwrap(), first_int + i) {
                    val |= 1 << i;
                }
            }
            let idx = emu_ctx.reg;
            current_cpu().set_gpr(idx, val);
        }
    }

    /* set gpr get gpr */
    /*  */
    fn emu_pendr_access(&self, emu_ctx: &EmuContext, set: bool) {
        // trace!("emu_pendr_access");
        let reg_idx = (emu_ctx.address & 0b1111111) / 4;
        let idx = emu_ctx.reg;
        let mut val = if emu_ctx.write { current_cpu().get_gpr(idx) } else { 0 };
        let first_int = reg_idx * 32;
        let vm_id = active_vm_id();
        let vm = match active_vm() {
            Some(vm) => vm,
            None => {
                panic!("emu_pendr_access: current vcpu.vm is none");
            }
        };
        let mut vm_has_interrupt_flag = false;

        for i in 0..emu_ctx.width {
            if vm.has_interrupt(first_int + i) || vm.emu_has_interrupt(first_int + i) {
                vm_has_interrupt_flag = true;
                break;
            }
        }
        if first_int >= 16 && !vm_has_interrupt_flag {
            // error!("emu_pendr_access: vm[{}] does not have interrupt {}", vm_id, first_int);
            return;
        }

        if emu_ctx.write {
            for i in 0..32 {
                if bit_get(val, i) != 0 {
                    self.set_pend(current_cpu().active_vcpu.as_ref().unwrap(), first_int + i, set);
                }
            }
        } else {
            for i in 0..32 {
                match self.get_int(current_cpu().active_vcpu.as_ref().unwrap(), first_int + i) {
                    Some(interrupt) => {
                        if vgic_get_state(interrupt) & 1 != 0 {
                            val |= 1 << i;
                        }
                    }
                    None => {
                        unimplemented!();
                    }
                }
            }
            let idx = emu_ctx.reg;
            current_cpu().set_gpr(idx, val);
        }
    }

    /* nothing */
    fn emu_ispendr_access(&self, emu_ctx: &EmuContext) {
        self.emu_pendr_access(emu_ctx, true);
    }

    fn emu_activer_access(&self, emu_ctx: &EmuContext, set: bool) {
        // println!("DEBUG: in emu_activer_access");
        let reg_idx = (emu_ctx.address & 0b1111111) / 4;
        let idx = emu_ctx.reg;
        let mut val = if emu_ctx.write { current_cpu().get_gpr(idx) } else { 0 };
        let first_int = reg_idx * 32;
        let vm_id = active_vm_id();
        let vm = match active_vm() {
            Some(vm) => vm,
            None => {
                panic!("emu_activer_access: current vcpu.vm is none");
            }
        };
        let mut vm_has_interrupt_flag = false;

        for i in 0..32 {
            if vm.has_interrupt(first_int + i) || vm.emu_has_interrupt(first_int + i) {
                vm_has_interrupt_flag = true;
                break;
            }
        }
        if first_int >= 16 && !vm_has_interrupt_flag {
            // warn!(
            //     "emu_activer_access: vm[{}] does not have interrupt {}",
            //     vm_id, first_int
            // );
            return;
        }

        if emu_ctx.write {
            for i in 0..32 {
                if bit_get(val, i) != 0 {
                    self.set_active(current_cpu().active_vcpu.as_ref().unwrap(), first_int + i, set);
                }
            }
        } else {
            for i in 0..32 {
                match self.get_int(current_cpu().active_vcpu.as_ref().unwrap(), first_int + i) {
                    Some(interrupt) => {
                        if vgic_get_state(interrupt) & 2 != 0 {
                            val |= 1 << i;
                        }
                    }
                    None => {
                        unimplemented!();
                    }
                }
            }
            let idx = emu_ctx.reg;
            current_cpu().set_gpr(idx, val);
        }
    }

    /* nothing */
    fn emu_isactiver_access(&self, emu_ctx: &EmuContext) {
        self.emu_activer_access(emu_ctx, true);
    }

    fn emu_icenabler_access(&self, emu_ctx: &EmuContext) {
        let reg_idx = (emu_ctx.address & 0b1111111) / 4;
        let idx = emu_ctx.reg;
        let mut val = if emu_ctx.write { current_cpu().get_gpr(idx) } else { 0 };
        let first_int = reg_idx * 32;
        let vm_id = active_vm_id();
        let vm = match active_vm() {
            Some(vm) => vm,
            None => {
                panic!("emu_activer_access: current vcpu.vm is none");
            }
        };
        let mut vm_has_interrupt_flag = false;

        if emu_ctx.write {
            for i in 0..32 {
                if vm.has_interrupt(first_int + i) || vm.emu_has_interrupt(first_int + i) {
                    vm_has_interrupt_flag = true;
                    break;
                }
            }
            if first_int >= 16 && !vm_has_interrupt_flag {
                // warn!(
                //     "emu_icenabler_access: vm[{}] does not have interrupt {}",
                //     vm_id, first_int
                // );
                return;
            }
        }

        if emu_ctx.write {
            for i in 0..32 {
                if bit_get(val, i) != 0 {
                    self.set_enable(current_cpu().active_vcpu.as_ref().unwrap(), first_int + i, false);
                }
            }
        } else {
            for i in 0..32 {
                if self.get_enable(current_cpu().active_vcpu.as_ref().unwrap(), first_int + i) {
                    val |= 1 << i;
                }
            }
            let idx = emu_ctx.reg;
            current_cpu().set_gpr(idx, val);
        }
    }

    /* nothing */
    fn emu_icpendr_access(&self, emu_ctx: &EmuContext) {
        self.emu_pendr_access(emu_ctx, false);
    }

    /* nothing */
    fn emu_icativer_access(&self, emu_ctx: &EmuContext) {
        self.emu_activer_access(emu_ctx, false);
    }

    fn emu_icfgr_access(&self, emu_ctx: &EmuContext) {
        let first_int = (32 / GIC_CONFIG_BITS) * bit_extract(emu_ctx.address, 0, 9) / 4;
        let vm_id = active_vm_id();
        let vm = match active_vm() {
            Some(vm) => vm,
            None => {
                panic!("emu_icfgr_access: current vcpu.vm is none");
            }
        };
        let mut vm_has_interrupt_flag = false;

        if emu_ctx.write {
            for i in 0..emu_ctx.width * 8 {
                if vm.has_interrupt(first_int + i) || vm.emu_has_interrupt(first_int + i) {
                    vm_has_interrupt_flag = true;
                    break;
                }
            }
            if first_int >= 16 && !vm_has_interrupt_flag {
                // warn!("emu_icfgr_access: vm[{}] does not have interrupt {}", vm_id, first_int);
                return;
            }
        }

        if emu_ctx.write {
            let idx = emu_ctx.reg;
            let cfg = current_cpu().get_gpr(idx);
            let mut irq = first_int;
            let mut bit = 0;
            while bit < emu_ctx.width * 8 {
                self.set_icfgr(
                    current_cpu().active_vcpu.as_ref().unwrap(),
                    irq,
                    bit_extract(cfg as usize, bit, 2) as u8,
                );
                bit += 2;
                irq += 1;
            }
        } else {
            let mut cfg = 0;
            let mut irq = first_int;
            let mut bit = 0;
            while bit < emu_ctx.width * 8 {
                cfg |= (self.get_icfgr(current_cpu().active_vcpu.as_ref().unwrap(), irq) as usize) << bit;
                bit += 2;
                irq += 1;
            }
            let idx = emu_ctx.reg;
            let val = cfg;
            current_cpu().set_gpr(idx, val);
        }
    }

    fn emu_sgiregs_access(&self, emu_ctx: &EmuContext) {
        let idx = emu_ctx.reg;
        let val = if emu_ctx.write { current_cpu().get_gpr(idx) } else { 0 };
        let vm = match active_vm() {
            Some(vm) => vm,
            None => {
                panic!("emu_sgiregs_access: current vcpu.vm is none");
            }
        };

        if bit_extract(emu_ctx.address, 0, 12) == bit_extract(GicDistributor::gicd_base() + 0x0f00, 0, 12) {
            if emu_ctx.write {
                let sgir_trglstflt = bit_extract(val, 24, 2);
                let mut trgtlist = 0;
                // println!("addr {:x}, sgir trglst flt {}, vtrgt {}", emu_ctx.address, sgir_trglstflt, bit_extract(val, 16, 8));
                match sgir_trglstflt {
                    0 => {
                        trgtlist = vgic_target_translate(&vm, bit_extract(val, 16, 8) as u32, true) as usize;
                    }
                    1 => {
                        trgtlist = active_vm_ncpu() & !(1 << current_cpu().id());
                    }
                    2 => {
                        trgtlist = 1 << current_cpu().id();
                    }
                    3 => {
                        return;
                    }
                    _ => {}
                }

                for i in 0..8 {
                    if trgtlist & (1 << i) != 0 {
                        let m = IpiInitcMessage {
                            event: InitcEvent::VgicdSetPend,
                            vm_id: active_vm_id(),
                            int_id: (bit_extract(val, 0, 8) | (active_vcpu_id() << 10)) as u16,
                            val: true as u8,
                        };
                        //TODO
                        /*
                        if !ipi_send_msg(i, IpiType::IpiTIntc, IpiInnerMsg::Initc(m)) {
                            // error!(
                            //     "emu_sgiregs_access: Failed to send ipi message, target {} type {}",
                            //     i, 0
                            // );
                        }
                        */
                    }
                }
            }
        } else {
            // TODO: CPENDSGIR and SPENDSGIR access
            // warn!("unimplemented: CPENDSGIR and SPENDSGIR access");
        }
    }

    fn emu_ipriorityr_access(&self, emu_ctx: &EmuContext) {
        let idx = emu_ctx.reg;
        let mut val = if emu_ctx.write { current_cpu().get_gpr(idx) } else { 0 };
        let first_int = (8 / GIC_PRIO_BITS) * bit_extract(emu_ctx.address, 0, 9);
        let vm_id = active_vm_id();
        let vm = match active_vm() {
            Some(vm) => vm,
            None => {
                panic!("emu_ipriorityr_access: current vcpu.vm is none");
            }
        };
        let mut vm_has_interrupt_flag = false;

        if emu_ctx.write {
            for i in 0..emu_ctx.width {
                if vm.has_interrupt(first_int + i) || vm.emu_has_interrupt(first_int + i) {
                    vm_has_interrupt_flag = true;
                    break;
                }
            }
            if first_int >= 16 && !vm_has_interrupt_flag {
                // warn!(
                //     "emu_ipriorityr_access: vm[{}] does not have interrupt {}",
                //     vm_id, first_int
                // );
                return;
            }
        }

        if emu_ctx.write {
            for i in 0..emu_ctx.width {
                self.set_prio(
                    current_cpu().active_vcpu.as_ref().unwrap(),
                    first_int + i,
                    bit_extract(val, GIC_PRIO_BITS * i, GIC_PRIO_BITS) as u8,
                );
            }
        } else {
            for i in 0..emu_ctx.width {
                val |= (self.get_prio(current_cpu().active_vcpu.as_ref().unwrap(), first_int + i) as usize)
                    << (GIC_PRIO_BITS * i);
            }
            let idx = emu_ctx.reg;
            current_cpu().set_gpr(idx, val);
        }
    }

    fn emu_itargetr_access(&self, emu_ctx: &EmuContext) {
        let idx = emu_ctx.reg;
        let mut val = if emu_ctx.write { current_cpu().get_gpr(idx) } else { 0 };
        let first_int = (8 / GIC_TARGET_BITS) * bit_extract(emu_ctx.address, 0, 9);

        if emu_ctx.write {
            val = vgic_target_translate(&active_vm().unwrap(), val as u32, true) as usize;
            for i in 0..emu_ctx.width {
                self.set_trgt(
                    current_cpu().active_vcpu.as_ref().unwrap(),
                    first_int + i,
                    bit_extract(val, GIC_TARGET_BITS * i, GIC_TARGET_BITS) as u8,
                );
            }
        } else {
            for i in 0..emu_ctx.width {
                val |= (self.get_trgt(current_cpu().active_vcpu.as_ref().unwrap(), first_int + i) as usize)
                    << (GIC_TARGET_BITS * i);
            }
            val = vgic_target_translate(&active_vm().unwrap(), val as u32, false) as usize;
            let idx = emu_ctx.reg;
            current_cpu().set_gpr(idx, val);
        }
    }


}


impl EmuDev for Vgic {
    fn emu_type(&self) -> EmuDeviceType {
        EmuDeviceType::EmuDeviceTGicd
    }

    fn address_range(&self) -> Range<usize> {
        self.address_range.clone()
    }

    fn handler(&self, emu_ctx: &EmuContext) -> bool {
        let offset = emu_ctx.address & 0xfff;

        let vgicd_offset_prefix = offset >> 7;
        if !vgicd_emu_access_is_vaild(emu_ctx) {
            return false;
        }

        // trace!(
        //     "current_cpu:{} emu_intc_handler offset:{:#x} is write:{},val:{:#x}",
        //     current_cpu().id,
        //     emu_ctx.address,
        //     emu_ctx.write,
        //     current_cpu().get_gpr(emu_ctx.reg)
        // );
        match vgicd_offset_prefix {
            VGICD_REG_OFFSET_PREFIX_ISENABLER => {
                self.emu_isenabler_access(emu_ctx);
            }
            VGICD_REG_OFFSET_PREFIX_ISPENDR => {
                self.emu_ispendr_access(emu_ctx);
            }
            VGICD_REG_OFFSET_PREFIX_ISACTIVER => {
                self.emu_isactiver_access(emu_ctx);
            }
            VGICD_REG_OFFSET_PREFIX_ICENABLER => {
                self.emu_icenabler_access(emu_ctx);
            }
            VGICD_REG_OFFSET_PREFIX_ICPENDR => {
                self.emu_icpendr_access(emu_ctx);
            }
            VGICD_REG_OFFSET_PREFIX_ICACTIVER => {
                self.emu_icativer_access(emu_ctx);
            }
            VGICD_REG_OFFSET_PREFIX_ICFGR => {
                self.emu_icfgr_access(emu_ctx);
            }
            VGICD_REG_OFFSET_PREFIX_SGIR => {
                self.emu_sgiregs_access(emu_ctx);
            }
            _ => {
                match offset {
                    // VGICD_REG_OFFSET(CTLR)
                    0 => {
                        self.emu_ctrl_access(emu_ctx);
                    }
                    // VGICD_REG_OFFSET(TYPER)
                    0x004 => {
                        self.emu_typer_access(emu_ctx);
                    }
                    // VGICD_REG_OFFSET(IIDR)
                    0x008 => {
                        self.emu_iidr_access(emu_ctx);
                    }
                    _ => {
                        if !emu_ctx.write {
                            current_cpu().set_gpr(emu_ctx.reg, 0);
                        }
                    }
                }
                if (0x400..0x800).contains(&offset) {
                    self.emu_ipriorityr_access(emu_ctx);
                } else if (0x800..0xc00).contains(&offset) {
                    self.emu_itargetr_access(emu_ctx);
                }
            }
        }
        true
    }
}