use super::*;
use std::{num::NonZeroUsize, str::FromStr};
use std::fmt;
use nom::{
    IResult,
    combinator::{ value, map, recognize, opt, cut },
    branch::alt,
    multi::{ fold_many0, many0, many0_count, separated_list0 },
    sequence::{ delimited, pair, preceded, terminated, tuple },
    character::{ is_alphanumeric, is_hex_digit },
    // we do not plan to stream verilog. so we
    // only include completes.
    character::complete::{ one_of, char, satisfy, digit1, multispace0 },
    bytes::complete::{ tag, take_until, take_till1, take_till, is_not },
};

fn u82str_unsafe(i: &[u8]) -> &str {
    unsafe { std::str::from_utf8_unchecked(i) }
}

/// comment: starts with // and ends with a line.
/// do not use streaming operators here.
/// performance critical.
pub fn skip_whitespace_and_comment(mut i: &[u8]) -> IResult<&[u8], ()> {
    (i, _) = multispace0(i)?;
    while i.len() > 0 {
        if i[0] == b'/' {
            i = match value((), tuple((char('/'), alt((
                value((), pair(char('/'), is_not("\r\n"))),
                value((), tuple((char('*'), take_until("*/"), tag("*/"))))
            )))))(i) as IResult<&[u8], ()> {
                Ok((i, ())) => i,
                Err(_) => break
            };
            (i, _) = multispace0(i)?;
        }
        else if i[0] == b'(' {
            // currently, we regard attributes as comments.
            i = match value((), tuple((
                tag("(*"), take_until("*)"), tag("*)")
            )))(i) as IResult<&[u8], ()> {
                Ok((i, ())) => i,
                Err(_) => break
            };
            (i, _) = multispace0(i)?;
        }
        else {
            break
        }
    }
    Ok((i, ()))
}

/// a higher-order parser transforming a parser to one that
/// eats up all spaces.
pub fn ws<'a, F, O>(inner: F) ->
impl FnMut(&'a [u8]) -> IResult<&'a [u8], O>
where F: FnMut(&'a [u8]) -> IResult<&'a [u8], O> {
    delimited(skip_whitespace_and_comment,
              inner,
              skip_whitespace_and_comment)
}

/// Parse an identifier.
fn ident(i: &[u8]) -> IResult<&[u8], CompactString> {
    map(alt((
        preceded(char('\\'), cut(take_till1(|c| {
            c == b'\n' || c == b'\r' || c == b' ' || c == b'\t'
        }))),
        recognize(pair(
            satisfy(|c| c.is_alphabetic() || c == '_'),
            take_till(|c| {
                !is_alphanumeric(c) && c != b'_' && c != b'$'
            }),
        )),
    )), |s| CompactString::from(u82str_unsafe(s)))(i)
}

fn int(input: &[u8]) -> IResult<&[u8], isize> {
    map(recognize(
        preceded(
            opt(char('-')),
            digit1
        )
    ), |i| isize::from_str(u82str_unsafe(i)).unwrap())(input)
}

fn uint(input: &[u8]) -> IResult<&[u8], usize> {
    map(recognize(
        digit1
    ), |i| usize::from_str(u82str_unsafe(i)).unwrap())(input)
}

/// parses a constant literal.
/// if the width of the constant exceeds 128, it will be split to multiple
/// 128-bit entries.
fn literal(i: &[u8]) -> IResult<&[u8], Vec<WirexprBasic>> {
    map(tuple((
        uint,
        char('\''),
        one_of("bBoOdDhH"),
        take_till1(|c| {
            !is_hex_digit(c) && c != b'_' &&
                c != b'x' && c != b'X' && c != b'z' && c != b'Z'
        })
    )), |(width, _, radix, hexint)| {
        use awint::ExtAwi;
        let width_nonzero = NonZeroUsize::new(width).unwrap();
        let radix = match radix {
            'b' | 'B' => 2,
            'o' | 'O' => 8,
            'd' | 'D' => 10,
            'h' | 'H' => 16,
            _ => unreachable!()
        };

        let has_xz = hexint.iter().any(|c| matches!(*c, b'x' | b'z'));
        let (mut value, mut is_xz) = if !has_xz {
            (ExtAwi::from_bytes_radix(None, hexint, radix, width_nonzero).unwrap(),
             ExtAwi::zero(width_nonzero))
        } else {
            // we need to decode the x/z into is_xz bits.
            // also we need to expand the highest-digit if it is x or z.
            let (radix_full_char, radix_bits) = match radix {
                10 => panic!("base 10 literals cannot have x/z."),
                2 => (b'1', 1), 8 => (b'7', 3), 16 => (b'f', 4),
                _ => unreachable!()
            };
            let full_ndigits = (width + radix_bits - 1) / radix_bits;
            let mut tmp_value = vec![b'0'; full_ndigits];
            let mut tmp_is_xz = vec![b'0'; full_ndigits];
            let full_width_nonzero = NonZeroUsize::new(full_ndigits * radix_bits).unwrap();

            let mut pos = full_ndigits;
            for i in (0..hexint.len()).rev() {
                let (d_value, d_is_xz) = match hexint[i] {
                    b'_' => continue,
                    b'x' => (b'0', radix_full_char),
                    b'z' => (radix_full_char, radix_full_char),
                    num @ _ => (num, b'0')
                };
                pos -= 1;
                tmp_value[pos] = d_value;
                tmp_is_xz[pos] = d_is_xz;
            }
            if pos != 0 && tmp_is_xz[pos] == radix_full_char {
                for i in 0..pos {
                    tmp_value[i] = tmp_value[pos];
                    tmp_is_xz[i] = tmp_is_xz[pos];
                }
            }
            let value = ExtAwi::from_bytes_radix(None, &tmp_value, radix, full_width_nonzero).unwrap();
            let is_xz = ExtAwi::from_bytes_radix(None, &tmp_is_xz, radix, full_width_nonzero).unwrap();
            (value, is_xz)
        };

        let n_u128s = (width + 127) / 128;
        let mut ret = Vec::with_capacity(n_u128s);
        for i in 0..n_u128s {
            use std::num::Wrapping;
            let w = (width - i * 128).min(128);
            let mask = ((Wrapping(!0u128) << (128 - w)) >> (128 - w)).0;
            ret.push(WirexprBasic::Literal(
                w, value.to_u128() & mask, is_xz.to_u128() & mask
            ));
            if i + 1 != n_u128s {
                value.lshr_(128).unwrap();
                is_xz.lshr_(128).unwrap();
            }
        }
        ret.reverse();
        ret
    })(i)
}

fn wirexpr(i: &[u8]) -> IResult<&[u8], Wirexpr> {
    use Wirexpr::*;
    use WirexprBasic::*;
    alt((
        // ident[int:int]
        map(pair(ws(ident), cut(opt(
            delimited(char('['),
                      pair(ws(int), opt(preceded(char(':'), ws(int)))),
                      char(']'))
        ))), |(name, o)| Basic(match o {
            None => Full(name),
            Some((l, None)) => SingleBit(name, l),
            Some((l, Some(r))) => Slice(name, SVerilogRange(l, r))
        })),
        // literal
        map(literal, |mut l| {
            if l.len() == 1 {
                Basic(l.swap_remove(0))
            } else {
                Concat(l)
            }
        }),
        // concat
        map(delimited(
            ws(char('{')),
            separated_list0(char(','), ws(cut(wirexpr))),
            ws(char('}'))
        ), |ws| {
            let mut v = Vec::new();
            for mut w in ws {
                match w {
                    Basic(basic) => v.push(basic),
                    Concat(ref mut v2) => v.append(v2)
                }
            }
            Concat(v)
        })
    ))(i)
}

fn portdef(i: &[u8]) -> IResult<&[u8], SVerilogPortDef> {
    use SVerilogPortDef::*;
    alt((
        map(ident, |name| Basic(name)),
        map(pair(preceded(ws(char('.')), cut(ident)),
                 delimited(ws(char('(')), cut(wirexpr), ws(char(')')))),
            |(name, expr)| Conn(name, expr))
    ))(i)
}

fn wiredef_push_<'i>(i: &'i [u8], defs: &mut Vec<SVerilogWireDef>) -> IResult<&'i [u8], ()> {
    use WireDefType::*;
    let (i, typ) = ws(alt((
        value(Input, tag("input")),
        value(Output, tag("output")),
        value(InOut, tag("inout")),
        value(Wire, tag("wire")),
    )))(i)?;
    let (i, width) = opt(map(tuple((
        char('['), ws(int), char(':'), ws(int), char(']')
    )), |(_, l, _, r, _)| SVerilogRange(l, r)))(i)?;
    // println!("after wiredef parsing typ {:?}, rng {:?}, remain: {:?}",
    //          typ, width, u82str_unsafe(i));
    let build_def = |name| SVerilogWireDef { name, width, typ };
    let (i, ()) = cut(
        map(ws(ident), |name| defs.push(build_def(name)))
    )(i)?;
    let (i, ()) = fold_many0(
        preceded(char(','), ws(cut(ident))), || (),
        |_, name| defs.push(build_def(name))
    )(i)?;
    let (i, _) = ws(cut(char(';')))(i)?;
    Ok((i, ()))
}

fn assign(i: &[u8]) -> IResult<&[u8], SVerilogAssign> {
    map(tuple((
        ws(tag("assign")),
        cut(wirexpr),
        ws(cut(char('='))),
        cut(wirexpr),
        ws(cut(char(';')))
    )), |(_, lhs, _, rhs, _)| SVerilogAssign { lhs, rhs })(i)
}

fn cell(i: &[u8]) -> IResult<&[u8], SVerilogCell> {
    map(tuple((
        ws(ident), ident,
        delimited(ws(char('(')), cut(separated_list0(
            char(','), map(tuple((
                ws(char('.')), ident,
                ws(char('(')), opt(wirexpr), ws(char(')'))
            )), |(_, name, _, expr, _)| {
                expr.map(|e| (name, e)) 
            })
        )), pair(ws(char(')')), ws(char(';'))))
    )), |(macro_name, cell_name, ioports)| SVerilogCell {
        macro_name, cell_name, ioports: ioports.into_iter().filter_map(|port| port).collect(),
    })(i)
}

fn module(i: &[u8]) -> IResult<&[u8], (CompactString, SVerilogModule)> {
    let (i, (name, ports)) = pair(
        preceded(ws(tag("module")), cut(ident)),
        cut(delimited(ws(char('(')),
                      separated_list0(char(','), ws(portdef)),
                      pair(ws(char(')')), ws(char(';')))))
    )(i)?;
    let mut defs = Vec::new();
    let mut assigns = Vec::new();
    let mut cells = Vec::new();
    // println!("after header parsing.. at: {:?}", u82str_unsafe(i));
    let (i, _) = many0_count(alt((
        |i| wiredef_push_(i, &mut defs),
        map(assign, |a| assigns.push(a)),
        map(cell, |c| cells.push(c))
    )))(i)?;
    let (i, _) = cut(ws(tag("endmodule")))(i)?;
    Ok((i, (name, SVerilogModule {
        ports, defs, assigns, cells
    })))
}

fn sverilog(i: &[u8]) -> IResult<&[u8], SVerilog> {
    map(preceded(
        many0_count(ws(char(';'))),
        ws(many0(terminated(
            ws(module),
            many0_count(ws(char(';'))))))),
        |modules| SVerilog { modules })(i)
}

/// a `Display`able parsing error type, which prints at most
/// 50 characters after the error position.
pub(crate) struct ParseError {
    code: nom::error::ErrorKind,
    partial_input: String
}

impl From<nom::Err<nom::error::Error<&[u8]>>> for ParseError {
    fn from(e: nom::Err<nom::error::Error<&[u8]>>) -> Self {
        match e {
            nom::Err::Incomplete(_) => ParseError {
                code: nom::error::ErrorKind::Fail,
                partial_input: "<incomplete>".into()
            },
            nom::Err::Error(e) | nom::Err::Failure(e) => ParseError {
                code: e.code,
                partial_input: std::str::from_utf8(
                    &e.input[..e.input.len().min(50)]).unwrap()
                    .to_string()
            }
        }
    }
}

impl From<std::io::Error> for ParseError {
    fn from(e: std::io::Error) -> Self {
        ParseError {
            code: nom::error::ErrorKind::Fail,
            partial_input: format!("<io error: {}>", e)
        }
    }
}

impl From<ParseError> for String {
    fn from(e: ParseError) -> String {
        format!("{}", e)
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error {:?} at: {}",
               self.code,
               self.partial_input)
    }
}

pub(crate) fn parse_sverilog(i: &[u8]) -> Result<SVerilog, ParseError> {
    let (rem, sv) = sverilog(i)?;
    if rem.len() > 0 {
        return Err(nom::Err::Error(nom::error::Error {
            input: rem,
            code: nom::error::ErrorKind::Complete
        }).into())
    }
    Ok(sv)
}
