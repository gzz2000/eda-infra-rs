//! A structural verilog parser written in Rust.
//!
//! # Usage
//! 
//! Just pass a `&str` to [SVerilog::parse_str]. Example:
//! ```
//! use sverilogparse::SVerilog;
//! 
//! let _parsed = SVerilog::parse_str(r#"
//! module simple (a, b);
//! input a;
//! output b;
//! not n1 (.a(a), .out(b));
//! endmodule
//! "#).expect("parse error");
//! ```

use compact_str::CompactString;

/// Packages all content in structural verilog, in an unmodified manner.
#[derive(Debug, Clone)]
pub struct SVerilog {
    /// A vector of module names and parsed module object.
    pub modules: Vec<(CompactString, SVerilogModule)>,
}

mod range;
pub use range::SVerilogRange;

/// A wire/io definition with optional vector width.
#[derive(Debug, Clone)]
pub struct SVerilogWireDef {
    /// Wire name. E.g. `net0`
    pub name: CompactString,
    /// Wire width if it is a vector.
    pub width: Option<SVerilogRange>,
    /// Wire type.
    pub typ: WireDefType,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum WireDefType {
    Input,
    Output,
    InOut,
    Wire
}

/// A parsed structural verilog module.
#[derive(Debug, Clone)]
pub struct SVerilogModule {
    /// Module ports.
    pub ports: Vec<SVerilogPortDef>,
    /// Module I/O and net definitions.
    pub defs: Vec<SVerilogWireDef>,
    /// Assignment operations in the module body.
    pub assigns: Vec<SVerilogAssign>,
    /// Cells in the module body.
    pub cells: Vec<SVerilogCell>,
}

/// A port definition. Can be either a single identifier, or
/// a named port connection like `.gpio({g1, g2, g3})`.
#[derive(Debug, Clone)]
pub enum SVerilogPortDef {
    /// E.g. `gpio`.
    Basic(CompactString),
    /// E.g. `.gpio({g1, g2, g3})`.
    Conn(CompactString, Wirexpr)
}

/// Basic component of a wire expression, which can be
/// a wire reference, reference to a single wire bit,
/// slice of a wire vector, or a constant literal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WirexprBasic {
    /// E.g. `somepin`.
    Full(CompactString),
    /// E.g. `somepin[1]`.
    SingleBit(CompactString, isize),
    /// E.g. `somepin[0:7]`.
    Slice(CompactString, SVerilogRange),
    /// E.g. `4'b01xz`.
    /// The pairs are (size, value, is\_xz).
    Literal(usize, u128, u128),
}

/// A wire expression containing either a basic component or a
/// concatenation of multiple basic components.
#[derive(Debug, Clone)]
pub enum Wirexpr {
    /// A single basic component.
    Basic(WirexprBasic),
    /// Multiple basic components enclosed in curly braces.
    /// E.g. `{somepin, 1'b0, otherpin[0:7]}`.
    Concat(Vec<WirexprBasic>),
}

/// An assign operation.
#[derive(Debug, Clone)]
pub struct SVerilogAssign {
    /// Left-hand side expr.
    pub lhs: Wirexpr,
    /// Right-hand side expr.
    pub rhs: Wirexpr,
}

/// A parsed cell instantiation in structural verilog.
#[derive(Debug, Clone)]
pub struct SVerilogCell {
    /// The name of macro. E.g. `NAND`.
    pub macro_name: CompactString,
    /// The name of cell. E.g. `nand01`.
    pub cell_name: CompactString,
    /// contains tuples of (macro_pin_name, wire_name).
    pub ioports: Vec<(CompactString, Wirexpr)>,
}

mod sverilognom;

impl SVerilog {
    /// Parses a string of structural verilog code, and returns a [Result], indicating successful parse result or an error string.
    #[inline]
    pub fn parse_str(s: &str) -> Result<SVerilog, String> {
        Ok(sverilognom::parse_sverilog(s.as_bytes())?)
    }
    
    /// Parses a u8 slice of structural verilog code, and returns a [Result], indicating successful parse result or an error string.
    #[inline]
    pub fn parse_u8slice(s: &[u8]) -> Result<SVerilog, String> {
        Ok(sverilognom::parse_sverilog(s)?)
    }
    
    /// Parses a structural verilog code at the specific path, and returns a [Result], indicating successful parse result or an error string.
    #[inline]
    pub fn parse_file(path: impl AsRef<std::path::Path>) -> Result<SVerilog, String> {
        let s = match std::fs::read(&path) {
            Ok(s) => s,
            Err(e) => return Err(format!("{}", e))
        };
        SVerilog::parse_u8slice(&s)
    }
}

mod fmt;
