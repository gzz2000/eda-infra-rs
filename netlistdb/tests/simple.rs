use netlistdb::*;
use std::fs;
use compact_str::CompactString;

#[test]
fn simple() {
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
    
    let db: NetlistDB = NetlistDB::from_sverilog_source(
        &verilog, None, &directions
    ).unwrap();
    
    println!("The database: {db:#?}");
    assert_eq!(db.num_cells, 4);
    assert_eq!(db.num_pins, 12);
    assert_eq!(db.num_nets, 6);
    assert_eq!(db.pin2cell, vec![0, 0, 0, 0, 1, 1, 1, 2, 2, 2, 3, 3].into());
    assert_eq!(db.pin2net, vec![0, 1, 2, 3, 0, 1, 4, 4, 2, 5, 5, 3].into());
    assert_eq!(db.cell2pin.start, vec![0, 4, 7, 10, 12].into());
    assert_eq!(db.cell2pin.items, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11].into());
    assert_eq!(db.net2pin.start, vec![0, 2, 4, 6, 8, 10, 12].into());

    // after direction assignment, `11` advances before `3`.
    assert_eq!(db.net2pin.items, vec![0, 4, 1, 5, 2, 8, 11, 3, 6, 7, 9, 10].into());
    use Direction::*;
    assert_eq!(db.pindirect, vec![O, O, O, I, I, I, O, I, I, O, I, O].into());
    assert_eq!(db.cell2noutputs, vec![3, 1, 1, 1].into());

    assert_eq!(db.net_zero, None);
    assert_eq!(db.net_one, None);
}
