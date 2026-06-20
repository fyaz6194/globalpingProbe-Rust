// Integration tests for ProgressBuffer across all three modes

#[test]
fn append_mode_accumulates_across_calls() {
    use globalping_probe::util::progress_buffer::{BufferMode, ProgressBuffer};

    let mut buf = ProgressBuffer::new(BufferMode::Append);
    buf.push("rawOutput", "PING 1.1.1.1\n");
    buf.push("rawOutput", "64 bytes from 1.1.1.1: icmp_seq=1\n");
    buf.push("rawOutput", "64 bytes from 1.1.1.1: icmp_seq=2\n");

    let out = buf.take_progress();
    assert_eq!(
        out["rawOutput"],
        "PING 1.1.1.1\n64 bytes from 1.1.1.1: icmp_seq=1\n64 bytes from 1.1.1.1: icmp_seq=2\n"
    );
}

#[test]
fn diff_mode_emits_only_incremental_output() {
    use globalping_probe::util::progress_buffer::{BufferMode, ProgressBuffer};

    let mut buf = ProgressBuffer::new(BufferMode::Diff);

    buf.push("rawOutput", "line1\n");
    let d1 = buf.take_progress();
    assert_eq!(d1["rawOutput"], "line1\n");

    buf.push("rawOutput", "line1\nline2\n");
    let d2 = buf.take_progress();
    assert_eq!(d2["rawOutput"], "line2\n");

    buf.push("rawOutput", "line1\nline2\nline3\n");
    let d3 = buf.take_progress();
    assert_eq!(d3["rawOutput"], "line3\n");
}
