use netlistdb::NetlistDB;
use std::env;

fn main() {
    clilog::init_stderr_color_debug();
    clilog::enable_timer("netlistdb");
    let args: Vec<String> = env::args().collect();
    assert!(args.len() == 2 || args.len() == 3,
            "Usage: {} <verilog_path> [<top_module>]", args[0]);

    let db = NetlistDB::from_sverilog_file(
        &args[1],
        args.get(2).map(|x| x.as_ref()),
        &netlistdb::NoDirection
    ).expect("Error parsing the verilog into netlist");

    println!("Benchmark statistics for {}", args[1]);
    println!("top module: {}", db.name);
    println!("num cells:  {}", db.num_cells);
    println!("num nets:   {}", db.num_nets);
    println!("num pins:   {}", db.num_pins);
}
