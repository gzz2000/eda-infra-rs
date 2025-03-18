// from https://www.chipverify.com/verilog/verilog-assign-statement

// previously, input/output and vector widths are specified directly
// in defs, and here we moved them to body. -- because PT also
// does not support such syntax.
// 
module xyz (x,		// x is a 4-bit vector net
						y, 		// y is a scalar net (1-bit)
						z ); 	// z is a 5-bit vector net

   input [3:0]            x;
   input                  y;
   output [4:0]           z;

wire [1:0] 	a;
wire 				b;

// Assume one of the following assignments are chosen in real design
// If x='hC and y='h1 let us see the value of z

// Case #1: 4-bits of x and 1 bit of y is concatenated to get a 5-bit net
// and is assigned to the 5-bit nets of z. So value of z='b11001 or z='h19
assign z = {x, y};

// Case #2: 4-bits of x and 1 bit of y is concatenated to get a 5-bit net
// and is assigned to selected 3-bits of net z. Remaining 2 bits of z remains
// undriven and will be high-imp. So value of z='bZ001Z
assign z[3:1] = {x, y};

// Case #3: The same statement is used but now bit4 of z is driven with a constant
// value of 1. Now z = 'b1001Z because only bit0 remains undriven
assign z[3:1] = {x, y};
// assign z[4] = 1;    // TODO: decimal literal unsupported yet.
   assign z[4] = 1'b1;

// Case #4: Assume bit3 is driven instead, but now there are two drivers for bit3,
// and both are driving the same value of 0. So there should be no contention and
// value of z = 'bZ001Z
assign z[3:1] = {x, y};
// assign z[3] = 0;   // TODO: decimal literal unsupported yet.

// Case #5: Assume bit3 is instead driven with value 1, so now there are two drivers
// with different values, where the first line is driven with the value of X which
// at the time is 0 and the second assignment where it is driven with value 1, so
// now it becomes unknown which will win. So z='bZX01Z
assign z[3:1] = {x, y};
// assign z[3] = 1;   // TODO: decimal literal unsupported yet.

// Case #6: Partial selection of operands on RHS is also possible and say only 2-bits
// are chosen from x, then z = 'b00001 because z[4:3] will be driven with 0
assign z = {x[1:0], y};

// Case #7: Say we explicitly assign only 3-bits of z and leave remaining unconnected
// then z = 'bZZ001
assign z[2:0] = {x[1:0], y};

// Case #8: Same variable can be used multiple times as well and z = 'b00111
// 3{y} is the same as {y, y, y}
/// assign z = {3{y}};    /// TODO: replication not supported yet.

// Case #9: LHS can also be concatenated: a is 2-bit vector and b is scalar
// RHS is evaluated to 11001 and LHS is 3-bit wide so first 3 bits from LSB of RHS
// will be assigned to LHS. So a = 'b00 and b ='b1
assign {a, b} = {x, y};

// Case #10: If we reverse order on LHS keeping RHS same, we get a = 'b01 and b='b0
assign {a, b} = {x, y};

endmodule
