module simple2_submodule_doubleinv(.n3_port({n3_x}), out);
   input [3:3] n3_x;
   wire  n4;
   output [1:1] out;
   
   INV_X1 u2 ( .a(n3_x[3]), .o(n4) );
   INV_X2 u3 ( .a(n4), .o(out) );
endmodule

module simple2_test (
                     inp1,
                     .real_inp2({inp2}),
                     tau2015_clk,
                     out
                     );

   // Start PIs
   input inp1;
   input inp2;
   input       tau2015_clk;

   // Start POs
   output      out;

   // Start wires
   wire [3:1]  n;
   
   wire        tau2015_clk;
   wire        out;

   wire [1:1]       n3_1;

   assign n3_1 = n[3];

   // Start cells
   NAND2_X1 u1 ( .a(inp1), .b(inp2), .o(n[1]) );
   DFF_X80 f1 ( .d(n[2]), .ck(tau2015_clk), .q(n[3]) );
   // INV_X1 u2 ( .a(n[3]), .o(n4_1) );
   // INV_X2 u3 ( .a(n[4]), .o(out) );
   
   simple2_submodule_doubleinv dins1(.n3_port(n[3]), .out(out1));
   simple2_submodule_doubleinv dins2(.n3_port(n3_1), .out(out2));
   NAND2_X1 ud12(.a(out1), .b(out2), .o(out));
   
   NOR2_X1 u4 ( .a(n[1]), .b(n[3]), .o(n[2]) );

   wire             nzero;
   assign nzero = 1'b0;

endmodule // simple2_test
