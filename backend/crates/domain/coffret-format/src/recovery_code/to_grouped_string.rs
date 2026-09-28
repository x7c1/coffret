use super::RecoveryCode;

impl RecoveryCode {
    /// The code as it is printed: everything after `coffret1` in groups of
    /// four, the data characters and the checksum alike.
    ///
    /// Grouping is presentation and not part of the form — [`parse`] strips it
    /// along with any other whitespace — so a code printed this way and the
    /// same code typed back as one run of characters are one value
    /// (spec: KD-11).
    ///
    /// [`parse`]: Self::parse
    pub fn to_grouped_string(&self) -> String {
        let separator = Self::HUMAN_READABLE_PART.len() + 1;
        let (prefix, data) = self.text.split_at(separator);

        let groups = data.len().div_ceil(Self::GROUP_LEN);
        let mut grouped = String::with_capacity(self.text.len() + groups);
        grouped.push_str(prefix);
        for (index, character) in data.chars().enumerate() {
            if index % Self::GROUP_LEN == 0 {
                grouped.push(' ');
            }
            grouped.push(character);
        }
        grouped
    }
}
