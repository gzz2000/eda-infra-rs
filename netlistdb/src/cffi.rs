//! This implements the C foreign-function interface, allowing one
//! to build a netlistdb object from native C++ netlist databases
//! in C++ projects like DREAMPlace.
//!
//! In the project root, run
//! ``` sh
//! cbindgen --config cbindgen.toml
//! ```
//! to generate a C bindings header.

use super::*;
use compact_str::CompactString;
use std::collections::HashMap;
use std::ffi::CStr;
use std::os::raw::c_char;
use regex::Regex;

/// This is a C-compatible netlist database interface
/// that one need to build on their C-side and passed into
/// [netlistdb_new].
///
/// Currently it only has basic functionality.
/// Various functions like hierarchy parsing and constant-tied
/// nets (0/1) are NOT supported.
#[repr(C)]
pub struct NetlistDBCppInterface {
    /// Gives the top design name as a NULL-terminated string.
    ///
    /// If this is not a valid string, the top design name
    /// will be set to empty.
    pub top_design_name: *const c_char,
    /// Gives the number of cells
    pub num_cells: usize,
    /// Gives the number of pins
    pub num_pins: usize,
    /// Gives the number of top-level ports
    pub num_ports: usize,
    /// Gives the number of nets
    pub num_nets: usize,
    /// Gives the number of nets that are tied to 0.
    pub num_nets_zero: usize,
    /// Gives the number of nets that are tied to 1.
    pub num_nets_one: usize,
    /// Gives an array of pointers to NULL-terminated string,
    /// indicating the cell names.
    ///
    /// The cell name hierarchy is NOT handled at this moment.
    /// If the name contains '/', it will not be interpreted,
    /// but will rather be treated as a whole name.
    pub cellname_array: *const *const c_char,
    /// Gives an array of pointers to NULL-terminated string,
    /// indicating the cell macro types (e.g., ANDV1).
    pub celltype_array: *const *const c_char,
    /// Gives an array of pointers to NULL-terminated string,
    /// indicating the pin names.
    ///
    /// The pin name hierarchy is NOT handled at this moment.
    /// If the name contains '/', it will not be interpreted,
    /// but will rather be treated as a whole name.
    ///
    /// The pin macro type (the short string follow ':') will
    /// be parsed separately though.
    /// If there is no ':' in the string given, the pin will
    /// be treated as a top port.
    pub pinname_array: *const *const c_char,
    /// Gives an array of pointers to NULL-terminated string,
    /// indicating the net names.
    ///
    /// The net name hierarchy is NOT handled at this moment.
    /// If the name contains '/', it will not be interpreted,
    /// but will rather be treated as a whole name.
    pub netname_array: *const *const c_char,
    /// Gives an array of u8 the length of num_pins,
    /// each represents a direction.
    ///
    /// 0 means input ([Direction::I]),
    /// 1 means output ([Direction::O]),
    /// all others will be treated as [Direction::Unknown].
    ///
    /// Caution: the input and output follows a
    /// standard-cell-oriented definition. This means a top
    /// input port is actually an **output** pin, and vice versa.
    /// If your external database defines it differently,
    /// you need to invert the direction of top ports before
    /// filling in this struct.
    pub pindirection_array: *const u8,
    /// Gives an array of cell indices that each pin belongs to.
    pub pin2cell_array: *const usize,
    /// Gives an array of net indices that each pin belongs to.
    ///
    /// The net to pin CSR will be built according to this.
    /// It will then be ordered according to the direction given.
    pub pin2net_array: *const usize,
    /// Gives an array of net indices that are tied to 0.
    pub nets_zero_array: *const usize,
    /// Gives an array of net indices that are tied to 1.
    pub nets_one_array: *const usize,
}

/// Build a Rust-side netlistdb object from the given
/// C-compatible netlist information ([NetlistDBCppInterface]).
///
/// The returning pointer **owns** a **[Box]** of [NetlistDB],
/// that must be retrieved in other Rust code using
/// [Box::from_raw].
#[no_mangle]
pub extern "C" fn netlistdb_new(netlist_ptr: &NetlistDBCppInterface) -> *mut NetlistDB {
    // top_name
    let name = unsafe {
        let top_name_str = CStr::from_ptr((*netlist_ptr).top_design_name).to_str();
        match top_name_str {
            Ok(s) => CompactString::new_inline(s),
            Err(_) => CompactString::default(),
        }
    };
    // nums
    let num_cells =  netlist_ptr.num_cells;
    let num_pins  =  netlist_ptr.num_pins;
    let num_ports =  netlist_ptr.num_ports;
    let num_nets  =  netlist_ptr.num_nets;
    let num_nets_zero = netlist_ptr.num_nets_zero;
    let num_nets_one = netlist_ptr.num_nets_one;
    clilog::info!(
        NLCFFI_INFO,
        "NetlistDB: {} cells, {} pins, {} ports, {} nets, {}/{} zero/one constant nets",
        num_cells, num_pins, num_ports, num_nets, num_nets_zero, num_nets_one
    );

    // cellname_vec cellname2id_map
    let cellname_array: *const *const c_char =  netlist_ptr.cellname_array;
    let cellnames = (0..num_cells).map(|i| {
        let cell_name = unsafe { CStr::from_ptr(*cellname_array.add(i)) };
        let cur = CompactString::new(cell_name.to_str().unwrap());
        HierName { cur, prev: None }
    }).collect::<Vec<_>>();
    let cellname2id = cellnames.iter().enumerate().map(|(i, hier)| (hier.clone(), i)).collect::<HashMap<_, _>>();

    // celltype_vec
    let celltype_array: *const *const c_char = netlist_ptr.celltype_array;
    let celltypes = (0..num_cells).map(|i| {
        let cell_type = unsafe { CStr::from_ptr(*celltype_array.add(i)) };
        CompactString::new( cell_type.to_str().unwrap())
    }).collect::<Vec<_>>();

    // pindirect
    let pindirection_array: *const u8 = netlist_ptr.pindirection_array;
    let pindirect = (0..num_pins).map(|i| {
        let value = unsafe { *pindirection_array.add(i) };
        let direction = match value {
            0 => Direction::I,
            1 => Direction::O,
            _ => Direction::Unknown,
        };
        direction
    }).collect::<Vec<_>>();

    // pinname_vec pinname2id_map
    let pinname_array: *const *const c_char = netlist_ptr.pinname_array;
    let re = Regex::new(r"^(.*)\[(\d+)\]$").unwrap();
    let pinnames = (0..num_pins).map(|i| {
        let pin_name_ptr = unsafe { CStr::from_ptr(*pinname_array.add(i)).to_str().unwrap() };
        if let Some((hier_name, pin_name)) = pin_name_ptr.split_once(':') {
            let cur = CompactString::new(hier_name);

            if let Some(captures) = re.captures(pin_name) {
                let name_without_brackets = CompactString::from(&captures[1]);
                let number = captures[2].parse::<u32>().ok();
                let number_isize: Option<isize> = number.map(|n| n as isize);
                ( HierName { cur, prev: None }, name_without_brackets, number_isize )
            } else {
                ( HierName { cur, prev: None }, CompactString::from(pin_name), None )
            }
        } else {
            let pin_name = CompactString::new(pin_name_ptr);
            ( HierName::empty(), CompactString::from(pin_name.to_string()), None )
        }
    }).collect::<Vec<_>>();
    let pinname2id = pinnames.iter().enumerate().map(|(i, pinname)| (pinname.clone(), i)).collect::<HashMap<_, _>>();

    // netname_vec netname2id_map
    let netname_array: *const *const c_char = netlist_ptr.netname_array;
    let netnames = (0..num_nets).map(|i| {
        let net_name_ptr = unsafe { CStr::from_ptr(*netname_array.add(i)).to_str().unwrap() };
        ( HierName::empty(), CompactString::from(net_name_ptr.to_string()), None)   
    }).collect::<Vec<_>>();
    let netname2id = netnames.iter().enumerate().map(|(i, netname)| (netname.clone(), i)).collect::<HashMap<_, _>>();

    let pin2cell: UVec<usize> = unsafe { 
        std::slice::from_raw_parts(netlist_ptr.pin2cell_array, num_pins) 
    }.to_vec().into();    

    // construct CSR of cell
    let cell2pin = VecCSR::from(num_cells, num_pins, &pin2cell);

    // portname2pinid_map
    let portname2pinid = cell2pin.iter_set(0).map(|i| {
        let port_name = pinnames[i].dbg_fmt_pin().clone();
        ((CompactString::from(port_name), None), i)
    }).collect::<HashMap<_, _>>();

    if portname2pinid.len() != num_ports {
        clilog::error!(
            NLCFFI_PORT,
            "Mismatch in port name count: Rust has {}, but C++ has {}",
            portname2pinid.len(),
            num_ports
        );
    }    

    let pin2net: UVec<usize> = unsafe { 
        std::slice::from_raw_parts(netlist_ptr.pin2net_array, num_pins) 
    }.to_vec().into();    

    // construct CSR of net
    let net2pin = VecCSR::from(num_nets, num_pins, &pin2net);

    let constant_nets = unsafe {
        std::slice::from_raw_parts(netlist_ptr.nets_zero_array, num_nets_zero)
    }.iter().map(|n| (*n, false)).chain(unsafe {
        std::slice::from_raw_parts(netlist_ptr.nets_one_array, num_nets_one)
    }.iter().map(|n| (*n, true))).collect();

    // construct the netlistdb instance
    let mut netlistdb_instance = NetlistDB {
        name, num_cells,
        num_logic_pins: 0,
        logicpinname2id: HashMap::new(),
        num_pins, num_nets,
        cellname2id, pinname2id,
        netname2id, portname2pinid,
        celltypes, cellnames,
        logicpintypes: Vec::new(),
        logicpinnames: Vec::new(),
        pinid2logicpinid: Vec::new(),
        netnames, pinnames,
        pin2cell, pin2net,
        cell2pin, net2pin,
        pindirect: pindirect.into(),
        cell2noutputs: UVec::new(),
        constant_nets,
        #[cfg(feature = "molt")] molt_names_cache: Default::default(),
    };

    if netlistdb_instance.post_assign_direction().is_none() {
        clilog::error!(NLCFFI_DIRECT, "Error occurred during direction assignment. this error will be ignored atm.");
    }

    // return the raw ptr
    let raw_ptr = Box::into_raw(Box::new(netlistdb_instance));
    raw_ptr
}


/// Converts a placement nodeid to a timer cellid by adjusting the value of `nodeid`.
/// 
/// - `nodeid` and `cellid` are identifiers used in different contexts. 
///   - `nodeid`: Represents the identifier used in the placement or netlist domain.
///   - `cellid`: Represents the identifier used in the timing analysis (`timer`) domain.
/// - The difference arises because the `timer` internally maintains identifiers for 
///   the top-level design.
/// 
/// - The return value represents the corresponding index of the cell in the `timer` netlist database.
#[no_mangle]
pub extern "C" fn convert_place_nodeid_to_timer_cellid(nodeid: usize) -> usize {
    nodeid + 1
}
