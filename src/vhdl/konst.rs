// Copyright (c) 2017 Fabian Schuiki

//! This module implements constant value calculation for VHDL.

use std::fmt;
use num::BigInt;
pub use hir::Dir;


/// A constant value.
#[derive(Debug)]
pub enum Const {
	Int(ConstInt),
	Float(ConstFloat),
	IntRange(ConstIntRange),
	FloatRange(ConstFloatRange),
}

impl Const {
	pub fn negate(&self) -> Const {
		match *self {
			Const::Int(ref c) => Const::Int(c.negate()),
			Const::Float(ref c) => Const::Float(c.negate()),
			Const::IntRange(_) => panic!("cannot negate integer range"),
			Const::FloatRange(_) => panic!("cannot negate float range"),
		}
	}


	/// Provide a textual description of the kind of constant.
	pub fn kind_desc(&self) -> &'static str {
		match *self {
			Const::Int(_) => "integer",
			Const::Float(_) => "float",
			Const::IntRange(_) => "integer range",
			Const::FloatRange(_) => "float range",
		}
	}
}

impl From<ConstInt> for Const {
	fn from(k: ConstInt) -> Const {
		Const::Int(k)
	}
}

impl From<ConstFloat> for Const {
	fn from(k: ConstFloat) -> Const {
		Const::Float(k)
	}
}

impl From<ConstIntRange> for Const {
	fn from(k: ConstIntRange) -> Const {
		Const::IntRange(k)
	}
}

impl From<ConstFloatRange> for Const {
	fn from(k: ConstFloatRange) -> Const {
		Const::FloatRange(k)
	}
}


/// A constant integer value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstInt {
	pub value: BigInt,
}

impl ConstInt {
	/// Create a new constant integer.
	pub fn new(value: BigInt) -> ConstInt {
		ConstInt {
			value: value
		}
	}

	pub fn negate(&self) -> ConstInt {
		ConstInt::new(-self.value.clone())
	}
}


/// A constant float value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstFloat {
}

impl ConstFloat {
	pub fn negate(&self) -> ConstFloat {
		ConstFloat{}
	}
}


/// A constant range value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstRange<T: fmt::Display + fmt::Debug> {
	pub dir: Dir,
	pub left_bound: T,
	pub right_bound: T,
}

impl<T> ConstRange<T> where T: fmt::Display + fmt::Debug {
	/// Create a new constant range.
	pub fn new(dir: Dir, left_bound: T, right_bound: T) -> ConstRange<T> {
		ConstRange {
			dir: dir,
			left_bound: left_bound,
			right_bound: right_bound,
		}
	}
}

pub type ConstIntRange = ConstRange<ConstInt>;
pub type ConstFloatRange = ConstRange<ConstFloat>;


// ----- FORMATTING ------------------------------------------------------------

impl fmt::Display for Const {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			Const::Int(ref k) => k.fmt(f),
			Const::Float(ref k) => k.fmt(f),
			Const::IntRange(ref k) => k.fmt(f),
			Const::FloatRange(ref k) => k.fmt(f),
		}
	}
}

impl fmt::Display for ConstInt {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		self.value.fmt(f)
	}
}

impl fmt::Display for ConstFloat {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "<float>")
	}
}

impl<T> fmt::Display for ConstRange<T> where T: fmt::Display + fmt::Debug {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{} {} {}", self.left_bound, self.dir, self.right_bound)
	}
}
