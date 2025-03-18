use std::fmt;
use itertools::Itertools;
use std::fmt::Write;
use lazy_static::lazy_static;
use regex::Regex;

use super::*;

lazy_static! {
    static ref RE_SAFE_IDENT: Regex = Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_\$]*$").unwrap();
}

pub struct SVIdentFmt<'i>(&'i str);

impl fmt::Display for SVIdentFmt<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if RE_SAFE_IDENT.is_match(self.0) {
            write!(f, "{}", self.0)
        }
        else {
            write!(f, "\\{} ", self.0)
        }
    }
}

impl fmt::Display for SVerilog {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (m_name, m) in &self.modules {
            writeln!(f, "module {}({});",
                     SVIdentFmt(&m_name), m.ports.iter().format(", "))?;
            let mut ind = indenter::indented(f)
                .with_format(indenter::Format::Uniform{ indentation: "  " });
            for def in &m.defs {
                writeln!(ind, "{}", def)?;
            }
            writeln!(ind)?;
            for assign in &m.assigns {
                writeln!(ind, "{}", assign)?;
            }
            for cell in &m.cells {
                writeln!(ind, "{}", cell)?;
            }
            writeln!(f, "endmodule")?
        }
        Ok(())
    }
}

impl fmt::Display for SVerilogPortDef {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use SVerilogPortDef::*;
        match self {
            Basic(s) => write!(f, "{}", SVIdentFmt(&s)),
            Conn(s, e) => write!(f, ".{}({})", SVIdentFmt(&s), e)
        }
    }
}

impl fmt::Display for WireDefType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use WireDefType::*;
        write!(f, "{}", match self {
            Input => "input",
            Output => "output",
            InOut => "inout",
            Wire => "wire",
        })
    }
}

impl fmt::Display for SVerilogWireDef {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.width {
            None => write!(f, "{} {};", self.typ, SVIdentFmt(&self.name)),
            Some(SVerilogRange(l, r)) => write!(
                f, "{} [{}:{}] {};", self.typ, l, r,
                SVIdentFmt(&self.name)),
        }
    }
}

impl fmt::Display for WirexprBasic {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use WirexprBasic::*;
        match self {
            Full(s) => write!(f, "{}", SVIdentFmt(s)),
            SingleBit(s, i) => write!(f, "{}[{}]", SVIdentFmt(s), i),
            Slice(s, SVerilogRange(i, j)) => write!(f, "{}[{}:{}]", SVIdentFmt(s), i, j),
            Literal(w, v, is_xz) => {
                write!(f, "{}'b{}", w, (0..*w).rev().map(|i| {
                    match ((v >> i & 1), (is_xz >> i & 1)) {
                        (0, 0) => '0',
                        (1, 0) => '1',
                        (0, 1) => 'x',
                        (1, 1) => 'z',
                        _ => panic!()
                    }
                }).collect::<String>())
            }
        }
    }
}

impl fmt::Display for Wirexpr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Wirexpr::*;
        match self {
            Basic(b) => write!(f, "{}", b),
            Concat(v) => write!(f, "{{{}}}", v.iter().format(", "))
        }
    }
}

impl fmt::Display for SVerilogAssign {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "assign {} = {};", self.lhs, self.rhs)
    }
}

impl fmt::Display for SVerilogCell {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}({});",
               SVIdentFmt(&self.macro_name), SVIdentFmt(&self.cell_name),
               self.ioports.iter().map(
                   |(n, e)| format!(".{}({})", SVIdentFmt(&n), e)
               ).format(", "))
    }
}
