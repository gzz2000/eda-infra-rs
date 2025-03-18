#![cfg(feature = "molt")]

use netlistdb::*;
use std::fs;
use compact_str::CompactString;

#[test]
fn simple_molt() {
    clilog::init_stdout_simple_trace();
    let verilog = fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/simple.v")
    ).expect("unable to read simple.v");
    let directions = |r#macro: &CompactString, pin: &CompactString, pinwidth: Option<isize>| {
        assert_eq!(pinwidth, None);
        use Direction::*;
        match (r#macro.as_str(), pin.as_str()) {
            ("na02s01", "o") => O,
            ("ms00f80", "o") => O,
            ("in01s01", "o") => O,
            _ => I,
        }
    };
    let mut db: NetlistDB = NetlistDB::from_sverilog_source(
        &verilog, None, &directions
    ).unwrap();

    let mut interp = molt_ng::Interp::new();
    let db_ctx = interp.save_context_mut(&mut db);
    NetlistDB::init_molt_commands(db_ctx, &mut interp, true);

    assert_eq!(interp.eval("current_design").unwrap().as_str(), "simple");
    assert_eq!(interp.eval("get_cells").unwrap().as_str(),
               "@cell:1 @cell:2 @cell:3");
    assert_eq!(interp.eval("get_nets").unwrap().as_str(),
               "@net:0 @net:1 @net:2 @net:3 @net:4 @net:5");
    assert_eq!(interp.eval("get_pins").unwrap().as_str(),
               "@pin:0 @pin:1 @pin:2 @pin:3 @pin:4 @pin:5 @pin:6 @pin:7 @pin:8 @pin:9 @pin:10 @pin:11");
    assert_eq!(interp.eval("get_pins {inp1 inp2}").unwrap().as_str(),
               "@pin:0 @pin:1");
    assert_eq!(interp.eval("get_pins {{inp1}}").unwrap().as_str(),
               "@pin:0");
    assert_eq!(interp.eval("get_pins {{inp1} {inp2}}").unwrap().as_str(),
               "@pin:0 @pin:1");
    assert_eq!(interp.eval("get_cells u1").unwrap().as_str(),
               "@cell:1");
    assert_eq!(interp.eval("get_cells u3").is_err(), true);
    assert_eq!(interp.eval("get_cells u*").unwrap().as_str(),
               "@cell:1 @cell:3");
    assert_eq!(interp.eval("get_pins -hierarchical a").unwrap().as_str(),
               "@pin:4 @pin:10");
    assert_eq!(interp.eval("get_nets -regexp {inp[12]}").unwrap().as_str(),
               "@net:0 @net:1");
    assert_eq!(interp.eval("get_pins f1/*").unwrap().as_str(),
               "@pin:7 @pin:8 @pin:9");
    assert_eq!(interp.eval("all_inputs").unwrap().as_str(),
               "@pin:0 @pin:1 @pin:2");
    assert_eq!(interp.eval("all_outputs").unwrap().as_str(),
               "@pin:3");
    assert_eq!(interp.eval("get_pins out").unwrap().as_str(),
               "@pin:3");
    assert_eq!(interp.eval("get_ports real_out").unwrap().as_str(),
               "@pin:3");
}
