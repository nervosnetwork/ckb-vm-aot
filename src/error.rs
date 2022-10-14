#[derive(Debug, PartialEq, Clone, Eq, Display)]
pub enum AotError {
    #[display(fmt = "aot error: dynasm ret {}", "_0")]
    Dynasm(i32),
    #[display(fmt = "aot error: section is empty")]
    SectionIsEmpty,
    #[display(fmt = "aot error: section overlaps with another")]
    SectionOverlaps,
    #[display(fmt = "aot error: limit reached maximum dummy sections")]
    LimitReachedMaximumDummySections,
    #[display(fmt = "aot error: limit reached maximum labels")]
    LimitReachedMaximumLabels,
    #[display(fmt = "aot error: limit reached maximum sections")]
    LimitReachedMaximumSections,
    #[display(fmt = "aot error: limit reached maximum temp register")]
    LimitReachedMaximumTempRegisters,
    #[display(fmt = "aot error: out of bound due to not start of basic block")]
    OutOfBoundDueToNotStartOfBasicBlock,
}

impl std::error::Error for AotError {}
