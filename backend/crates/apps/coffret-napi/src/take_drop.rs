use std::panic::{catch_unwind, AssertUnwindSafe};

use coffret_desktop_core::Dropped;
use napi::bindgen_prelude::*;

use crate::DropAnswer;

/// The walk of one drop, run on a thread of Node's pool rather than on the
/// shell's main thread, which a folder of a few thousand files would otherwise
/// hold up.
pub struct TakeDrop {
    pub(crate) paths: Vec<String>,
}

impl Task for TakeDrop {
    type Output = Dropped;
    type JsValue = DropAnswer;

    fn compute(&mut self) -> Result<Self::Output> {
        let desktop = crate::desktop()?;
        // A panic here is on a pool thread, outside the export that started
        // it, so it is caught here for the reason every export catches one.
        match catch_unwind(AssertUnwindSafe(|| desktop.take_drop(&self.paths))) {
            Ok(taken) => taken.map_err(crate::failed),
            Err(_) => Err(Error::from_reason("the drop could not be walked")),
        }
    }

    fn resolve(&mut self, _: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output.into())
    }
}
