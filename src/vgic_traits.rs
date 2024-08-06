use crate::fake::Vm;
use alloc::sync::Arc;

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