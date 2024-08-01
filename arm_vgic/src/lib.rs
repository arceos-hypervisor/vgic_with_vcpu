
pub struct VgicGlobal {
    nr_lr: u32,
    mainten_irq: u32,
    max_gic_vcpus: u32,
}

use std::sync::Mutex;

lazy_static! {
    static ref VGG: Mutex<Option<VgicGlobal>> = Mutex::new(None);
}

impl VgicGlobal {
    pub fn new(__vgg: VgicGlobal) {
        let mut vgg = VGG.lock().unwrap();
        *vgg = Some(__vgg);
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test1() -> Result<(), String> {
        Err(String::from("test1 failed"))
    }
}