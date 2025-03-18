module aes256 (VGND,
               VPWR,
               clk,
               key,
               out,
               state);
   input VGND, VPWR, clk;
   input [255:0] key;
   output [127:0] out;
   inout [127:0]  state;

   wire [255:0]   long_wire;
   assign long_wire = 256'hdeadbxxf_1383c09a_581820ec_2d3e484d_fcc246a83212e5b65bb0eed912fd2b9a;

   anothercell x(.in({key[32:1], {4'bx01, 3'b1z, state}}), .out(out));
endmodule // aes256
