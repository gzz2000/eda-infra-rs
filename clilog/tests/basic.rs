
#[test]
fn test_main() {
    clilog::init_stderr_color_debug();
    clilog::set_default_max_print_count(6);
    clilog::enable_timer("basic");
    let timer_a = clilog::stimer!("timer_a");
    for i in 0..20 {
        clilog::warn!(WTESTBASIC, "basic test is executing {}", i);
    }
    clilog::finish!(timer_a, "done");
}
