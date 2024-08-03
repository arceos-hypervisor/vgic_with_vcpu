
use crate::consts::*;
use crate::GicHypervisorInterface;
use arm_gic::gic_v2::GicCpuInterface;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct GicState {
    pub hcr     : u32,
    eisr        : [u32; GIC_LIST_REGS_NUM / 32],
    elrsr       : [u32; GIC_LIST_REGS_NUM / 32],
    apr         : u32,
    pub lr      : [u32; GIC_LIST_REGS_NUM],
    pub ctlr    : u32,
}

impl Default for GicState {
    fn default() -> Self {
        GicState {
            hcr: 0,
            eisr: [0; GIC_LIST_REGS_NUM / 32],
            elrsr: [0; GIC_LIST_REGS_NUM / 32],
            apr: 0,
            lr: [0; GIC_LIST_REGS_NUM],
            ctlr: 0,
        }
    }
}

impl GicState {
    fn save_state(&mut self) {
        self.hcr = GicHypervisorInterface::hcr();
        self.apr = GicHypervisorInterface::apr();
        for i in 0..(GIC_LIST_REGS_NUM / 32) {
            self.eisr[i] = GicHypervisorInterface::eisr(i);
            self.elrsr[i] = GicHypervisorInterface::elrsr(i);
        }
        for i in 0..GicHypervisorInterface::gich_lrs_num() {
            if self.elrsr[0] & 1 << i == 0 {
                self.lr[i] = GicHypervisorInterface::lr(i);
            } else {
                self.lr[i] = 0;
            }
        }

        self.ctlr = GicCpuInterface::ctrlr();   
    }

    fn restore_state(&self) {
        GicHypervisorInterface::set_hcr(self.hcr);
        GicHypervisorInterface::set_apr(self.apr);

        for i in 0..GicHypervisorInterface::gich_lrs_num() {
            GicHypervisorInterface::set_lr(i, self.lr[i]);
        }

        GicCpuInterface::set_ctrlr(self.ctlr);
              
    }
}
