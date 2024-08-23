use crate::fake::Vcpu;
use crate::vgic::Vgic;
use alloc::sync::Arc;


pub trait VmTrait<V> {
    fn if_id(&self) -> usize;

    // fn if_vcpu_list(&self) -> &[V] ;
    
    // fn if_vcpu(&self, id :usize) -> Option<&V>;

    // fn if_has_interrupt(&self, _id: usize) -> bool;

    // fn if_emu_has_interrupt(&self, _id: usize) -> bool;

    // fn get_vgic(&self) -> &Vgic<Vcpu> ;
}


/* 定义trait */
pub trait VcpuTrait {

    fn if_id(&self) -> usize;

    fn if_vm_id(&self) -> usize;

    fn if_phys_id(&self) -> usize;

    // fn vm(&self) -> Option<Arc<M>> ;

    fn if_get_gpr(&self, idx: usize) -> usize;
    
    fn if_set_gpr(&mut self, idx: usize, val: usize);
}

/* 定义trait */
pub trait PcpuTrait <V> {

    fn id(&self) -> usize;
}