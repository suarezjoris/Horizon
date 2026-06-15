use std::sync::{Arc, atomic::AtomicBool, Mutex};
use std::collections::HashMap;

pub struct ArmataState {
    pub running_flags: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl ArmataState {
    pub fn new() -> Self {
        Self {
            running_flags: Mutex::new(HashMap::new()),
        }
    }
}
