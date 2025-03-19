//! unit tests for sverilogparse

use sverilogparse::*;

const VERILOG_SIMPLE: &str = include_str!("simple.v");
const VERILOG_NOT_CONNECTED: &str = include_str!("not_connected.v");

#[test]
fn test_simple() {
  clilog::init_stdout_simple_trace();
  let parsed = SVerilog::parse_str(VERILOG_SIMPLE).expect("parse error");
  println!("Parsed is: {parsed:?}");
  println!("Restructure: \n{parsed}");
  assert_eq!(format!("{parsed}"), "\
module simple(inp1, inp2, ispd_clk, out);
  input inp1;
  input inp2;
  input ispd_clk;
  output out;
  wire n1;
  wire n2;
  wire inp1;
  wire inp2;
  wire ispd_clk;
  wire out;

  na02s01 u1(.a(inp1), .b(inp2), .o(n1));
  ms00f80 f1(.d(n1), .ck(ispd_clk), .o(n2));
  in01s01 u2(.a(n2), .o(out));
endmodule
");
}

const VERILOG_VECTOR: &str = include_str!("vector.v");

#[test]
fn test_vector() {
  clilog::init_stdout_simple_trace();
  let parsed = SVerilog::parse_str(VERILOG_VECTOR).expect("parse error");
  println!("Restructure: \n{parsed}");
  assert_eq!(format!("{parsed}"), "\
module aes256(VGND, VPWR, clk, key, out, state);
  input VGND;
  input VPWR;
  input clk;
  input [255:0] key;
  output [127:0] out;
  inout [127:0] state;
  wire [255:0] long_wire;

  assign long_wire = {128'b11011110101011011011xxxxxxxx1111000100111000001111000000100110100101100000011000001000001110110000101101001111100100100001001101, 128'b11111100110000100100011010101000001100100001001011100101101101100101101110110000111011101101100100010010111111010010101110011010};
  anothercell x(.in({key[32:1], 4'bxx01, 3'b01z, state}), .out(out));
endmodule
");
}

const VERILOG_ASSIGN: &str = include_str!("assign.v");

#[test]
fn test_assign() {
  clilog::init_stdout_simple_trace();
  let parsed = SVerilog::parse_str(VERILOG_ASSIGN).expect("parse error");
  println!("Restructure: \n{parsed}");
  assert_eq!(format!("{parsed}"), "\
module xyz(x, y, z);
  input [3:0] x;
  input y;
  output [4:0] z;
  wire [1:0] a;
  wire b;

  assign z = {x, y};
  assign z[3:1] = {x, y};
  assign z[3:1] = {x, y};
  assign z[4] = 1'b1;
  assign z[3:1] = {x, y};
  assign z[3:1] = {x, y};
  assign z = {x[1:0], y};
  assign z[2:0] = {x[1:0], y};
  assign {a, b} = {x, y};
  assign {a, b} = {x, y};
endmodule
");
}

const VERILOG_PORTDEF: &str = include_str!("portdef.v");

#[test]
fn test_portdef() {
  clilog::init_stdout_simple_trace();
  let parsed = SVerilog::parse_str(VERILOG_PORTDEF).expect("parse error");
  println!("Restructure: \n{parsed}");
  assert_eq!(format!("{parsed}"), "\
module portdef_test_pulpino_top(clk, rst_n, clk_sel_i, clk_standalone_i, testmode_i, fetch_enable_i, scan_enable_i, spi_clk_i, spi_cs_i, spi_mode_o, uart_rx, uart_rts, uart_dtr, uart_cts, uart_dsr, gpio_in, gpio_out, gpio_dir, .gpio_padcfg({\\gpio_padcfg[31][5] , \\gpio_padcfg[30][3] , \\gpio_padcfg[30][2] , \\gpio_padcfg[30][1] }), \\masters[1].w_last );

  TIEHBWP7T35P140 U5(.Z(\\masters[1].w_last ));
  TIELBWP7T35P140 U6(.ZN(net44357));
endmodule
");
}

#[test]
fn test_allow_not_connected() {
  let parsed = SVerilog::parse_str(VERILOG_NOT_CONNECTED).expect("parse error");
  println!("Parsed is: {parsed:?}");
  println!("Restructure: \n{parsed}");
  assert_eq!(format!("{parsed}"), "\
module simple(inp1, inp2, ispd_clk, out);
  input inp1;
  input inp2;
  input ispd_clk;
  output out;
  wire n1;
  wire n2;
  wire inp1;
  wire inp2;
  wire ispd_clk;
  wire out;

  na02s01 u1(.a(inp1), .b(inp2), .o(n1));
  ms00f80 f1(.d(n1), .ck(ispd_clk), .o(n2));
  in01s01 u2(.a(n2));
endmodule
");
}
