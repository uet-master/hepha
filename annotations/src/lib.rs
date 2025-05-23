// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// A set of macros and functions to use for annotating source files that are being checked with HEPHA

#![allow(unexpected_cfgs)]

/// Provides a way to specify a value that should be treated abstractly by the verifier.
/// The concrete argument provides type information to the verifier and a meaning for
/// the expression when compiled by the rust compiler.
///
/// The expected use case for this is inside test cases. Principally this would be test cases
/// for the verifier itself, but it can also be used to "fuzz" unit tests in user code.
#[macro_export]
macro_rules! abstract_value {
    ($value:expr) => {
        if cfg!(hepha) {
            hepha_annotations::hepha_abstract_value($value)
        } else {
            $value
        }
    };
}

/// A type used to specify how tag types transfer over operations. The type is an alias of `u128`.
/// Each bit of the bit vector controls the transfer function for an operation.
/// If a bit is set to one, the corresponding operation will propagate the tag.
/// If a bit is set to zero, the corresponding operation will block the tag.
///
/// For convenience the `tag_propagation_set!` macro can be used to construct a
/// `TagPropagationSet` out of `TagPropagation`s. Also the functions
/// `add_propagation` and `remove_propagation` are provided which take a set and
/// a `TagPropagation` as input and respectively enable or disable the
/// particular propagation.
pub type TagPropagationSet = u128;

/// Enable this `TagPropagation` kind in this `TagPropagationSet`. This function
/// is `const` so you are able to call it when constructing a mask for taint
/// propagation.
///
/// This function is a no-op if this propagation is already enabled.
pub const fn add_propagation(
    set: TagPropagationSet,
    propagation: TagPropagation,
) -> TagPropagationSet {
    set | propagation.into_set()
}

/// Disable this `TagPropagation` kind in this `TagPropagationSet`. This
/// function is `const` so you are able to call it when constructing a mask for
/// taint propagation.
///
/// This function is a no-op if this propagation is already disabled.
///
/// The intended is so you can conveniently disable propagations from the set of
/// all propagations, e.g. `remove_propagation(TAG_PROPAGATION_ALL,
/// TagPropagation::Add)`.
pub const fn remove_propagation(
    set: TagPropagationSet,
    propagation: TagPropagation,
) -> TagPropagationSet {
    set & !propagation.into_set()
}

#[test]
fn test_rem_prop() {
    assert!(
        remove_propagation(
            tag_propagation_set!(TagPropagation::Add),
            TagPropagation::Add
        ) == 0
    );
    assert!(
        remove_propagation(TAG_PROPAGATION_ALL, TagPropagation::SuperComponent)
            & TagPropagation::SuperComponent.into_set()
            == 0
    )
}

#[test]
fn test_add_prop() {
    assert!(add_propagation(TAG_PROPAGATION_ALL, TagPropagation::Add) == TAG_PROPAGATION_ALL);
    assert!(
        add_propagation(0, TagPropagation::SuperComponent)
            == TagPropagation::SuperComponent.into_set()
    )
}

/// An enum type of controllable operations for HEPHA tag types.
/// In general, the result of the operation corresponding to an enum value will
/// get tagged with all of the tags of the operands.
#[derive(Ord, PartialOrd, Eq, PartialEq, Debug, Copy, Clone)]
pub enum TagPropagation {
    Add,
    AddOverflows,
    And,
    BitAnd,
    BitNot,
    BitOr,
    BitXor,
    Cast,
    Div,
    Equals,
    GreaterOrEqual,
    GreaterThan,
    IntrinsicBinary,
    IntrinsicBitVectorUnary,
    IntrinsicFloatingPointUnary,
    LessOrEqual,
    LessThan,
    LogicalNot,
    Memcmp,
    Mul,
    MulOverflows,
    Ne,
    Neg,
    Or,
    Offset,
    Rem,
    Shl,
    ShlOverflows,
    Shr,
    ShrOverflows,
    Sub,
    /// Tagging a structured value also tags all of the component values.
    SubComponent,
    SubOverflows,
    /// Tagging a value also tags any structured value that includes it.
    SuperComponent,
    Transmute,
    UninterpretedCall,
}

impl TagPropagation {
    /// Construct a singleton `TagPropagationSet` that only enables this
    /// propagation type.
    pub const fn into_set(self) -> TagPropagationSet {
        1 << (self as u8)
    }
}

/// Provide a way to create tag propagation sets. It is equivalent to bitwise-or of all its arguments.
#[macro_export]
macro_rules! tag_propagation_set {
    ($($x:expr),*) => {
        0 $(| (1 << ($x as u8)))*
    };
}
/// A tag propagation set indicating a tag is propagated by all operations.
pub const TAG_PROPAGATION_ALL: TagPropagationSet = tag_propagation_set!(
    TagPropagation::Add,
    TagPropagation::AddOverflows,
    TagPropagation::And,
    TagPropagation::BitAnd,
    TagPropagation::BitNot,
    TagPropagation::BitOr,
    TagPropagation::BitXor,
    TagPropagation::Cast,
    TagPropagation::Div,
    TagPropagation::Equals,
    TagPropagation::GreaterOrEqual,
    TagPropagation::GreaterThan,
    TagPropagation::IntrinsicBinary,
    TagPropagation::IntrinsicBitVectorUnary,
    TagPropagation::IntrinsicFloatingPointUnary,
    TagPropagation::LessOrEqual,
    TagPropagation::LessThan,
    TagPropagation::LogicalNot,
    TagPropagation::Memcmp,
    TagPropagation::Mul,
    TagPropagation::MulOverflows,
    TagPropagation::Ne,
    TagPropagation::Neg,
    TagPropagation::Or,
    TagPropagation::Offset,
    TagPropagation::Rem,
    TagPropagation::Shl,
    TagPropagation::ShlOverflows,
    TagPropagation::Shr,
    TagPropagation::ShrOverflows,
    TagPropagation::Sub,
    TagPropagation::SubComponent,
    TagPropagation::SubOverflows,
    TagPropagation::SuperComponent,
    TagPropagation::Transmute,
    TagPropagation::UninterpretedCall
);

/// Equivalent to a no op when used with an unmodified Rust compiler.
/// When compiled with HEPHA, this causes HEPHA to associate (tag) the value with the given type.
/// Typically the type will be private to a scope so that only privileged code can add the tag.
/// Once added, a tag cannot be removed and the tagged value may not be modified.
/// To determine if a value has been tagged, use the has_tag! macro.
#[macro_export]
macro_rules! add_tag {
    ($value:expr, $tag:ty) => {
        if cfg!(hepha) {
            hepha_annotations::hepha_add_tag::<_, $tag>($value)
        }
    };
}

/// Provides a way to check if a value has been tagged with a type, using the add_tag! macro.
/// When compiled with an unmodified Rust compiler, this results in true.
/// When compiled with HEPHA, this will be true if all data flows into the argument of this
/// call has gone via a call to add_tag!.
#[macro_export]
macro_rules! has_tag {
    ($value:expr, $tag:ty) => {
        if cfg!(hepha) {
            hepha_annotations::hepha_has_tag::<_, $tag>($value)
        } else {
            true
        }
    };
}

/// Provides a way to check if a value has *not* been tagged with a type using add_tag!.
/// When compiled with an unmodified Rust compiler, this results in true.
/// When compiled with HEPHA, this will be true if none data flows into the argument of this
/// call has gone via a call to add_tag!.
#[macro_export]
macro_rules! does_not_have_tag {
    ($value:expr, $tag:ty) => {
        if cfg!(hepha) {
            hepha_annotations::hepha_does_not_have_tag::<_, $tag>($value)
        } else {
            true
        }
    };
}

/// Equivalent to a no op when used with an unmodified Rust compiler.
/// When compiled with HEPHA, this causes HEPHA to assume the condition unless it can
/// prove it to be false.
#[macro_export]
macro_rules! assume {
    ($condition:expr) => {
        if cfg!(hepha) {
            hepha_annotations::hepha_assume($condition)
        }
    };
}

/// Equivalent to a no op when used with an unmodified Rust compiler.
/// When compiled with HEPHA, this causes HEPHA to assume that the preconditions of the next
/// function call have been met.
/// This is to be used when the precondition has been inferred and involves private state that
/// cannot be constrained by a normal assumption.
/// Note that it is bad style for an API to rely on preconditions that cannot be checked by the
/// caller, so this is only here for supporting legacy APIs.
#[macro_export]
macro_rules! assume_preconditions {
    () => {
        if cfg!(hepha) {
            hepha_annotations::hepha_assume_preconditions()
        }
    };
}

/// Equivalent to the standard assert! when used with an unmodified Rust compiler.
/// When compiled with HEPHA, this causes HEPHA to assume the condition unless it can
/// prove it to be false.
#[macro_export]
macro_rules! checked_assume {
    ($condition:expr) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_assume($condition)
        } else {
            assert!($condition);
        }
    );
    ($condition:expr, $($arg:tt)*) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_assume($condition);
        } else {
            assert!($condition, $($arg)*);
        }
    );
}

/// Equivalent to the standard assert_eq! when used with an unmodified Rust compiler.
/// When compiled with HEPHA, this causes HEPHA to assume the condition unless it can
/// prove it to be false.
#[macro_export]
macro_rules! checked_assume_eq {
    ($left:expr, $right:expr) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_assume($left == $right)
        } else {
            assert_eq!($left, $right);
        }
    );
    ($left:expr, $right:expr, $($arg:tt)*) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_assume($left == $right);
        } else {
            assert_eq!($left, $right, $($arg)*);
        }
    );
}

/// Equivalent to the standard assert_ne! when used with an unmodified Rust compiler.
/// When compiled with HEPHA, this causes HEPHA to assume the condition unless it can
/// prove it to be false.
#[macro_export]
macro_rules! checked_assume_ne {
    ($left:expr, $right:expr) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_assume($left != $right)
        } else {
            assert_ne!($left, $right);
        }
    );
    ($left:expr, $right:expr, $($arg:tt)*) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_assume($left != $right);
        } else {
            assert_ne!($left, $right, $($arg)*);
        }
    );
}

/// Equivalent to the standard debug_assert! when used with an unmodified Rust compiler.
/// When compiled with HEPHA, this causes HEPHA to assume the condition unless it can
/// prove it to be false.
#[macro_export]
macro_rules! debug_checked_assume {
    ($condition:expr) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_assume($condition)
        } else {
            debug_assert!($condition);
        }
    );
    ($condition:expr, $($arg:tt)*) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_assume($condition);
        } else {
            debug_assert!($condition, $($arg)*);
        }
    );
}

/// Equivalent to the standard debug_assert_eq! when used with an unmodified Rust compiler.
/// When compiled with HEPHA, this causes HEPHA to assume the condition unless it can
/// prove it to be false.
#[macro_export]
macro_rules! debug_checked_assume_eq {
    ($left:expr, $right:expr) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_assume($left == $right)
        } else {
            debug_assert_eq!($left, $right);
        }
    );
    ($left:expr, $right:expr, $($arg:tt)*) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_assume($left == $right);
        } else {
            debug_assert_eq!($left, $right, $($arg)*);
        }
    );
}

/// Equivalent to the standard debug_assert_ne! when used with an unmodified Rust compiler.
/// When compiled with HEPHA, this causes HEPHA to assume the condition unless it can
/// prove it to be false.
#[macro_export]
macro_rules! debug_checked_assume_ne {
    ($left:expr, $right:expr) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_assume($left != $right)
        } else {
            debug_assert_ne!($left, $right);
        }
    );
    ($left:expr, $right:expr, $($arg:tt)*) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_assume($left != $right);
        } else {
            debug_assert_ne!($left, $right, $($arg)*);
        }
    );
}

/// Equivalent to a no op when used with an unmodified Rust compiler.
/// When compiled with HEPHA, this causes HEPHA to verify the condition at the
/// point where it appears in a function, but to also add it a postcondition that can
/// be assumed by the caller of the function.
#[macro_export]
macro_rules! postcondition {
    ($condition:expr) => {
        #[cfg(hepha)] {
            hepha_annotations::hepha_postcondition($condition, false, "unsatisfied postcondition");
        }
    };
    ($condition:expr, $message:literal) => {
        #[cfg(hepha)] {
            hepha_annotations::hepha_postcondition($condition, false,  concat!("unsatisfied postcondition: ", $message));
        }
    };
    ($condition:expr, $($arg:tt)*) => {
        #[cfg(hepha)] {
            hepha_annotations::hepha_postcondition($condition, false,  concat!("unsatisfied postcondition: ", stringify!($($arg)*)));
        }
    };
}

/// Equivalent to a no op when used with an unmodified Rust compiler.
/// When compiled with HEPHA, this causes HEPHA to assume the condition at the
/// point where it appears in a function, but to also add it a postcondition that can
/// be assumed by the caller of the function.
#[macro_export]
macro_rules! assumed_postcondition {
    ($condition:expr) => {
        #[cfg(hepha)]
        {
            hepha_annotations::hepha_postcondition($condition, true, "")
        }
        #[cfg(not(hepha))]
        {
            debug_assert!($condition);
        }
    };
}

/// Equivalent to the standard assert! when used with an unmodified Rust compiler.
/// When compiled with HEPHA, this causes HEPHA to verify the condition at the
/// point where it appears in a function, but to also add it a postcondition that can
/// be assumed by the caller of the function.
#[macro_export]
macro_rules! checked_postcondition {
    ($condition:expr) => (
        #[cfg(hepha)] {
            hepha_annotations::hepha_postcondition($condition, false,  "unsatisfied postcondition")
        }
        #[cfg(not(hepha))] {
            assert!($condition);
        }
    );
    ($condition:expr, $message:literal) => {
        #[cfg(hepha)] {
            hepha_annotations::hepha_postcondition($condition, false,  concat!("unsatisfied postcondition: ", $message))
        }
        #[cfg(not(hepha))] {
            assert!($condition, $message);
        }
    };
    ($condition:expr, $($arg:tt)*) => {
        #[cfg(hepha)] {
            hepha_annotations::hepha_postcondition($condition, false,  concat!("unsatisfied postcondition: ", stringify!($($arg)*)));
        }
        #[cfg(not(hepha))] {
            assert!($condition, $($arg)*);
        }
    };
}

/// Equivalent to the standard assert_eq! when used with an unmodified Rust compiler.
/// When compiled with HEPHA, this causes HEPHA to verify the condition at the
/// point where it appears in a function, but to also add it a postcondition that can
/// be assumed by the caller of the function.
#[macro_export]
macro_rules! checked_postcondition_eq {
    ($left:expr, $right:expr) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_postcondition($left == $right, false,  concat!("unsatisfied postcondition: ", stringify!($left == $right)))
        } else {
            assert_eq!($left, $right);
        }
    );
    ($left:expr, $right:expr, $message:literal) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_postcondition($left == $right, false,  concat!("unsatisfied postcondition: ", stringify!($left == $right), ", ", $message))
        } else {
            assert_eq!($left, $right, $message);
        }
    );
    ($left:expr, $right:expr, $($arg:tt)*) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_postcondition($left == $right, false,  concat!("unsatisfied postcondition: ", stringify!($left == $right), ", ", stringify!($($arg)*)));
        } else {
            assert_eq!($left, $right, $($arg)*);
        }
    );
}

/// Equivalent to the standard assert_ne! when used with an unmodified Rust compiler.
/// When compiled with HEPHA, this causes HEPHA to verify the condition at the
/// point where it appears in a function, but to also add it a postcondition that can
/// be assumed by the caller of the function.
#[macro_export]
macro_rules! checked_postcondition_ne {
    ($left:expr, $right:expr) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_postcondition($left != $right, false,  concat!("unsatisfied postcondition: ", stringify!($left != $right)))
        } else {
            assert_ne!($left, $right);
        }
    );
    ($left:expr, $right:expr, $message:literal) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_postcondition($left != $right, false,  concat!("unsatisfied postcondition: ", stringify!($left != $right), ", ", $message))
        } else {
            assert_ne!($left, $right, $message);
        }
    );
    ($left:expr, $right:expr, $($arg:tt)*) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_postcondition($left != $right, false,  concat!("unsatisfied postcondition: ", stringify!($left != $right), ", ", stringify!($($arg)*)));
        } else {
            assert_ne!($left, $right, $($arg)*);
        }
    );
}

/// Equivalent to the standard debug_assert! when used with an unmodified Rust compiler.
/// When compiled with HEPHA, this causes HEPHA to verify the condition at the
/// point where it appears in a function, but to also add it a postcondition that can
/// be assumed by the caller of the function.
#[macro_export]
macro_rules! debug_checked_postcondition {
    ($condition:expr) => (
        #[cfg(hepha)] {
            hepha_annotations::hepha_postcondition($condition, false,  "unsatisfied postcondition")
        }
        #[cfg(not(hepha))] {
            debug_assert!($condition);
        }
    );
    ($condition:expr, $message:literal) => (
        #[cfg(hepha)] {
            hepha_annotations::hepha_postcondition($condition, false,  concat!("unsatisfied postcondition: ", $message))
        }
        #[cfg(not(hepha))] {
            debug_assert!($condition, $message);
        }
    );
    ($condition:expr, $($arg:tt)*) => (
        #[cfg(hepha)] {
            hepha_annotations::hepha_postcondition($condition, false,  concat!("unsatisfied postcondition: ", stringify!($($arg)*)));
        }
        #[cfg(not(hepha))] {
            debug_assert!($condition, $($arg)*);
        }
    );
}

/// Equivalent to the standard debug_assert_eq! when used with an unmodified Rust compiler.
/// When compiled with HEPHA, this causes HEPHA to verify the condition at the
/// point where it appears in a function, but to also add it a postcondition that can
/// be assumed by the caller of the function.
#[macro_export]
macro_rules! debug_checked_postcondition_eq {
    ($left:expr, $right:expr) => (
        #[cfg(hepha)] {
            hepha_annotations::hepha_postcondition($left == $right, false,  concat!("unsatisfied postcondition: ", stringify!($left == $right)))
        }
        #[cfg(not(hepha))] {
            debug_assert_eq!($left, $right);
        }
    );
    ($left:expr, $right:expr, $message:literal) => (
        #[cfg(hepha)] {
            hepha_annotations::hepha_postcondition($left == $right, false,  concat!("unsatisfied postcondition: ", stringify!($left == $right), ", ", $message))
        }
        #[cfg(not(hepha))] {
            debug_assert_eq!($left, $right, $message);
        }
    );
    ($left:expr, $right:expr, $($arg:tt)*) => (
        #[cfg(hepha)] {
            hepha_annotations::hepha_postcondition($left == $right, false,  concat!("unsatisfied postcondition: ", stringify!($left == $right), ", ", stringify!($($arg)*)));
        }
        #[cfg(not(hepha))] {
            debug_assert_eq!($left, $right, $($arg)*);
        }
    );
}

/// Equivalent to the standard debug_assert_ne! when used with an unmodified Rust compiler.
/// When compiled with HEPHA, this causes HEPHA to verify the condition at the
/// point where it appears in a function, but to also add it a postcondition that can
/// be assumed by the caller of the function.
#[macro_export]
macro_rules! debug_checked_postcondition_ne {
    ($left:expr, $right:expr) => (
        #[cfg(hepha)] {
            hepha_annotations::hepha_postcondition($left != $right, false,  concat!("unsatisfied postcondition: ", stringify!($left != $right)))
        }
        #[cfg(not(hepha))] {
            debug_assert_ne!($left, $right);
        }
    );
    ($left:expr, $right:expr, $message:literal) => (
        #[cfg(hepha)] {
            hepha_annotations::hepha_postcondition($left != $right, false,  concat!("unsatisfied postcondition: ", stringify!($left != $right), ", ", $message))
        }
        #[cfg(not(hepha))] {
            debug_assert_ne!($left, $right, $message);
        }
    );
    ($left:expr, $right:expr, $($arg:tt)*) => (
        #[cfg(hepha)] {
            hepha_annotations::hepha_postcondition($left != $right, false,  concat!("unsatisfied postcondition: ", stringify!($left != $right), ", ", stringify!($($arg)*)));
        }
        #[cfg(not(hepha))] {
            debug_assert_ne!($left, $right, $($arg)*);
        }
    );
}

/// Equivalent to a no op when used with an unmodified Rust compiler.
/// When compiled with HEPHA, this causes HEPHA to assume the condition at the
/// point where it appears in a function, but to also add it a precondition that must
/// be verified by the caller of the function.
#[macro_export]
macro_rules! precondition {
    ($condition:expr) => {
        if cfg!(hepha) {
            hepha_annotations::hepha_precondition_start();
            hepha_annotations::hepha_precondition($condition, "unsatisfied precondition")
        }
    };
    ($condition:expr, $message:literal) => {
        if cfg!(hepha) {
            hepha_annotations::hepha_precondition_start();
            hepha_annotations::hepha_precondition($condition, concat!("unsatisfied precondition: ", $message))
        }
    };
    ($condition:expr, $($arg:tt)*) => {
        if cfg!(hepha) {
            hepha_annotations::hepha_precondition_start();
            hepha_annotations::hepha_precondition($condition, concat!("unsatisfied precondition: ", stringify!($($arg)*)));
        }
    };
}

/// Equivalent to the standard assert! when used with an unmodified Rust compiler.
/// When compiled with HEPHA, this causes HEPHA to assume the condition at the
/// point where it appears in a function, but to also add it a precondition that must
/// be verified by the caller of the function.
#[macro_export]
macro_rules! checked_precondition {
    ($condition:expr) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_precondition_start();
            hepha_annotations::hepha_precondition($condition, "unsatisfied precondition")
        } else {
            assert!($condition);
        }
    );
    ($condition:expr, $message:literal) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_precondition_start();
            hepha_annotations::hepha_precondition($condition, concat!("unsatisfied precondition: ", $message))
        } else {
            assert!($condition, $message);
        }
    );
    ($condition:expr, $($arg:tt)*) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_precondition_start();
            hepha_annotations::hepha_precondition($condition, concat!("unsatisfied precondition: ", stringify!($($arg)*)));
        } else {
            assert!($condition, $($arg)*);
        }
    );
}

/// Equivalent to the standard assert_eq! when used with an unmodified Rust compiler.
/// When compiled with HEPHA, this causes HEPHA to assume the condition at the
/// point where it appears in a function, but to also add it a precondition that must
/// be verified by the caller of the function.
#[macro_export]
macro_rules! checked_precondition_eq {
    ($left:expr, $right:expr) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_precondition_start();
            hepha_annotations::hepha_precondition($left == $right, concat!("unsatisfied precondition: ", stringify!($left == $right)))
        } else {
            assert_eq!($left, $right);
        }
    );
    ($left:expr, $right:expr, $message:literal) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_precondition_start();
            hepha_annotations::hepha_precondition($left == $right, concat!("unsatisfied precondition: ", stringify!($left == $right), ", ", $message))
        } else {
            assert_eq!($left, $right, $message);
        }
    );
    ($left:expr, $right:expr, $($arg:tt)*) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_precondition_start();
            hepha_annotations::hepha_precondition($left == $right, concat!("unsatisfied precondition: ", stringify!($left == $right), ", ", stringify!($($arg)*)));
        } else {
            assert_eq!($left, $right, $($arg)*);
        }
    );
}

/// Equivalent to the standard assert_ne! when used with an unmodified Rust compiler.
/// When compiled with HEPHA, this causes HEPHA to assume the condition at the
/// point where it appears in a function, but to also add it a precondition that must
/// be verified by the caller of the function.
#[macro_export]
macro_rules! checked_precondition_ne {
    ($left:expr, $right:expr) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_precondition_start();
            hepha_annotations::hepha_precondition($left != $right, concat!("unsatisfied precondition: ", stringify!($left != $right)))
        } else {
            assert_ne!($left, $right);
        }
    );
    ($left:expr, $right:expr, $message:literal) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_precondition_start();
            hepha_annotations::hepha_precondition($left != $right, concat!("unsatisfied precondition: ", stringify!($left != $right), ", ", $message))
        } else {
            assert_ne!($left, $right, $message);
        }
    );
    ($left:expr, $right:expr, $($arg:tt)*) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_precondition_start();
            hepha_annotations::hepha_precondition($left != $right, concat!("unsatisfied precondition: ", stringify!($left != $right), ", ", stringify!($($arg)*)));
        } else {
            assert_ne!($left, $right, $($arg)*);
        }
    );
}

/// Equivalent to the standard debug_assert! when used with an unmodified Rust compiler.
/// When compiled with HEPHA, this causes HEPHA to assume the condition at the
/// point where it appears in a function, but to also add it a precondition that must
/// be verified by the caller of the function.
#[macro_export]
macro_rules! debug_checked_precondition {
    ($condition:expr) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_precondition_start();
            hepha_annotations::hepha_precondition($condition, "unsatisfied precondition")
        } else {
            debug_assert!($condition);
        }
    );
    ($condition:expr, $message:literal) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_precondition_start();
            hepha_annotations::hepha_precondition($condition, concat!("unsatisfied precondition: ", $message))
        } else {
            debug_assert!($condition, $message);
        }
    );
    ($condition:expr, $($arg:tt)*) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_precondition_start();
            hepha_annotations::hepha_precondition($condition, concat!("unsatisfied precondition: ", stringify!($($arg)*)));
        } else {
            debug_assert!($condition, $($arg)*);
        }
    );
}

/// Equivalent to the standard debug_assert_eq! when used with an unmodified Rust compiler.
/// When compiled with HEPHA, this causes HEPHA to assume the condition at the
/// point where it appears in a function, but to also add it a precondition that must
/// be verified by the caller of the function.
#[macro_export]
macro_rules! debug_checked_precondition_eq {
    ($left:expr, $right:expr) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_precondition_start();
            hepha_annotations::hepha_precondition($left == $right, concat!("unsatisfied precondition: ", stringify!($left == $right)))
        } else {
            debug_assert_eq!($left, $right);
        }
    );
    ($left:expr, $right:expr, $message:literal) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_precondition_start();
            hepha_annotations::hepha_precondition($left == $right, concat!("unsatisfied precondition: ", stringify!($left == $right), ", ", $message))
        } else {
            debug_assert_eq!($left, $right, $message);
        }
    );
    ($left:expr, $right:expr, $($arg:tt)*) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_precondition_start();
            hepha_annotations::hepha_precondition($left == $right, concat!("unsatisfied precondition: ", stringify!($left == $right), ", ", stringify!($($arg)*)));
        } else {
            debug_assert_eq!($left, $right, $($arg)*);
        }
    );
}

/// Equivalent to the standard debug_assert_ne! when used with an unmodified Rust compiler.
/// When compiled with HEPHA, this causes HEPHA to assume the condition at the
/// point where it appears in a function, but to also add it a precondition that must
/// be verified by the caller of the function.
#[macro_export]
macro_rules! debug_checked_precondition_ne {
    ($left:expr, $right:expr) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_precondition_start();
            hepha_annotations::hepha_precondition($left != $right, concat!("unsatisfied precondition: ", stringify!($left != $right)))
        } else {
            debug_assert_ne!($left, $right);
        }
    );
    ($left:expr, $right:expr, $message:literal) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_precondition_start();
            hepha_annotations::hepha_precondition($left != $right, concat!("unsatisfied precondition: ", stringify!($left != $right), ", ", $message))
        } else {
            debug_assert_ne!($left, $right, $message);
        }
    );
    ($left:expr, $right:expr, $($arg:tt)*) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_precondition_start();
            hepha_annotations::hepha_precondition($left != $right, concat!("unsatisfied precondition: ", stringify!($left != $right), ", ", stringify!($($arg)*)));
        } else {
            debug_assert_ne!($left, $right, $($arg)*);
        }
    );
}

/// Terminates the program with a panic that is tagged as being an unrecoverable error.
/// Use this for errors that arise in correct programs due to external factors.
/// For example, if a file that is essential for running cannot be found for some reason.
#[macro_export]
macro_rules! unrecoverable {
    ($fmt:expr) => (
        panic!(concat!("unrecoverable: ", stringify!($fmt)));
    );
    ($fmt:expr, $($arg:tt)+) => (
        panic!(concat!("unrecoverable: ", stringify!($fmt)), $($arg)+);
    );
}

/// Equivalent to a no op when used with an unmodified Rust compiler.
/// When compiled with HEPHA, this causes HEPHA to check the condition and
/// emit a diagnostic unless it can prove it to be true.
#[macro_export]
macro_rules! verify {
    ($condition:expr) => {
        if cfg!(hepha) {
            hepha_annotations::hepha_verify($condition, "false verification condition")
        }
    };
}

/// Equivalent to the standard assert! when used with an unmodified Rust compiler.
/// When compiled with HEPHA, this causes HEPHA to check the condition and
/// emit a diagnostic unless it can prove it to be true.
#[macro_export]
macro_rules! checked_verify {
    ($condition:expr) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_verify($condition, "false verification condition")
        } else {
            assert!($condition);
        }
    );
    ($condition:expr, $message:literal) => {
        if cfg!(hepha) {
            hepha_annotations::hepha_verify($condition, concat!("false verification condition: ", $message))
        } else {
            assert!($condition, $message);
        }
    };
    ($condition:expr, $($arg:tt)*) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_verify($condition,  concat!("false verification condition: ", stringify!($($arg)*)));
        } else {
            assert!($condition, $($arg)*);
        }
    );
}

/// Equivalent to the standard assert_eq! when used with an unmodified Rust compiler.
/// When compiled with HEPHA, this causes HEPHA to check the condition and
/// emit a diagnostic unless it can prove it to be true.
#[macro_export]
macro_rules! checked_verify_eq {
    ($left:expr, $right:expr) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_verify($left == $right, concat!("false verification condition: ", stringify!($left == $right)))
        } else {
            assert_eq!($left, $right);
        }
    );
    ($left:expr, $right:expr, $message:literal) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_verify($left == $right, concat!("false verification condition: ", stringify!($left == $right), ", ", $message))
        } else {
            assert_eq!($left, $right, $message);
        }
    );
    ($left:expr, $right:expr, $($arg:tt)*) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_verify($left == $right, concat!("false verification condition: ", stringify!($left == $right), ", ", stringify!($($arg)*)));
        } else {
            assert_eq!($left, $right, $($arg)*);
        }
    );
}

/// Equivalent to the standard assert_eq! when used with an unmodified Rust compiler.
/// When compiled with HEPHA, this causes HEPHA to check the condition and
/// emit a diagnostic unless it can prove it to be true.
#[macro_export]
macro_rules! checked_verify_ne {
    ($left:expr, $right:expr) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_verify($left != $right, concat!("false verification condition: ", stringify!($left != $right)))
        } else {
            assert_ne!($left, $right);
        }
    );
    ($left:expr, $right:expr, $message:literal) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_verify($left != $right, concat!("false verification condition: ", stringify!($left != $right), ", ", $message))
        } else {
            assert_ne!($left, $right, $message);
        }
    );
    ($left:expr, $right:expr, $($arg:tt)*) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_verify($left != $right, concat!("false verification condition: ", stringify!($left != $right), ", ", stringify!($($arg)*)));
        } else {
            assert_ne!($left, $right, $($arg)*);
        }
    );
}

/// Equivalent to the standard debug_assert! when used with an unmodified Rust compiler.
/// When compiled with HEPHA, this causes HEPHA to check the condition and
/// emit a diagnostic unless it can prove it to be true.
#[macro_export]
macro_rules! debug_checked_verify {
    ($condition:expr) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_verify($condition, "false verification condition")
        } else {
            debug_assert!($condition);
        }
    );
    ($condition:expr, $message:literal) => {
        if cfg!(hepha) {
            hepha_annotations::hepha_verify($condition, concat!("false verification condition: ", $message))
        } else {
            debug_assert!($condition, $message);
        }
    };
    ($condition:expr, $($arg:tt)*) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_verify($condition,  concat!("false verification condition: ", stringify!($($arg)*)));
        } else {
            debug_assert!($condition, $($arg)*);
        }
    );
}

/// Equivalent to the standard debug_assert_eq! when used with an unmodified Rust compiler.
/// When compiled with HEPHA, this causes HEPHA to check the condition and
/// emit a diagnostic unless it can prove it to be true.
#[macro_export]
macro_rules! debug_checked_verify_eq {
    ($left:expr, $right:expr) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_verify($left == $right, concat!("false verification condition: ", stringify!($left == $right)))
        } else {
            debug_assert_eq!($left, $right);
        }
    );
    ($left:expr, $right:expr, $message:literal) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_verify($left == $right, concat!("false verification condition: ", stringify!($left == $right), ", ", $message))
        } else {
            debug_assert_eq!($left, $right, $message);
        }
    );
    ($left:expr, $right:expr, $($arg:tt)*) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_verify($left == $right, concat!("false verification condition: ", stringify!($left == $right), ", ", stringify!($($arg)*)));
        } else {
            debug_assert_eq!($left, $right, $($arg)*);
        }
    );
}

/// Equivalent to the standard debug_assert_ne! when used with an unmodified Rust compiler.
/// When compiled with HEPHA, this causes HEPHA to check the condition and
/// emit a diagnostic unless it can prove it to be true.
#[macro_export]
macro_rules! debug_checked_verify_ne {
    ($left:expr, $right:expr) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_verify($left != $right, concat!("false verification condition: ", stringify!($left != $right)))
        } else {
            debug_assert_ne!($left, $right);
        }
    );
    ($left:expr, $right:expr, $message:literal) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_verify($left != $right, concat!("false verification condition: ", stringify!($left != $right), ", ", $message))
        } else {
            debug_assert_ne!($left, $right, $message);
        }
    );
    ($left:expr, $right:expr, $($arg:tt)*) => (
        if cfg!(hepha) {
            hepha_annotations::hepha_verify($left != $right, concat!("false verification condition: ", stringify!($left != $right), ", ", stringify!($($arg)*)));
        } else {
            debug_assert_ne!($left, $right, $($arg)*);
        }
    );
}

/// Retrieves the value of the specified model field, or the given default value if the model field
/// is not set.
/// This function has no meaning outside of a verification
/// condition and should not be used with checked or debug_checked conditions.
/// For example: precondition!(get_model_field!(x, f) > 1).
#[macro_export]
macro_rules! get_model_field {
    ($target:expr, $field_name:ident, $default_value:expr) => {
        hepha_annotations::hepha_get_model_field($target, stringify!($field_name), $default_value)
    };
}

/// Provides a way to refer to the result value of an abstract or contract function without
/// specifying an actual value anywhere.
/// This macro expands to unimplemented!() unless the program is compiled with HEPHA.
/// It result should therefore not be assigned to a variable unless the assignment is contained
/// inside a specification macro argument list.
/// It may, however, be the return value of the function, which should never be called and
/// therefore unimplemented!() is the right behavior for it at runtime.
#[macro_export]
macro_rules! result {
    () => {
        if cfg!(hepha) {
            hepha_annotations::hepha_result()
        } else {
            unimplemented!()
        }
    };
}

/// Sets the value of the specified model field.
/// A model field does not exist at runtime and is invisible to the Rust compiler.
/// This macro expands to nothing unless the program is compiled with HEPHA.
#[macro_export]
macro_rules! set_model_field {
    ($target:expr, $field_name:ident, $value:expr) => {
        if cfg!(hepha) {
            hepha_annotations::hepha_set_model_field($target, stringify!($field_name), $value);
        }
    };
}

/// Equivalent to unreachable! when used with an unmodified Rust compiler.
/// When compiled with HEPHA, this causes HEPHA to assume that the annotation statement cannot be reached.
#[macro_export]
macro_rules! assume_unreachable {
    () => {
        if cfg!(hepha) {
            unreachable!()
        } else {
            unreachable!()
        }
    };
    ($message:literal) => {
        if cfg!(hepha) {
            unreachable!()
        } else {
            unreachable!($message)
        }
    };
    ($msg:expr,) => ({
        if cfg!(hepha) {
            unreachable!()
        } else {
            unreachable!($msg)
        }
    });
    ($fmt:expr, $($arg:tt)*) => {
        if cfg!(hepha) {
            unreachable!()
        } else {
            unreachable!($fmt, $($arg)*)
        }
    };
}

/// Equivalent to unreachable! when used with an unmodified Rust compiler.
/// When compiled with HEPHA, this causes HEPHA to verify that the annotation statement cannot be reached.
#[macro_export]
macro_rules! verify_unreachable {
    () => {
        if cfg!(hepha) {
            panic!("statement is reachable");
        } else {
            unreachable!()
        }
    };
    ($message:literal) => {
        if cfg!(hepha) {
            panic!($message);
        } else {
            unreachable!($message)
        }
    };
    ($msg:expr) => ({
        if cfg!(hepha) {
            panic!($message)
        } else {
            unreachable!($msg)
        }
    });
    ($fmt:expr, $($arg:tt)*) => {
        if cfg!(hepha) {
            panic!($fmt, $($arg)*);
        } else {
            unreachable!($fmt, $($arg)*)
        }
    };
}

// Helper function for HEPHA. Should only be called via the abstract_value! macro.
#[doc(hidden)]
pub fn hepha_abstract_value<T>(x: T) -> T {
    x
}

// Helper function for HEPHA. Should only be called via the add_tag! macro.
#[doc(hidden)]
pub fn hepha_add_tag<V: ?Sized, T>(_v: &V) {}

// Helper function for HEPHA. Should only be called via the has_tag! macro.
#[doc(hidden)]
pub fn hepha_has_tag<V: ?Sized, T>(_v: &V) -> bool {
    false
}

// Helper function for HEPHA. Should only be called via the does_not_have_tag! macro.
#[doc(hidden)]
pub fn hepha_does_not_have_tag<V: ?Sized, T>(_v: &V) -> bool {
    false
}

// Helper function for HEPHA. Should only be called via the assume macros.
#[doc(hidden)]
pub fn hepha_assume(_condition: bool) {}

// Helper function for HEPHA. Should only be called via the assume_precondition macro.
#[doc(hidden)]
pub fn hepha_assume_preconditions() {}

// Helper function for HEPHA. Should only be called via the postcondition macros.
#[doc(hidden)]
pub fn hepha_postcondition(_condition: bool, _assumed: bool, _message: &str) {}

// Helper function for HEPHA. Should only be called via the precondition macros.
#[doc(hidden)]
pub fn hepha_precondition_start() {}

// Helper function for HEPHA. Should only be called via the precondition macros.
#[doc(hidden)]
pub fn hepha_precondition(_condition: bool, _message: &str) {}

// Helper function for HEPHA. Should only be called via the verify macros.
#[doc(hidden)]
pub fn hepha_verify(_condition: bool, _message: &str) {}

// Helper function for HEPHA. Should only be called via the get_model_field macro.
#[doc(hidden)]
pub fn hepha_get_model_field<T, V>(_target: T, _field_name: &str, default_value: V) -> V {
    default_value
}

// Helper function for HEPHA. Should only be called via the result! macro.
#[doc(hidden)]
pub fn hepha_result<T>() -> T {
    unreachable!()
}

// Helper function for HEPHA. Should only be called via the set_model_field macro.
#[doc(hidden)]
pub fn hepha_set_model_field<T, V>(_target: T, _field_name: &str, _value: V) {}
