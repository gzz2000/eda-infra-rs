use netlistdb::*;
use compact_str::CompactString;
use itertools::Itertools;

#[test]
fn not_simple() {
    clilog::init_stdout_simple_trace();
    
    let directions = |r#macro: &CompactString, pin: &CompactString, pinwidth: Option<isize>| {
        assert_eq!(pinwidth, None);
        use Direction::*;
        match (r#macro.as_str(), pin.as_str()) {
            (_, "a" | "b" | "ck" | "d") => I,
            (_, "o" | "q") => O,
            _ => Unknown
        }
    };
    
    let db: NetlistDB = NetlistDB::from_sverilog_file(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/notsimple.v"),
        None, &directions
    ).unwrap();
    
    println!("The database: {db:?}");
    assert_eq!(db.num_cells, 9);
    assert_eq!(db.num_pins, 24);
    assert_eq!(db.num_nets, 12);
    assert_eq!(format!("{}", db.cellnames.iter().skip(1).format(", ")),
               "u1, f1, dins1/u2, dins1/u3, dins2/u2, dins2/u3, ud12, u4");  // first one is empty.
    assert_eq!(format!("{}", db.celltypes.iter().format(", ")),
               "simple2_test, NAND2_X1, DFF_X80, INV_X1, INV_X2, INV_X1, INV_X2, NAND2_X1, NOR2_X1");
    assert_eq!(
        format!("{}", db.pinnames.iter()
                .map(|pinname| pinname.dbg_fmt_pin()).format(", ")),
        "inp1, tau2015_clk, out, real_inp2, u1:a, u1:b, u1:o, f1:d, f1:ck, f1:q, dins1/u2:a, dins1/u2:o, dins1/u3:a, dins1/u3:o, dins2/u2:a, dins2/u2:o, dins2/u3:a, dins2/u3:o, ud12:a, ud12:b, ud12:o, u4:a, u4:b, u4:o");

    let mut portnames = db.portname2pinid.iter().map(|((name, idx), &pin)| {
        let idx = match idx {
            Some(v) => format!("[{}]", *v),
            None => "".to_string()
        };
        format!("{}{}={}", name, idx, pin)
    }).collect::<Vec<_>>();
    portnames.sort();
    assert_eq!(
        format!("{}", portnames.iter().format(", ")),
        "inp1=0, inp2=3, out=2, tau2015_clk=1");  // real_inp2 -> inp2

    assert_eq!(db.pin2cell, vec![0, 0, 0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 7, 8, 8, 8].into());
    assert_eq!(db.pin2net, vec![0, 2, 3, 1, 0, 1, 6, 5, 2, 4, 4, 8, 8, 9, 4, 10, 10, 11, 9, 11, 3, 6, 4, 5].into());
    use Direction::*;
    assert_eq!(db.pindirect, vec![O, O, I, O, I, I, O, I, I, O, I, O, I, O, I, O, I, O, I, I, O, I, I, O].into());
    assert_eq!(db.cell2noutputs, vec![3, 1, 1, 1, 1, 1, 1, 1, 1].into());
    
    assert_eq!(db.net_zero, Some(7));
    assert_eq!(db.net_one, None);
}
