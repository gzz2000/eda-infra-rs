use super::*;
use sverilogparse::*;
use either::Either;
use std::borrow::Cow;
use rayon::prelude::*;

/// Leaf pin direction and width provider trait.
///
/// Downstream databases (e.g., Liberty library or LEF/DEF library)
/// should implement this to provide pin direction information
/// used in constructing NetlistDB.
///
/// This was called DirectionProvider before. That name can still
/// be used as an alias, but deprecated.
pub trait LeafPinProvider {
    /// This function is called from NetlistDB constructor to
    /// query the direction of library cell pins.
    fn direction_of(
        &self,
        macro_name: &CompactString,
        pin_name: &CompactString, pin_idx: Option<isize>
    ) -> Direction;

    // This function is called to query the range of the bus width
    fn width_of(
        &self,
        macro_name: &CompactString,
        pin_name: &CompactString
    ) -> Option<SVerilogRange>;

    /// This function allows downstream databases to specify
    /// whether there should be a warning on unspecified
    /// directions when building netlist.
    #[inline]
    fn should_warn_missing_directions(&self) -> bool {
        true
    }
}

#[doc(hidden)]
pub use LeafPinProvider as DirectionProvider;

impl DirectionProvider for HashMap<(CompactString, CompactString, Option<isize>), Direction> {
    #[inline]
    fn direction_of(
        &self,
        macro_name: &CompactString,
        pin_name: &CompactString, pin_idx: Option<isize>
    ) -> Direction {
        let k = (macro_name.clone(), pin_name.clone(), pin_idx);
        self.get(&k).copied().unwrap_or(Direction::Unknown)
    }

    #[inline]
    fn width_of(
        &self,
        _: &CompactString,
        _: &CompactString
    ) -> Option<SVerilogRange>{
        return None;
    }
}

impl<T> DirectionProvider for T
where T: Fn(&CompactString, &CompactString, Option<isize>) -> Direction {
    #[inline]
    fn direction_of(
        &self,
        macro_name: &CompactString,
        pin_name: &CompactString, pin_idx: Option<isize>
    ) -> Direction {
        self(macro_name, pin_name, pin_idx)
    }

    #[inline]
    fn width_of(
        &self,
        _: &CompactString,
        _: &CompactString
    ) -> Option<SVerilogRange>{
        return None;
    }
}

/// A special direction hint that gives no answer to every pin.
/// This is useful if you do not care about the pin direction
/// (e.g. if you are outputting the benchmark statistics only).
pub struct NoDirection;

impl DirectionProvider for NoDirection {
    #[inline]
    fn direction_of(&self,
                    _: &CompactString, _: &CompactString, _: Option<isize>
    ) -> Direction {
        Direction::Unknown
    }

    #[inline]
    fn width_of(
        &self,
        _: &CompactString,
        _: &CompactString
    ) -> Option<SVerilogRange>{
        return None;
    }

    /// Directions are explicitly ignored, so disable all
    /// such warnings.
    #[inline]
    fn should_warn_missing_directions(&self) -> bool {
        false
    }
}

impl NetlistDB {
    /// Get or insert a logic pin with hier, name and index.
    /// If exists, return index. Otherwise, add it and return it.
    #[inline]
    fn get_or_insert_logic_pin(
        &mut self, hier: &HierName, name: &CompactString, idx: Option<isize>
    ) -> usize {
        let k = (hier.clone(), name.clone(), idx);
        if let Some(i) = self.logicpinname2id.get(&k) {
            return *i
        }
        let id = self.num_logic_pins;
        self.num_logic_pins += 1;
        self.logicpinname2id.insert(k.clone(), id);
        self.logicpintypes.push(LogicPinType::Others);
        self.logicpinnames.push(k);
        id
    }

    /// Find the logic pin with a specific name.
    /// If not found, *prints an error message* and returns None.
    /// This is useful in netlist construction.
    #[inline]
    #[must_use]
    fn try_find_logic_pin(
        &self, hier: &HierName, name: &CompactString, idx: Option<isize>
    ) -> Option<usize> {
        let k = (hier.clone(), name.clone(), idx);
        let r = self.logicpinname2id.get(&k);
        if r.is_none() {
            clilog::error!(
                NL_SV_REF, "pin/net reference {}/{} [{:?}] not found",
                hier, name, idx);
        }
        r.copied()
    }

    /// Insert a cell with hier name and macro name.
    /// Returns the new cell id.
    #[inline]
    fn insert_cell(
        &mut self, hier: HierName, macro_name: CompactString
    ) -> usize {
        let id = self.num_cells;
        self.num_cells += 1;
        self.cellname2id.insert(hier.clone(), id);
        self.celltypes.push(macro_name);
        self.cellnames.push(hier);
        id
    }

    /// Recursively build (and flatten) the hierarchical modules.
    /// `net_sets` is the disjoint set of logic pins into nets.
    /// `hier` is the current hier name.
    /// This call will build all port pins inside it. So no need to
    /// build them outside.
    #[must_use]
    fn build_modules(
        &mut self,
        modules: &HashMap<CompactString, (SVerilogModule, ModuleMap)>,
        (_m_name, m, mm): (&CompactString, &SVerilogModule, &ModuleMap),
        net_sets: &mut DisjointSet,
        hier: HierName,
        lib: &impl LeafPinProvider
    ) -> Option<()> {
        // create nets/IO logic pins
        for def in &m.defs {
            for w in enum_in_width(def.width) {
                let id = self.get_or_insert_logic_pin(&hier, &def.name, w);
                self.logicpintypes[id] = LogicPinType::Net;
                // for the top module, the ports are tagged as
                // LogicPinType::TopPort outside the first invocation.
                // (in [NetlistDB::from_sverilog]).
            }
        }

        #[must_use]
        fn pin_assign_literal(
            net_sets: &mut DisjointSet, l: usize, c: u8
        ) -> Option<()> {
            match c {
                0 => net_sets.set_value(l, false),
                1 => net_sets.set_value(l, true),
                2 => {
                    clilog::warn!(NL_SV_LIT, "X literal unsupported, treating as 0");
                    // return None
                    net_sets.set_value(l, false);
                },
                3 => {},
                _ => unreachable!()
            }
            Some(())
        }

        // create named ports
        for (name, expr) in m.ports.iter().filter_map(|p| match p {
            SVerilogPortDef::Basic(_) => None,
            SVerilogPortDef::Conn(name, expr) => Some((name, expr))
        }) {
            let width = mm.port_widths.get(name).copied();
            for (id, eb) in enum_in_width(width).zip(mm.eval_expr(expr)) {
                let port_id = self.get_or_insert_logic_pin(&hier, name, id);
                use ExprBit::*;
                match eb {
                    Const(c) => {
                        pin_assign_literal(net_sets, port_id, c)?;
                    },
                    Var(pname, pidx) => {
                        let pin_id = self.try_find_logic_pin(
                            &hier, &pname, pidx)?;
                        net_sets.merge(port_id, pin_id);
                    }
                }
            }
        }

        let hier_prev = match hier.is_empty() {
            true => None,
            false => Some(Arc::new(hier.clone()))
        };

        // recurse into submodules and cells
        for cell in &m.cells {
            let new_hier = HierName {
                prev: hier_prev.clone(),
                cur: cell.cell_name.clone()
            };

            // build submodule/cell and get ranges of ports
            let (is_leaf, ioport_ranges) = match modules.get(&cell.macro_name) {
                Some((m, mm)) => {
                    // non-leaf.
                    self.build_modules(
                        modules,
                        (&cell.macro_name, m, mm),
                        net_sets,
                        new_hier.clone(),
                        lib
                    )?;
                    (false,
                     Either::Left(cell.ioports.iter().map(|(name, _)| {
                         mm.port_widths.get(name.as_str()).copied()
                     })))
                },
                None => {
                    // leaf cell.
                    self.insert_cell(new_hier.clone(),
                                     cell.macro_name.clone());
                    (true,
                     Either::Right(cell.ioports.iter().map(|(macro_pin_name, expr)| {
                         // mimics CadXX InnoXX:
                         // a leaf cell port is a scalar iff width == 1.
                         match mm.eval_expr_len(expr) {
                             1 => None,
                             _ => {
                                match lib.width_of(&cell.macro_name, &macro_pin_name) {
                                    Some(SVerilogRange(left, right))  => Some(SVerilogRange(left, right)),
                                    _ => None
                                }
                            }
                         }
                     })))
                }
            };

            // connect edges.
            for (w, (name, expr)) in ioport_ranges.zip(cell.ioports.iter()) {
                for (i, eb) in enum_in_width(w).zip(mm.eval_expr(expr)) {
                    let id = match is_leaf {
                        // if it is a leaf, we insert the pin.
                        true => self.get_or_insert_logic_pin(&new_hier, &name, i),
                        // if it is a submodule, the pin should already be ready,
                        // so we assert its existence.
                        false => self.try_find_logic_pin(&new_hier, &name, i)?
                    };
                    if is_leaf {
                        self.logicpintypes[id] = LogicPinType::LeafCellPin;
                    }
                    match eb {
                        ExprBit::Const(c) => {
                            pin_assign_literal(net_sets, id, c)?;
                        }
                        ExprBit::Var(pname, pidx) => {
                            // a wire might be used but not defined.
                            let eb_id = self.get_or_insert_logic_pin(
                                &hier, &pname, pidx);
                            let typ = &mut self.logicpintypes[eb_id];
                            if *typ == LogicPinType::Others {
                                *typ = LogicPinType::Net;
                            }
                            net_sets.merge(id, eb_id);
                        }
                    }
                }
            }
        }
        
        // connect assignments
        for assign in &m.assigns {
            let len_lhs = mm.eval_expr_len(&assign.lhs);
            let len_rhs = mm.eval_expr_len(&assign.rhs);
            if len_lhs != len_rhs {
                clilog::error!(
                    NL_SV_INCOMP,
                    "incompatible assign width for `{}`: \
                     len(LHS) = {}, len(RHS) = {}",
                    assign, len_lhs, len_rhs);
                return None
            }

            for (lb, rb) in mm.eval_expr(&assign.lhs).zip(
                mm.eval_expr(&assign.rhs)
            ) {
                use ExprBit::*;
                match (lb, rb) {
                    (Var(nl, il), Var(nr, ir)) => {
                        let l = self.get_or_insert_logic_pin(&hier, &nl, il);
                        let r = self.get_or_insert_logic_pin(&hier, &nr, ir);
                        net_sets.merge(l, r);
                    }
                    (Var(nl, il), Const(c)) => {
                        let l = self.get_or_insert_logic_pin(&hier, &nl, il);
                        pin_assign_literal(net_sets, l, c)?;
                    }
                    (Const(c), Var(nr, ir)) => {
                        let r = self.get_or_insert_logic_pin(&hier, &nr, ir);
                        pin_assign_literal(net_sets, r, c)?;
                    }
                    _ => {
                        clilog::error!(NL_SV_LIT, "Bad lit-lit assign.");
                        return None;
                    }
                }
            }
        }

        Some(())
    }

    /// Building a netlist database STEP 1: initialize most of the
    /// graph structure using parsed verilog modules starting from
    /// the top-level module.
    /// 
    /// The only remaining thing to do is to assign pin directions.
    ///
    /// Splitting this out would possibly reduce the code bloat
    /// caused by the polymorphic DirectionProvider.
    /// (This is a premature optimization. Evil.)
    fn init_graph_from_modules(
        modules: &HashMap<CompactString, (SVerilogModule, ModuleMap)>,
        (top_name, top_m, top_mm): (&CompactString, &SVerilogModule, &ModuleMap),
        lib: &impl LeafPinProvider
    ) -> Option<NetlistDB> {
        let (est_num_cells, est_num_logic_pins) = estimate_size(
            modules, &mut HashSet::new(), (top_name, top_m, top_mm),
            &mut HashMap::new()
        )?;
        let est_num_cells = est_num_cells + 1; // top level

        let mut db = NetlistDB {
            name: top_name.clone(),
            num_cells: 1,
            num_pins: 0,
            num_logic_pins: 0,
            num_nets: 0,
            cellname2id: HashMap::with_capacity(est_num_cells), 
            logicpinname2id: HashMap::with_capacity(est_num_logic_pins),
            pinname2id: HashMap::new(),
            netname2id: HashMap::new(),
            portname2pinid: HashMap::new(),
            celltypes: Vec::with_capacity(est_num_cells),
            cellnames: Vec::with_capacity(est_num_cells),
            logicpintypes: Vec::with_capacity(est_num_logic_pins),
            logicpinnames: Vec::with_capacity(est_num_logic_pins),
            pinid2logicpinid: Vec::new(),
            netnames: Vec::new(),
            pinnames: Vec::new(),
            pin2cell: UVec::new(),
            pin2net: UVec::new(),
            cell2pin: Default::default(),
            net2pin: Default::default(),
            pindirect: UVec::new(),
            cell2noutputs: UVec::new(),
            net_zero: None,
            net_one: None,
        };

        db.cellname2id.insert(HierName::empty(), 0);
        db.celltypes.push(top_name.clone());
        db.cellnames.push(HierName::empty());

        let mut net_sets = DisjointSet::with_capacity(est_num_logic_pins);

        let time_build_modules = clilog::stimer!("build_modules");
        db.build_modules(
            modules, (top_name, top_m, top_mm),
            &mut net_sets, HierName::empty(), lib
        )?;
        clilog::finish!(time_build_modules);

        if db.num_logic_pins > est_num_logic_pins {
            clilog::warn!(
                NL_SV_MOREPIN,
                "there turns out to be more \
                 logic pin (net) than expected -- typically because there \
                 are nets undefined but used. It is not suggested to do so \
                 because this will lead to long construction time.");
        }

        // set TopPort property for top module ports.
        for port in &top_m.ports {
            let name = match port {
                SVerilogPortDef::Basic(name) => name,
                SVerilogPortDef::Conn(name, _) => name
            };
            let w = top_mm.port_widths.get(name).copied();
            for w in enum_in_width(w) {
                let id = db.try_find_logic_pin(&HierName::empty(), name, w)?;
                db.logicpintypes[id] = LogicPinType::TopPort;
            }
        }

        // create net maps.
        // first finalize the disjoint set and compute the sizes.
        let (num_nets, logicpin2nets, net_zero, net_one) =
            net_sets.finalize(db.num_logic_pins)?;
        db.num_nets = num_nets;
        db.net_zero = net_zero;
        db.net_one = net_one;

        // finalize pin index and pin-net mapping.
        db.pinid2logicpinid = db.logicpintypes.iter()
            .enumerate()
            .filter_map(|(id, t)| {
                match t.is_pin() {
                    false => None,
                    true => Some(id)
                }
            })
            .collect::<Vec<usize>>();
        db.num_pins = db.pinid2logicpinid.len();

        let time_build_public_maps = clilog::stimer!("build_public_maps");

        let mut ret_pinname2id = None;
        let mut ret_pin2cell = None;
        let mut ret_cell2pin = None;
        let mut ret_pinnames = None;
        let mut ret_pin2net = None;
        let mut ret_net2pin = None;
        let mut ret_netname2id = None;
        let mut ret_netnames = None;
        let mut ret_portname2pinid = None;
        rayon::scope(|s| {
            // pinname2id -> pin2cell -> cell2pin
            s.spawn(|_| {
                let pinname2id = db.pinid2logicpinid.iter()
                    .enumerate()
                    .map(|(id, logic_id)|
                         (db.logicpinnames[*logic_id].clone(), id))
                    .collect::<HashMap<_, _>>();

                // finalize pin to cell map
                let mut pin2cell = UVec::new_filled(
                    usize::MAX, db.num_pins, Device::CPU
                );
                let pinidcell = pinname2id.par_iter()
                    .map(|((hier, _, _), id)| {
                        (*id, *db.cellname2id.get(hier).unwrap())
                    })
                    .collect::<Vec<_>>();
                for (id, cellid) in pinidcell {
                    pin2cell[id] = cellid;
                }
                debug_assert!(!pin2cell.iter().any(|x| *x == usize::MAX));

                // construct cell CSR
                let cell2pin = VecCSR::from(db.num_cells, db.num_pins, &pin2cell);

                ret_pinname2id = Some(pinname2id);
                ret_pin2cell = Some(pin2cell);
                ret_cell2pin = Some(cell2pin);
            });

            // pinnames
            s.spawn(|_| {
                ret_pinnames = Some(db.pinid2logicpinid.iter()
                    .map(|logic_id| db.logicpinnames[*logic_id].clone())
                    .collect::<Vec<_>>());
            });

            // pin2net -> net2pin
            s.spawn(|_| {
                let pin2net = db.pinid2logicpinid.iter()
                    .map(|logic_id| logicpin2nets[*logic_id])
                    .collect::<UVec<_>>();

                // construct net CSR
                let net2pin = VecCSR::from(db.num_nets, db.num_pins, &pin2net);
                ret_pin2net = Some(pin2net);
                ret_net2pin = Some(net2pin);
            });

            // netname2id, netnames
            s.spawn(|_| {
                let netname2id = db.logicpinnames.iter()
                    .enumerate()
                    .filter_map(|(id, name)| match db.logicpintypes[id].is_net() {
                        false => None,
                        true => Some((name.clone(), logicpin2nets[id]))
                    })
                    .collect::<HashMap<_, _>>();
                let mut netnames = vec![(
                    HierName::empty(), CompactString::new_inline(""), None
                ); netname2id.len()];

                // find the best name for each net
                for (netname, id) in &netname2id {
                    let current_name = &mut netnames[*id];
                    let current_hier_depth = current_name.0.iter().count();
                    let netname_hier_depth = netname.0.iter().count();
                    if current_name.1.is_empty()
                        || netname_hier_depth < current_hier_depth
                        || (netname_hier_depth == current_hier_depth
                            && netname.1 < current_name.1)
                    {
                        *current_name = netname.clone();
                    }
                }
                ret_netname2id = Some(netname2id);
                ret_netnames = Some(netnames);
            });

            // portname2pinid
            s.spawn(|_| {
                let logicpinid2pinid = db.pinid2logicpinid.iter()
                    .enumerate().map(|(i, &lpi)| (lpi, i))
                    .collect::<HashMap<_, _>>();
                let mut portname2pinid = HashMap::new();
                for port in &top_m.ports {
                    let (name, expr) = match port {
                        SVerilogPortDef::Basic(name) => (
                            name, Cow::Owned(Wirexpr::Basic(
                                WirexprBasic::Full(name.clone())
                            ))
                        ),
                        SVerilogPortDef::Conn(name, expr) => (
                            name, Cow::Borrowed(expr)
                        )
                    };
                    let width = top_mm.port_widths.get(name).copied();
                    for (w, eb) in enum_in_width(width).zip(
                        top_mm.eval_expr(&expr)
                    ) {
                        let (pname, pidx) = match eb {
                            ExprBit::Const(_) => continue,
                            ExprBit::Var(pname, pidx) => (pname, pidx)
                        };
                        let id = db.try_find_logic_pin(
                            &HierName::empty(), name, w).unwrap();
                        let id = *logicpinid2pinid.get(&id).unwrap();
                        portname2pinid.insert((pname.clone(), pidx), id);
                    }
                }
                ret_portname2pinid = Some(portname2pinid);
            });
        });
        
        db.pinname2id = ret_pinname2id.unwrap();
        db.pin2cell = ret_pin2cell.unwrap();
        db.cell2pin = ret_cell2pin.unwrap();
        db.pinnames = ret_pinnames.unwrap();
        db.pin2net = ret_pin2net.unwrap();
        db.net2pin = ret_net2pin.unwrap();
        db.netname2id = ret_netname2id.unwrap();
        db.netnames = ret_netnames.unwrap();
        db.portname2pinid = ret_portname2pinid.unwrap();

        clilog::finish!(time_build_public_maps);
        
        Some(db)
    }

    /// Build a database from a parsed structural verilog object.
    /// 
    /// The top module to be built from can be optionally specified through
    /// the `top` parameter.
    ///
    /// There should be a way to specify library pin directions --
    /// through a trait called direction provider.
    pub fn from_sverilog(
        sverilog_source: SVerilog,
        top: Option<&str>,
        direction_provider: &impl DirectionProvider
    ) -> Option<NetlistDB> {
        let SVerilog{modules} = sverilog_source;
        
        let modules: HashMap<CompactString, (SVerilogModule, ModuleMap)> =
            modules.into_iter().map(|(k, v)| {
                let mm = ModuleMap::from(&v);
                (k, (v, mm))
            }).collect();
        
        let (top_name, top_m, top_mm) = find_top_module(&modules, top)?;
        
        let mut db = NetlistDB::init_graph_from_modules(
            &modules,
            (top_name, top_m, top_mm),
            direction_provider
        )?;

        db.assign_direction((top_name, top_m, top_mm), direction_provider)?;
        
        Some(db)
    }

    /// Set the first pin of net2pin.item is driver pin
    #[must_use]
    pub fn post_assign_direction(
        &mut self
    ) -> Option<()> {
        let mut num_undriven_nets = 0;
        // todo: parallelizable
        for i in 0..self.num_nets {
            let l = self.net2pin.start[i];
            let r = self.net2pin.start[i + 1];
            let outs = (l..r).zip(self.net2pin.items[l..r].iter())
                .filter_map(
                    |(i, x)| if self.pindirect[*x] == Direction::O { Some(i) } else { None }
                )
                .collect::<Vec<usize>>();
            if outs.len() == 0 {
                // if this net is not intended to be constant,
                // we report the error.
                if Some(i) != self.net_zero &&
                    Some(i) != self.net_one
                {
                    num_undriven_nets += 1;
                }
                continue;
            }
            if outs.len() != 1 {
                clilog::error!(
                    NL_SV_NETIO,
                    "There must be exactly one driver for each net. \
                     The net {} has outputs {:?}",
                    i, outs);
                return None
            }
            let p = outs[0];
            self.net2pin.items.swap(l, p);
        }
        if num_undriven_nets != 0 {
            clilog::warn!(NL_SV_NETIO_UNDRIV,
                          "There are {} nets without driving pins.",
                          num_undriven_nets);
        }
        self.cell2noutputs = (0..self.num_cells)
            .map(|cellid| {
                self.cell2pin.iter_set(cellid)
                    .filter(|&pinid| matches!(self.pindirect[pinid], Direction::O))
                    .count()
            })
            .collect();
        Some(())
    }

    /// Building a netlist database STEP 2:
    /// Assign directions to netlist pins given the macro and the pin type.
    #[must_use]
    fn assign_direction(
        &mut self,
        (_top_name, top_m, top_mm): (&CompactString, &SVerilogModule, &ModuleMap),
        lib: &impl DirectionProvider
    ) -> Option<()> {
        // query the provider for cell pins
        self.pindirect = self.pinid2logicpinid.iter()
            .enumerate()
            .map(|(i, logic_id)| {
                match self.logicpintypes[*logic_id] {
                    LogicPinType::TopPort => Direction::Unknown,
                    LogicPinType::LeafCellPin => {
                        let macro_name = &self.celltypes[self.pin2cell[i]];
                        let pin_name = &self.pinnames[i];
                        lib.direction_of(
                            macro_name, &pin_name.1, pin_name.2)
                    }
                    _ => unreachable!()
                }
            })
            .collect();

        // query the module definition for ports
        for port in &top_m.ports {
            use ExprBit::*;
            let (name, ref_names) = match port {
                SVerilogPortDef::Basic(name) => {
                    (name, Either::Left(std::iter::repeat(name)))
                }
                SVerilogPortDef::Conn(name, expr) => {
                    (name, Either::Right(
                        top_mm.eval_expr(expr).map(|eb| match eb {
                            Const(_) => {
                                clilog::error!(NL_SV_LIT, "Literal unsupported");
                                panic!() // for simplicity here.
                            }
                            Var(pname, _) => pname
                        })
                    ))
                }
            };
            let width = top_mm.port_widths.get(name).copied();
            for (id, ref_name) in enum_in_width(width).zip(ref_names) {
                let k = (HierName::empty(), name.clone(), id);
                let deftype = match top_mm.def_types.get(ref_name) {
                    Some(v) => v,
                    None => {
                        clilog::error!(
                            NL_SV_REF, "io reference {}/{}{:?} not found,\
                                        required for direction discovery.",
                            k.0, k.1, k.2);
                        return None
                    }
                };
                use WireDefType::*;
                use Direction::*;
                let dir = match deftype {
                    Input => O,  // input port is net output.
                    Output => I,
                    InOut => {
                        clilog::warn!(NL_SV_INOUT, "inout unsupported for pin {}/{}{:?}, treating as unknown. TODO",
                                      k.0, k.1, k.2);
                        Unknown
                    }
                    Wire => {
                        clilog::error!(
                            NL_SV_REF, "named port connection {} should \
                                        not refer to non-io wire {}.",
                            name, ref_name);
                        return None
                    }
                };
                self.pindirect[*self.pinname2id.get(&k).unwrap()] = dir;
            }
        }

        let num_unknowns = self.pindirect.iter()
            .filter(|t| **t == Direction::Unknown)
            .count();

        if num_unknowns != 0 && lib.should_warn_missing_directions() {
            clilog::warn!(NL_SV_DIRUNK, "There are {} pins with unknown \
                                         directions",
                          num_unknowns);
        }

        self.post_assign_direction()?;
        Some(())
    }

    /// Convenient shortcut to read from file.
    /// The parameters are similar to [NetlistDB::from_sverilog].
    pub fn from_sverilog_file(
        sverilog_source_path: impl AsRef<std::path::Path>,
        top: Option<&str>,
        direction_provider: &impl DirectionProvider
    ) -> Option<NetlistDB> {
        let sverilog = match SVerilog::parse_file(&sverilog_source_path) {
            Ok(sv) => sv,
            Err(e) => {
                clilog::error!(
                    NL_SV_PARSE,
                    "Parse sverilog file {} failed: {}",
                    sverilog_source_path.as_ref().display(), e);
                return None
            }
        };
        NetlistDB::from_sverilog(sverilog, top, direction_provider)
    }

    /// Convenient shortcut to read from a source string.
    /// The parameters are similar to [NetlistDB::from_sverilog].
    pub fn from_sverilog_source(
        sverilog_source: &str,
        top: Option<&str>,
        direction_provider: &impl DirectionProvider
    ) -> Option<NetlistDB> {
        let sverilog = match SVerilog::parse_str(sverilog_source) {
            Ok(sv) => sv,
            Err(e) => {
                clilog::error!(
                    NL_SV_PARSE,
                    "Parse sverilog source code failed: {}", e);
                return None
            }
        };
        NetlistDB::from_sverilog(sverilog, top, direction_provider)
    }
}
