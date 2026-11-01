//! The flags of a date. The parsing is in [`crate::fields`].

/// One of the flags of a date. The flag at
/// index `i` is bit `i` of the value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateFlag {
    /// Bit 0: the start date has no known day.
    StartUnknownDay,
    /// Bit 1: the start date has no known month, and so no known day.
    StartUnknownMonth,
    /// Bit 2: the end date has no known day.
    EndUnknownDay,
    /// Bit 3: the end date has no known month, and so no known day.
    EndUnknownMonth,
    /// Bit 4: the air date is in the past, the anime has ended.
    Ended,
    /// Bit 5: the start date has no known year.
    StartUnknownYear,
    /// Bit 6: the end date has no known year.
    EndUnknownYear,
}

impl DateFlag {
    pub(crate) const ALL: [Self; 7] = [
        Self::StartUnknownDay,
        Self::StartUnknownMonth,
        Self::EndUnknownDay,
        Self::EndUnknownMonth,
        Self::Ended,
        Self::StartUnknownYear,
        Self::EndUnknownYear,
    ];
}
