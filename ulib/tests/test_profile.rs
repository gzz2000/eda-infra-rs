
#[test]
fn test_profile() {
    clilog::init_stdout_simple_trace();
    ulib::profile::log_memory_stats();
}
