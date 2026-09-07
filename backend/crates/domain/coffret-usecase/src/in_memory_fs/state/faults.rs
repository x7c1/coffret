use std::io;
use std::path::Path;

use crate::in_memory_fs::state::State;
use crate::local_io_error::LocalIoError;
use crate::local_operation::LocalOperation;

/// One scripted refusal: the `nth` invocation of `operation` fails.
#[derive(Debug)]
pub(super) struct Fault {
    operation: LocalOperation,
    nth: usize,
}

impl State {
    /// Scripts the `nth` (1-based) invocation of `operation` to fail.
    pub(in crate::in_memory_fs) fn fail_on(&mut self, operation: LocalOperation, nth: usize) {
        self.script.push(Fault { operation, nth });
    }

    /// Counts one invocation of `operation`, and refuses it where the script
    /// says this is the one.
    ///
    /// Every operation goes through here before it does anything, so a refusal
    /// leaves the fake exactly as the failure it stands for would: a creation
    /// that fails creates nothing, a write that fails writes nothing, a flush
    /// that fails leaves the bytes already written where they are.
    pub(in crate::in_memory_fs) fn attempt(
        &mut self,
        operation: LocalOperation,
        path: &Path,
    ) -> Result<(), LocalIoError> {
        let attempt = self.attempts.entry(code(operation)).or_default();
        *attempt += 1;
        let refused = self
            .script
            .iter()
            .any(|fault| code(fault.operation) == code(operation) && fault.nth == *attempt);
        if refused {
            return Err(LocalIoError::new(
                operation,
                path,
                io::Error::other("the fake filesystem was told to refuse this"),
            ));
        }
        Ok(())
    }
}

/// Which counter one operation is tallied under.
///
/// [`LocalOperation`] is deliberately not [`Ord`] or [`Hash`] — it is a word for
/// a person, not a key — so the fake gives it one here. The match is exhaustive
/// on purpose: an operation added to the vocabulary is one this fake has to be
/// told what to do with rather than one that silently shares a counter.
fn code(operation: LocalOperation) -> u8 {
    match operation {
        LocalOperation::Listing => 0,
        LocalOperation::Stating => 1,
        LocalOperation::Reading => 2,
        LocalOperation::Creating => 3,
        LocalOperation::Writing => 4,
        LocalOperation::Flushing => 5,
        LocalOperation::Stamping => 6,
        LocalOperation::Renaming => 7,
        LocalOperation::Removing => 8,
    }
}
