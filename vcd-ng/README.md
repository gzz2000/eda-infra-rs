
# VCD Next Generation
This is a fork of [rust-vcd](https://github.com/kevinmehall/rust-vcd) that does some performance-centric changes, including:

* `IdCode` changed to natural byte order, which gives consecutive indices with Synopsys VCS generated trace.

* `FastFlow` is implemented which uses a fast line reader to scan for bit vector changes.

* `CompactString` and `BitVec` are used to represent strings and bits in the original API.

By experiments, `FastFlow` is very fast, but lacks some compatibility with ill-indented file and bad-formed whitespaces. 
`BitVec` actually slows down the program if there are many 1-bit signals.
Please benchmark before you use.

The original version is by Kevin Mehall, with README as follows.

----

# VCD

**[Documentation](https://docs.rs/vcd)** | **[Changelog](https://github.com/kevinmehall/rust-vcd/releases)**

This crate reads and writes [VCD (Value Change Dump)][wp] files, a common format used with logic analyzers, HDL simulators, and other EDA tools. It provides streaming wrappers around the `io::Read` and `io::Write` traits to read and write VCD commands and data.

[wp]: https://en.wikipedia.org/wiki/Value_change_dump
