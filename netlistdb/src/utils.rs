//! private utilities.

use super::*;
use sverilogparse::*;
use std::collections::HashSet;
use either::Either;

/// Find the top module.
/// Currently, if the module is not explicitly specified, we
/// guess it by scanning for unreferenced ones.
#[must_use]
#[inline]
pub(crate) fn find_top_module<'i>(
    modules: &'i HashMap<CompactString, (SVerilogModule, ModuleMap)>,
    top: Option<&'_ str>
) -> Option<(&'i CompactString, &'i SVerilogModule, &'i ModuleMap)> {
    if modules.len() == 0 {
        clilog::error!(NL_SV_PARSE, "Empty verilog netlist.");
        return None;
    }
    if let Some(top) = top {
        if let Some((k, (v1, v2))) = modules.get_key_value(top) {
            Some((k, v1, v2))
        }
        else {
            clilog::error!(
                NL_SV_TOPMODULE_NF,
                "Top module {} not found in the verilog code", top);
            return None;
        }
    }
    else if modules.len() != 1 {
        let mut referenced = HashSet::<&CompactString>::new();
        for (_, (m, _)) in modules {
            for cell in &m.cells {
                if modules.contains_key(&cell.macro_name) {
                    referenced.insert(&cell.macro_name);
                }
            }
        }
        let unrefs: Vec<_> = modules.iter()
            .filter(|(s, _)| !referenced.contains(s)).collect();
        if unrefs.len() == 1 {
            let (s, (m, mm)) = unrefs[0];
            clilog::info!(
                NL_SV_GUESSTOP,
                "The top module is guessed to be {}.", s);
            Some((s, m, mm))
        }
        else if unrefs.len() == 0 {
            clilog::error!(
                NL_SV_CANTGUESSTOP,
                "There are cyclic references in netlist, cannot guess top module.");
            return None;
        }
        else {
            clilog::error!(
                NL_SV_CANTGUESSTOP,
                "There are {} potential top modules: {:?}. Please explicitly specify one of them as the top module.",
                unrefs.len(), unrefs.iter().map(|(s, _)| s).collect::<Vec<_>>());
            return None;
        }
    }
    else {
        let (s, (m, mm)) = modules.iter().next().unwrap();
        Some((s, m, mm))
    }
}

/// Enumerate an optional range object.
pub fn enum_in_width(
    w: Option<SVerilogRange>
) -> impl Iterator<Item = Option<isize>> {
    match w {
        None => Either::Left(Some(None).into_iter()),
        Some(r) => Either::Right(r.map(|c| Some(c)))
    }
}

/// Useful preprocessed map for each SVerilog module.
#[readonly::make]
pub struct ModuleMap {
    /// For each vector def, we store its range here.
    ///
    /// Scalar defs do not present in this map. This can be used
    /// to check whether a def is a scalar or a vector.
    pub def_widths: HashMap<CompactString, SVerilogRange>,
    /// For each def, we store its type (input/output/wire/...) here.
    pub def_types: HashMap<CompactString, WireDefType>,
    /// For each port, we store its range (if any) here.
    ///
    /// This is tricky for named port connections, as we have to
    /// determine whether it is a vector or a scalar based on the
    /// context.
    pub port_widths: HashMap<CompactString, SVerilogRange>,
}

/// An enum representing a bit of a Wirexpr.
///
/// This is expected to be created from [`ModuleMap::eval_expr_iter`].
pub enum ExprBit<'i> {
    Const(u8 /* 0, 1, x, z => 0, 1, 2, 3 */),
    Var(&'i CompactString, Option<isize>)
}

/// Evaluate the length of an expr, based on the preprocessed widths.
///
/// This is intended to be fast, as no need to enumerate the slice indices.
fn eval_expr_len(
    def_widths: &HashMap<CompactString, SVerilogRange>,
    expr: &Wirexpr
) -> usize {
    #[inline]
    fn len_basic(
        def_widths: &HashMap<CompactString, SVerilogRange>,
        exprbasic: &WirexprBasic
    ) -> usize {
        use WirexprBasic::*;
        match exprbasic {
            Full(s) => {
                match def_widths.get(s.as_str()) {
                    Some(range) => range.len(),
                    None => 1
                }
            },
            SingleBit(_, _) => 1,
            Slice(_, range) => range.len(),
            Literal(size, _, _) => *size
        }
    }
    use Wirexpr::*;
    match expr {
        Basic(basic) => len_basic(def_widths, basic),
        Concat(v) => v.iter().map(|b| len_basic(def_widths, b)).sum()
    }
}

impl ModuleMap {
    pub fn from(m: &SVerilogModule) -> ModuleMap {
        // compute def widths:
        // just create a map of all defs that have nontrivial widths
        let def_widths: HashMap<CompactString, SVerilogRange> = m.defs.iter()
            .filter_map(|SVerilogWireDef{name, width, ..}| {
                let w = width.as_ref()?.clone();
                Some((name.clone(), w))
            })
            .collect();

        // compute def types:
        // the tricky thing is that an input/output port may later
        // be defined as wire again (which is then ignored).
        let mut def_types = HashMap::with_capacity(m.defs.len());
        for SVerilogWireDef{name, typ, ..} in &m.defs {
            use WireDefType::*;
            match def_types.get_mut(name) {
                Some(v) => {
                    match (&v, typ) {
                        (Wire, Input | Output | InOut) => { *v = *typ; }
                        (_, Wire) => {}
                        _ => { assert_eq!(v, typ, "conflicting def"); }
                    }
                }
                None => { def_types.insert(name.clone(), *typ); }
            }
        }

        // compute port widths by examining ports.
        // 1. normal (Basic) ports have their vecs inherited.
        // 2. named port connections (Conn) are further evaluated.
        let port_widths = m.ports.iter().filter_map(|def| {
            use SVerilogPortDef::*;
            match def {
                Basic(name) => match def_widths.get(name.as_str()) {
                    Some(w) => Some((name.clone(), *w)),
                    None => None
                },
                Conn(name, expr) => {
                    // the evaluation of named port connections
                    // is tricky -- we have to determine whether
                    // it is a scalar or a vector.
                    //
                    // Here, we mimic the behavior of CadXX InnoXX.
                    let width = eval_expr_len(&def_widths, expr);

                    // 1. if width \> 1, it is certainly a vector.
                    //    the vector is always indexed from 0.
                    //    note that it is reversed, [len - 1, 0] is right.
                    if width > 1 {
                        return Some((name.clone(),
                                     SVerilogRange(width as isize - 1, 0)))
                    }

                    // 2. if width == 1, then it is a 1-bit vector
                    //    iff it contains a vector. otherwise it is
                    //    a scalar. it is not related to whether
                    //    curly braces are used.
                    let expr_basic_has_vector = |eb: &WirexprBasic| -> bool {
                        use WirexprBasic::*;
                        match eb {
                            Full(name) => def_widths.contains_key(name),
                            SingleBit(_, _) => false,
                            Slice(_, _) => true,
                            Literal(_, _, _) => false
                        }
                    };
                    use Wirexpr::*;
                    let has_vector = match expr {
                        Basic(eb) => expr_basic_has_vector(eb),
                        Concat(v) => v.iter().any(expr_basic_has_vector)
                    };
                    match has_vector {
                        true => Some((name.clone(), SVerilogRange(0, 0))),
                        false => None
                    }
                }
            }
        }).collect();
        
        ModuleMap { def_widths, def_types, port_widths }
    }

    pub fn eval_expr<'a>(
        &'a self, expr: &'a Wirexpr
    ) -> impl Iterator<Item = ExprBit<'a>> + 'a {
        use Either::*;
        #[inline]
        fn eval_basic<'a>(
            mm: &'a ModuleMap, exprbasic: &'a WirexprBasic,
        ) -> impl Iterator<Item = ExprBit<'a>> + 'a {
            use WirexprBasic::*;
            use ExprBit::*;
            let index_map = |s: &'a CompactString| move |i| Var(s, Some(i));
            match exprbasic {
                Full(s) => match mm.def_widths.get(s.as_str()) {
                    Some(range) => Right(Left(range.map(index_map(s)))),
                    None => Left(Some(Var(s, None)).into_iter())
                },
                SingleBit(s, i) => Left(Some(Var(s, Some(*i))).into_iter()),
                Slice(s, range) => Right(Left(range.map(index_map(s)))),
                Literal(size, value, is_xz) => Right(Right({
                    let (value, is_xz) = (*value, *is_xz);
                    (0..*size).rev()
                        .map(move |i| Const((((is_xz >> i & 1) << 1) |
                                             ((value >> i & 1))) as u8))
                }))
            }
        }
        use Wirexpr::*;
        match expr {
            Basic(basic) => Left(eval_basic(self, basic)),
            Concat(v) => Right(v.iter().map(|b| eval_basic(self, b)).flatten())
        }
    }
    
    /// Evaluate the length of an expr, based on the preprocessed widths.
    ///
    /// This is intended to be fast, as no need to enumerate the
    /// slice indices.
    pub fn eval_expr_len(&self, expr: &Wirexpr) -> usize {
        eval_expr_len(&self.def_widths, expr)
    }
}

/// Estimate num_cells (leaf only) and num_logic_pins,
/// and check that no recursion occurs in the hierarchy.
/// 
/// The result estimation contains all ports of the
/// entry module, all pins of leaf cells, and the recursive
/// calculation of non-leaf submodules.
///
/// To call this function, one need to provide a [HashSet]
/// and a [HashMap] mutable reference.
#[must_use]
pub(crate) fn estimate_size<'i>(
    modules: &'i HashMap<CompactString, (SVerilogModule, ModuleMap)>,
    parent_modules: &mut HashSet<&'i CompactString>,
    (cur_name, cur_m, cur_mm): (&'i CompactString, &'i SVerilogModule, &'i ModuleMap),
    cache: &mut HashMap<&'i CompactString, (usize, usize)>
) -> Option<(usize, usize)> {
    // check if the result is already cached.
    if let Some((x, y)) = cache.get(cur_name) {
        return Some((*x, *y));
    }

    // check for infinite recursion.
    // this is the only way this function could fail.
    // without this check, the program would stuck on
    // bad user input.
    if !parent_modules.insert(cur_name) {
        clilog::error!(
            NL_SV_RECUR, "module {} has recursion which is NOT allowed",
            cur_name);
        return None;
    }
    let mut parent_modules = scopeguard::guard(parent_modules, |parent_modules| {
        parent_modules.remove(cur_name);
    });

    // init counter.
    // num_logic_pins is initialized with the entry ports.
    let mut num_cells = 0;
    let mut num_logic_pins = cur_m.defs.iter()
        .map(|def| def.width.map(|r| r.len()).unwrap_or(1))
        .sum::<usize>();

    // then, named port connections
    num_logic_pins += cur_m.ports.iter()
        .map(|port| match port {
            SVerilogPortDef::Basic(_) => 0,
            SVerilogPortDef::Conn(_, e) => cur_mm.eval_expr_len(e)
        })
        .sum::<usize>();

    // dive into cells.
    // 1. for submodules, recurse and collect result.
    // 2. for leaf cells, evaluate the pin width.
    for cell in &cur_m.cells {
        if let Some((m, mm)) = modules.get(&cell.macro_name) {
            let (c, lp) = estimate_size(
                modules, &mut parent_modules,
                (&cell.macro_name, m, mm), cache)?;
            num_cells += c;
            num_logic_pins += lp;
        }
        else {
            num_cells += 1;
            num_logic_pins += cell.ioports.iter()
                .map(|(_, expr)| cur_mm.eval_expr_len(expr))
                .sum::<usize>();
        }
    }

    // store cache and return result.
    cache.insert(cur_name, (num_cells, num_logic_pins));
    Some((num_cells, num_logic_pins))
}
