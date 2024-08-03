

pub const GIC_SGIS_NUM:    usize = 16;
const GIC_PPIS_NUM:        usize = 16;
pub const GIC_PRIVINT_NUM: usize = GIC_SGIS_NUM + GIC_PPIS_NUM;
pub const GIC_INTS_MAX:    usize = 1024;
pub const GIC_SPI_MAX:     usize = GIC_INTS_MAX - GIC_PRIVINT_NUM;

/* TODO: */
pub const GIC_LIST_REGS_NUM:    usize = 64;
pub const GICH_HCR_LRENPIE_BIT: usize = 1 << 2;


/* ============ handler use offset ============= */
pub const VGICD_REG_OFFSET_PREFIX_CTLR:      usize = 0x0;
pub const VGICD_REG_OFFSET_PREFIX_ISENABLER: usize = 0x2;
pub const VGICD_REG_OFFSET_PREFIX_ICENABLER: usize = 0x3;
pub const VGICD_REG_OFFSET_PREFIX_ISPENDR:   usize = 0x4;
pub const VGICD_REG_OFFSET_PREFIX_ICPENDR:   usize = 0x5;
pub const VGICD_REG_OFFSET_PREFIX_ISACTIVER: usize = 0x6;
pub const VGICD_REG_OFFSET_PREFIX_ICACTIVER: usize = 0x7;
pub const VGICD_REG_OFFSET_PREFIX_ICFGR:     usize = 0x18;
pub const VGICD_REG_OFFSET_PREFIX_SGIR:      usize = 0x1e;



pub const GIC_TARGET_BITS: usize = 8;
pub const GIC_TARGETS_MAX: usize = GIC_TARGET_BITS;
pub const GIC_PRIO_BITS:   usize = 8;
pub const GIC_CONFIG_BITS: usize = 2;



pub const GICD_TYPER_CPUNUM_MSK: usize = 0b11111;
pub const GICD_TYPER_CPUNUM_OFF: usize = 5;