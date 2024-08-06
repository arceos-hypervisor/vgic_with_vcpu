use crate::fake::Vcpu;
use crate::vgic::Vgic;
use alloc::sync::Arc;


pub trait VmTrait {
    fn id(&self) -> usize;

    fn vcpu_list(&self) -> &[Vcpu] ;
    
    fn vcpu(&self, id :usize) -> Option<&Vcpu>;

    fn has_interrupt(&self, _id: usize) -> bool;

    fn emu_has_interrupt(&self, _id: usize) -> bool;

    fn get_vgic(&self) -> &Vgic<Vcpu> ;
}


/* 定义trait */
pub trait VcpuTrait <M> {

    fn id(&self) -> usize;

    fn vm_id(&self) -> usize;

    fn phys_id(&self) -> usize;

    fn vm(&self) -> Option<Arc<M>> ;

    fn get_gpr(&self, idx: usize) -> usize;
    
    fn set_gpr(&self, idx: usize, val: usize);
}

/* 定义trait */
pub trait PcpuTrait <V> {

    fn id(&self) -> usize;
}