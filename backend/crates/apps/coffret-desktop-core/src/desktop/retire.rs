use super::Desktop;

impl Desktop {
    /// Discards every browser session, launch ticket and door token: what the
    /// shell does as it quits (spec: LA-18, LA-23).
    pub fn retire(&self) {
        self.shared.retire();
    }
}
