use sverilogparse::SVerilog;
use std::env;

fn main() {
    clilog::init_stderr_color_debug();
    let args: Vec<String> = env::args().collect();
    assert!(args.len() == 2,
            "Usage: {} <sverilog_path>", args[0]);
    clilog::info!("Verilog file {}", args[1]);

    let sv = match SVerilog::parse_str(&std::fs::read_to_string(&args[1]).unwrap()) {
        Ok(sv) => sv,
        Err(e) => panic!("{}", e)
    };

    clilog::info!("# Modules = {}", sv.modules.len());
    clilog::info!("# Cell lines = {}", sv.modules.iter().map(|m| m.1.cells.len())
                  .sum::<usize>());

    drop(sv);
    clilog::info!("cleaned up, exiting..")
}
