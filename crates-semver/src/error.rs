use common::FmtStr;

use crate::{Offset, NumField};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidChar(char, Offset),
    TrailingCharacters(FmtStr, Offset),
    MissingField(NumField, Offset),
    LeadingZero(NumField, Offset),
    InvalidIntChar(char, NumField, Offset),
    IntOverflow(NumField, Offset, u32),
    ExpectedDot(char, NumField, Offset),
    MissingDot(NumField, Offset),
}
