//! A flattened gate-level circuit netlist database.

use std::collections::HashMap;
use std::sync::Arc;
use std::collections::HashSet;
use compact_str::CompactString;
use ulib::{UVec, Device, UniversalCopy, Zeroable};

/// types of directions: input or output.
/// 
/// note: inout is not supported yet.
/// **should be identical to `csrc/lib.h`**.
#[derive(Zeroable, Debug, PartialEq, Eq, Clone, UniversalCopy)]
#[repr(u8)]
pub enum Direction {
    /// input
    I = 0,
    /// output
    O = 1,
    /// unknown (unassigned)
    Unknown = 2
}

mod csr;
pub use csr::VecCSR;

mod hier_name;
pub use hier_name::{
    HierName, GeneralHierName,
    GeneralPinName, RefPinName,
    GeneralMacroPinName, RefMacroPinName
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum LogicPinType {
    TopPort,
    Net,
    LeafCellPin,
    Others
}

impl LogicPinType {
    #[inline]
    pub fn is_pin(self) -> bool {
        use LogicPinType::*;
        if let TopPort | LeafCellPin = self { true } else { false }
    }

    #[inline]
    pub fn is_net(self) -> bool {
        use LogicPinType::*;
        if let TopPort | Net = self { true } else { false }
    }
}

/// The netlist storage.
/// 
/// The public members are all READ-ONLY outside. Please modify
/// them through the ECO commands that will be available
/// in the future.
#[readonly::make]
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct NetlistDB {
    /// top-level design name.
    pub name: CompactString,
    /// number of cells/nodes/instances in the netlist.
    ///
    /// This is always greater than 1, as the 0th cell is always
    /// the top-level macro.
    pub num_cells: usize,
    /// number of logical pins.
    /// 
    /// A logical pin is not necessarily a pin. It might
    /// be the I/O port of non-leaf modules, or the result
    /// of an assign operation.
    pub num_logic_pins: usize,
    /// number of pins.
    pub num_pins: usize,
    /// number of nets/wires.
    pub num_nets: usize,

    /// Cell name to index.
    ///
    /// The top-level macro is always the 0th cell, which has a
    /// special name of empty string.
    /// Also, the hierarchical non-leaf cells do NOT reside in here,
    /// yet -- they are to-be-added in the future.
    /// This map only contains leaf cells.
    pub cellname2id: HashMap<HierName, usize>,
    /// Logical pin name tuple (cell hier name, macro pin type, vec idx) to logical pin index.
    ///
    /// Logic pin names are always unique without ambiguity.
    /// The case of logic pins include:
    /// 1. net wires  (yes, nets are also ``logic pins''.)
    /// 2. I/O ports of top module and submodules
    /// 3. pins of leaf cells.
    logicpinname2id: HashMap<(HierName, CompactString, Option<isize>), usize>,
    /// Pin name tuple (cell hier name, macro pin type, vec idx) to index.
    /// 
    /// Pin names are always unique without ambiguity.
    /// For top-level named port connections, only the port names are
    /// created as valid pin names. The I/O definition can be referred
    /// in logicpinname2id (private member).
    pub pinname2id: HashMap<(HierName, CompactString, Option<isize>), usize>,
    /// Net name tuple (net hier name, vec idx) to index.
    ///
    /// Multiple nets can be mapped to one single
    /// index, due to connected nets across hierarchy boundaries.
    pub netname2id: HashMap<(HierName, CompactString, Option<isize>), usize>,
    /// Port name tuple (port name, vec idx) to pin index.
    ///
    /// For a design with only normal ports, this is a subset of pinname2id,
    /// but for named ports like .port({portx, porty}),
    /// pinname2id will store port\[0\] and port\[1\], where portname2id
    /// will store portx and porty.
    pub portname2pinid: HashMap<(CompactString, Option<isize>), usize>,

    /// Cell index to macro name.
    pub celltypes: Vec<CompactString>,
    /// Cell index to name (hierarchical).
    ///
    /// This information actually contains the tree structure that
    /// might be useful later when we implement verilog writer.
    pub cellnames: Vec<HierName>,
    /// Logic pin classes.
    logicpintypes: Vec<LogicPinType>,
    /// Logic pin index to name.
    logicpinnames: Vec<(HierName, CompactString, Option<isize>)>,
    /// Pin index to corresponding logic pin index.
    pinid2logicpinid: Vec<usize>,
    /// Net index to net hier and index.
    pub netnames: Vec<(HierName, CompactString, Option<isize>)>,
    /// Pin index to cell hier, macro pin name, and pin index.
    pub pinnames: Vec<(HierName, CompactString, Option<isize>)>,

    /// Pin to parent cell.
    pub pin2cell: UVec<usize>,
    /// Pin to parent net.
    pub pin2net: UVec<usize>,
    /// Cell CSR.
    pub cell2pin: VecCSR,
    /// Net CSR.
    ///
    /// **Caveat**: After assigning directions, it is guaranteed that
    /// the net root would be the first in net CSR.
    /// Before such assignment, the order is not determined.
    pub net2pin: VecCSR,

    /// Pin direction.
    pub pindirect: UVec<Direction>,

    /// Cell to number of outputs
    pub cell2noutputs: UVec<usize>,

    /// Net indices tied to low (VSS, false) or high (VDD, true).
    pub constant_nets: HashMap<usize, bool>,
}

impl NetlistDB {
    /// This changes the type (i.e. macro name) of a leaf cell.
    pub fn change_cell_type(&mut self, cellid: usize, new_cell_type: CompactString) {
        self.celltypes[cellid] = new_cell_type;
    }
}

mod utils;
use utils::*;

mod disjoint_set;
use disjoint_set::*;

mod builder;
pub use builder::{LeafPinProvider, NoDirection};

#[doc(hidden)]
pub use builder::DirectionProvider;

pub mod cffi;
