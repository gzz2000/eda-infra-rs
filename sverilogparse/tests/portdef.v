module portdef_test_pulpino_top ( clk, rst_n, clk_sel_i, clk_standalone_i, testmode_i, 
        fetch_enable_i, scan_enable_i, spi_clk_i, spi_cs_i, spi_mode_o, 
        uart_rx, uart_rts, uart_dtr, uart_cts, uart_dsr, gpio_in, gpio_out, 
        gpio_dir, .gpio_padcfg({\gpio_padcfg[31][5] , 
        \gpio_padcfg[30][3] , \gpio_padcfg[30][2] , \gpio_padcfg[30][1] }),
      \masters[1].w_last );

   TIEHBWP7T35P140 U5 ( .Z(\masters[1].w_last ) );
   TIELBWP7T35P140 U6 ( .ZN(net44357) );
   
endmodule // portdef_test_pulpino_top
