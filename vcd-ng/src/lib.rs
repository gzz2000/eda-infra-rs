//! This crate reads and writes [VCD (Value Change Dump)][wp] files, a common format used with
//! logic analyzers, HDL simulators, and other EDA tools.
//!
//! [wp]: https://en.wikipedia.org/wiki/Value_change_dump
//!
//! ## Example
//!
//! ```
//! use std::io;
//! use std::io::ErrorKind::InvalidInput;
//! use vcd_ng::{ self, Value, TimescaleUnit, SimulationCommand };
//!
//! /// Write out a clocked signal to a VCD file
//! fn write_clocked_vcd(shift_reg: u32, w: &mut impl io::Write) -> io::Result<()> {
//!   let mut writer = vcd_ng::Writer::new(w);
//!
//!   // Write the header
//!   writer.timescale(1, TimescaleUnit::US)?;
//!   writer.add_module("top")?;
//!   let clock = writer.add_wire(1, "clock")?;
//!   let data = writer.add_wire(1, "data")?;
//!   writer.upscope()?;
//!   writer.enddefinitions()?;
//!
//!   // Write the initial values
//!   writer.begin(SimulationCommand::Dumpvars)?;
//!   writer.change_scalar(clock, Value::V0)?;
//!   writer.change_scalar(data, Value::V0)?;
//!   writer.end()?;
//!
//!   // Write the data values
//!   let mut t = 0;
//!   for i in 0..32 {
//!     t += 4;
//!     writer.timestamp(t)?;
//!     writer.change_scalar(clock, Value::V1)?;
//!     writer.change_scalar(data, ((shift_reg >> i) & 1) != 0)?;
//!
//!     t += 4;
//!     writer.timestamp(t)?;
//!     writer.change_scalar(clock, Value::V0)?;
//!   }
//!   Ok(())
//! }
//!
//! /// Parse a VCD file containing a clocked signal and decode the signal
//! fn read_clocked_vcd(r: &mut impl io::Read) -> io::Result<u32> {
//!    let mut parser = vcd_ng::Parser::new(r);
//!
//!    // Parse the header and find the wires
//!    let header = parser.parse_header()?;
//!    let clock = header.find_var(&["top", "clock"])
//!       .ok_or_else(|| io::Error::new(InvalidInput, "no wire top.clock"))?.code;
//!    let data = header.find_var(&["top", "data"])
//!       .ok_or_else(|| io::Error::new(InvalidInput, "no wire top.data"))?.code;
//!
//!    // Iterate through the remainder of the file and decode the data
//!    let mut shift_reg = 0;
//!    let mut data_val = Value::X;
//!    let mut clock_val = Value::X;
//!
//!    for command_result in parser {
//!      use vcd_ng::Command::*;
//!      let command = command_result?;
//!      match command {
//!        ChangeScalar(i, v) if i == clock => {
//!          if clock_val == Value::V1 && v == Value::V0 { // falling edge on clock
//!             let shift_bit = match data_val { Value::V1 => (1 << 31), _ => 0 };
//!             shift_reg = (shift_reg >> 1) | shift_bit;
//!          }
//!          clock_val = v;
//!        }
//!        ChangeScalar(i, v) if i == data => {
//!          data_val = v;
//!        }
//!        _ => (),
//!      }
//!    }
//!
//!    Ok(shift_reg)
//! }
//!
//! let mut buf = Vec::new();
//! let data = 0xC0DE1234;
//! write_clocked_vcd(data, &mut buf).expect("Failed to write");
//! let value = read_clocked_vcd(&mut &buf[..]).expect("Failed to read");
//! assert_eq!(value, data);
//! ```

use std::error::Error;
use std::fmt::{self, Display};
use std::io;
use std::str::FromStr;
use compact_str::CompactString;
use bitvec::prelude::*;

mod read;
pub use read::Parser;

mod write;
pub use write::Writer;

mod idcode;
pub use idcode::IdCode;

mod fastflow;
pub use fastflow::{ FastFlow, FastFlowToken, FFValueChange };

/// Error wrapping a static string message explaining why parsing failed.
#[derive(Debug)]
pub struct InvalidData(&'static str);
impl Display for InvalidData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl Error for InvalidData {
    fn description(&self) -> &str {
        self.0
    }
}
impl From<InvalidData> for io::Error {
    fn from(e: InvalidData) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, e.0)
    }
}

/// A unit of time for the `$timescale` command.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TimescaleUnit {
    S,
    MS,
    US,
    NS,
    PS,
    FS,
}

impl FromStr for TimescaleUnit {
    type Err = InvalidData;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use TimescaleUnit::*;
        match s {
            "s" => Ok(S),
            "ms" => Ok(MS),
            "us" => Ok(US),
            "ns" => Ok(NS),
            "ps" => Ok(PS),
            "fs" => Ok(FS),
            _ => Err(InvalidData("invalid timescale unit")),
        }
    }
}

impl Display for TimescaleUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            self.as_str()
        )
    }
}

impl TimescaleUnit {
    pub fn divisor(&self) -> u64 {
        use TimescaleUnit::*;
        match *self {
            S => 1,
            MS => 1_000,
            US => 1_000_000,
            NS => 1_000_000_000,
            PS => 1_000_000_000_000,
            FS => 1_000_000_000_000_000,
        }
    }

    pub fn fraction(&self) -> f64 {
        1.0 / (self.divisor() as f64)
    }

    pub fn as_str(&self) -> &'static str {
        use TimescaleUnit::*;
        match *self {
            S => "s",
            MS => "ms",
            US => "us",
            NS => "ns",
            PS => "ps",
            FS => "fs"
        }
    }
}

/// A four-valued logic scalar value.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Value {
    /// Logic low (prefixed with `V` to make a valid Rust identifier)
    V0 = 0,

    /// Logic high (prefixed with `V` to make a valid Rust identifier)
    V1 = 1,

    /// An uninitialized or unknown value
    X = 2,

    /// The "high-impedance" value
    Z = 3,
}

impl Value {
    #[inline]
    fn parse(v: u8) -> Result<Value, InvalidData> {
        use Value::*;
        match v {
            b'0' => Ok(V0),
            b'1' => Ok(V1),
            b'x' | b'X' => Ok(X),
            b'z' | b'Z' => Ok(Z),
            _ => Err(InvalidData("invalid VCD value")),
        }
    }

    #[inline]
    fn from_01xz(is_01: bool, is_xz: bool) -> Value {
        match (is_01, is_xz) {
            (false, false) => Value::V0,
            (true, false) => Value::V1,
            (false, true) => Value::X,
            (true, true) => Value::Z,
        }
    }

    #[inline]
    fn as_01xz(self) -> (bool, bool) {
        let value = self as u8;
        ((value & 1) != 0, (value >> 1 & 1) != 0)
    }
}

impl FromStr for Value {
    type Err = InvalidData;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Value::parse(*s.as_bytes().get(0).unwrap_or(&b' '))
    }
}

impl From<bool> for Value {
    /// `true` converts to `V1`, `false` to `V0`
    #[inline]
    fn from(v: bool) -> Value {
        if v {
            Value::V1
        } else {
            Value::V0
        }
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Value::*;
        write!(
            f,
            "{}",
            match *self {
                V0 => "0",
                V1 => "1",
                X => "x",
                Z => "z",
            }
        )
    }
}

/// A fixed box of [`Value`]s with bit compression. It is more efficient than `Vec<Value>`.
#[derive(PartialEq, Clone)]
pub struct VecValue {
    is_01: BitVec,
    is_xz: BitVec
}

/// An iterator over bits in a [`VecValue`].
pub struct VecValueIter<'i> {
    is_01_iter: bitvec::slice::BitValIter<'i, usize, Lsb0>,
    is_xz_iter: bitvec::slice::BitValIter<'i, usize, Lsb0>
}

impl<'i> Iterator for VecValueIter<'i> {
    type Item = Value;

    #[inline]
    fn next(&mut self) -> Option<Value> {
        let is_01 = self.is_01_iter.next()?;
        let is_xz = self.is_xz_iter.next()?;
        Some(Value::from_01xz(is_01, is_xz))
    }
}

impl<'i> IntoIterator for &'i VecValue {
    type Item = Value;
    type IntoIter = VecValueIter<'i>;

    #[inline]
    fn into_iter(self) -> VecValueIter<'i> {
        // VecValueIter { i: 0, v: self }
        VecValueIter {
            is_01_iter: self.is_01.iter().by_vals(),
            is_xz_iter: self.is_xz.iter().by_vals()
        }
    }
}

impl std::fmt::Debug for VecValue {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "VecValue(")?;
        for value in self {
            write!(f, "{}", value)?;
        }
        write!(f, ")")
    }
}

impl VecValue {
    /// Create a new empty vector of values
    #[inline]
    pub fn new() -> VecValue {
        VecValue {
            is_01: BitVec::new(),
            is_xz: BitVec::new()
        }
    }

    /// Create a new vector with the same value repeated for `len` times.
    #[inline]
    pub fn repeat(value: Value, len: usize) -> VecValue {
        let (is_01, is_xz) = value.as_01xz();
        VecValue {
            is_01: BitVec::repeat(is_01, len),
            is_xz: BitVec::repeat(is_xz, len)
        }
    }

    /// Get a bit from the vector
    #[inline]
    pub fn get_bit(&self, i: usize) -> Value {
        Value::from_01xz(self.is_01[i], self.is_xz[i])
    }

    /// Set a bit in the vector
    #[inline]
    pub fn set_bit(&mut self, i: usize, value: Value) {
        let (is_01, is_xz) = value.as_01xz();
        self.is_01.set(i, is_01);
        self.is_xz.set(i, is_xz);
    }

    /// Set a range of bits in the vector
    #[inline]
    pub fn set_bits(&mut self, start: usize, vvalue: &VecValue) {
        let len = vvalue.len();
        self.is_01[start..start + len].copy_from_bitslice(&vvalue.is_01);
        self.is_xz[start..start + len].copy_from_bitslice(&vvalue.is_xz);
    }

    /// Get the bits that are different in the vector
    #[inline]
    pub fn difference(&self, start: usize, vvalue: &VecValue) -> BitVec {
        let len = vvalue.len();
        let mut x01 = vvalue.is_01.clone();
        let mut xxz = vvalue.is_xz.clone();
        x01 ^= &self.is_01[start..start + len];
        xxz ^= &self.is_xz[start..start + len];
        x01 |= &xxz;
        x01
    }

    /// Get the length of the vector
    #[inline]
    pub fn len(&self) -> usize {
        self.is_01.len()
    }

    /// Push a value to the tail of the vector.
    #[inline]
    pub fn push(&mut self, value: Value) {
        let (is_01, is_xz) = value.as_01xz();
        self.is_01.push(is_01);
        self.is_xz.push(is_xz);
    }

    /// Get immutable iterator into bit content.
    #[inline]
    pub fn iter(&self) -> VecValueIter {
        (&self).into_iter()
    }
}

impl From<Vec<Value>> for VecValue {
    fn from(vec: Vec<Value>) -> Self {
        let mut ret = VecValue::new();
        for v in vec {
            ret.push(v);
        }
        ret
    }
}

/// A type of scope, as used in the `$scope` command.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[non_exhaustive]
pub enum ScopeType {
    Module,
    Task,
    Function,
    Begin,
    Fork,
}

impl FromStr for ScopeType {
    type Err = InvalidData;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use ScopeType::*;
        match s {
            "module" => Ok(Module),
            "task" => Ok(Task),
            "function" => Ok(Function),
            "begin" => Ok(Begin),
            "fork" => Ok(Fork),
            _ => Err(InvalidData("invalid scope type")),
        }
    }
}

impl Display for ScopeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use ScopeType::*;
        write!(
            f,
            "{}",
            match *self {
                Module => "module",
                Task => "task",
                Function => "function",
                Begin => "begin",
                Fork => "fork",
            }
        )
    }
}

/// A type of variable, as used in the `$var` command.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[non_exhaustive]
pub enum VarType {
    Event,
    Integer,
    Parameter,
    Real,
    Reg,
    Supply0,
    Supply1,
    Time,
    Tri,
    TriAnd,
    TriOr,
    TriReg,
    Tri0,
    Tri1,
    WAnd,
    Wire,
    WOr,
    String,
}

impl FromStr for VarType {
    type Err = InvalidData;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use VarType::*;
        match s {
            "event" => Ok(Event),
            "integer" => Ok(Integer),
            "parameter" => Ok(Parameter),
            "real" => Ok(Real),
            "reg" => Ok(Reg),
            "supply0" => Ok(Supply0),
            "supply1" => Ok(Supply1),
            "time" => Ok(Time),
            "tri" => Ok(Tri),
            "triand" => Ok(TriAnd),
            "trior" => Ok(TriOr),
            "trireg" => Ok(TriReg),
            "tri0" => Ok(Tri0),
            "tri1" => Ok(Tri1),
            "wand" => Ok(WAnd),
            "wire" => Ok(Wire),
            "wor" => Ok(WOr),
            "string" => Ok(String),
            _ => Err(InvalidData("invalid variable type")),
        }
    }
}

impl Display for VarType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use VarType::*;
        write!(
            f,
            "{}",
            match *self {
                Event => "event",
                Integer => "integer",
                Parameter => "parameter",
                Real => "real",
                Reg => "reg",
                Supply0 => "supply0",
                Supply1 => "supply1",
                Time => "time",
                Tri => "tri",
                TriAnd => "triand",
                TriOr => "trior",
                TriReg => "trireg",
                Tri0 => "tri0",
                Tri1 => "tri1",
                WAnd => "wand",
                Wire => "wire",
                WOr => "wor",
                String => "string",
            }
        )
    }
}

/// Information on a VCD scope as represented by a `$scope` command and its children.
#[derive(Debug, Clone, PartialEq)]
pub struct Scope {
    pub scope_type: ScopeType,
    pub identifier: CompactString,
    pub children: Vec<ScopeItem>,
}

impl Scope {
    /// Looks up a variable by reference.
    pub fn find_var<'a>(&'a self, reference: &str) -> Option<&'a Var> {
        for c in &self.children {
            if let &ScopeItem::Var(ref v) = c {
                if v.reference == reference {
                    return Some(v);
                }
            }
        }
        None
    }
}

impl Default for Scope {
    fn default() -> Scope {
        Scope {
            scope_type: ScopeType::Module,
            identifier: "".into(),
            children: Vec::new(),
        }
    }
}

/// Index of a VCD variable reference, either a bit select index `[i]` or a range index `[msb:lsb]`
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ReferenceIndex {
    BitSelect(i32),
    Range(i32, i32),
}

impl FromStr for ReferenceIndex {
    type Err = std::io::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use std::io::{Error, ErrorKind};
        use ReferenceIndex::*;
        let s = s.trim_start_matches('[').trim_end_matches(']');
        match s.find(':') {
            Some(idx) => {
                let msb: i32 = s[..idx]
                    .trim()
                    .parse()
                    .map_err(|e| Error::new(ErrorKind::InvalidData, e))?;
                let lsb: i32 = s[idx..]
                    .trim_start_matches(':')
                    .trim()
                    .parse()
                    .map_err(|e| Error::new(ErrorKind::InvalidData, e))?;
                Ok(Range(msb, lsb))
            }
            None => {
                let idx = s
                    .trim()
                    .parse()
                    .map_err(|e| Error::new(ErrorKind::InvalidData, e))?;
                Ok(BitSelect(idx))
            }
        }
    }
}

impl Display for ReferenceIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use ReferenceIndex::*;
        match self {
            BitSelect(idx) => write!(f, "[{}]", idx)?,
            Range(msb, lsb) => write!(f, "[{}:{}]", msb, lsb)?,
        };
        Ok(())
    }
}

/// Information on a VCD variable as represented by a `$var` command.
#[derive(Debug, Clone, PartialEq)]
pub struct Var {
    pub var_type: VarType,
    pub size: u32,
    pub code: IdCode,
    pub reference: CompactString,
    pub index: Option<ReferenceIndex>,
}

/// An item in a scope -- either a child scope or a variable.
#[derive(Debug, Clone, PartialEq)]
pub enum ScopeItem {
    Scope(Scope),
    Var(Var),
    Comment(CompactString),
}

/// An element in a VCD file.
#[derive(Debug, PartialEq, Clone)]
#[non_exhaustive]
pub enum Command {
    /// A `$comment` command
    Comment(CompactString),

    /// A `$date` command
    Date(CompactString),

    /// A `$version` command
    Version(CompactString),

    /// A `$timescale` command
    Timescale(u32, TimescaleUnit),

    /// A `$scope` command
    ScopeDef(ScopeType, CompactString),

    /// An `$upscope` command
    Upscope,

    /// A `$var` command
    VarDef(VarType, u32, IdCode, CompactString, Option<ReferenceIndex>),

    /// An `$enddefinitions` command
    Enddefinitions,

    /// A `#xxx` timestamp
    Timestamp(u64),

    /// A `0a` change to a scalar variable
    ChangeScalar(IdCode, Value),

    /// A `b0000 a` change to a vector variable
    ChangeVector(IdCode, VecValue),

    /// A `r1.234 a` change to a real variable
    ChangeReal(IdCode, f64),

    /// A `sSTART a` change to a (real?) variable
    ChangeString(IdCode, CompactString),

    /// A beginning of a simulation command. Unlike header commands, which are parsed atomically,
    /// simulation commands emit a Begin, followed by the data changes within them, followed by
    /// End.
    Begin(SimulationCommand),

    /// An end of a simulation command.
    End(SimulationCommand),
}

/// A simulation command type, used in `Command::Begin` and `Command::End`.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[non_exhaustive]
pub enum SimulationCommand {
    Dumpall,
    Dumpoff,
    Dumpon,
    Dumpvars,
}

impl Display for SimulationCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use SimulationCommand::*;
        write!(
            f,
            "{}",
            match *self {
                Dumpall => "dumpall",
                Dumpoff => "dumpoff",
                Dumpon => "dumpon",
                Dumpvars => "dumpvars",
            }
        )
    }
}

/// Structure containing the data from the header of a VCD file.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct Header {
    pub comment: Option<CompactString>,
    pub date: Option<CompactString>,
    pub version: Option<CompactString>,
    pub timescale: Option<(u32, TimescaleUnit)>,
    pub items: Vec<ScopeItem>,
}

impl Header {
    /// Find the scope object at a specified path.
    ///
    /// ## Example
    ///
    /// ```rust
    /// let mut parser = vcd_ng::Parser::new(&b"
    /// $scope module a $end
    /// $scope module b $end
    /// $var integer 16 n0 counter $end
    /// $upscope $end
    /// $upscope $end
    /// $enddefinitions $end
    /// "[..]);
    /// let header = parser.parse_header().unwrap();
    /// let scope = header.find_scope(&["a", "b"]).unwrap();
    /// assert_eq!(scope.identifier, "b");
    /// ```
    pub fn find_scope<S>(&self, path: &[S]) -> Option<&Scope>
    where
        S: std::borrow::Borrow<str>,
    {
        fn find_nested_scope<'a, S>(mut scope: &'a Scope, mut path: &[S]) -> Option<&'a Scope>
        where
            S: std::borrow::Borrow<str>,
        {
            'deeper: while !path.is_empty() {
                for child in &scope.children {
                    match child {
                        ScopeItem::Scope(ref new_scope)
                            if new_scope.identifier == path[0].borrow() =>
                        {
                            scope = new_scope;
                            path = &path[1..];
                            continue 'deeper;
                        }
                        _ => (),
                    }
                }
                return None;
            }
            Some(scope)
        }

        if path.is_empty() {
            return None;
        }

        let scope = self.items.iter().find(|item| match item {
            ScopeItem::Scope(scope) => scope.identifier == path[0].borrow(),
            _ => false,
        });

        if let Some(ScopeItem::Scope(scope)) = scope {
            find_nested_scope(scope, &path[1..])
        } else {
            None
        }
    }

    /// Find the variable object at a specified path.
    ///
    /// ## Example
    ///
    /// ```rust
    /// let mut parser = vcd_ng::Parser::new(&b"
    /// $scope module a $end
    /// $scope module b $end
    /// $var integer 16 n0 counter $end
    /// $upscope $end
    /// $upscope $end
    /// $enddefinitions $end
    /// "[..]);
    /// let header = parser.parse_header().unwrap();
    /// let var = header.find_var(&["a", "b", "counter"]).unwrap();
    /// assert_eq!(var.reference, "counter");
    /// ```
    pub fn find_var<S>(&self, path: &[S]) -> Option<&Var>
    where
        S: std::borrow::Borrow<str>,
    {
        let scope = self.find_scope(&path[..path.len() - 1])?;
        scope.find_var(path[path.len() - 1].borrow())
    }
}
