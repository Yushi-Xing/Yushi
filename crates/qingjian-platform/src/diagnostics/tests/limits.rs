//! 导出读取量与文件数的实际边界；大文件用稀疏文件准备，不制造无界分配。

use std::fs;

use crate::diagnostics::DiagnosticExport;

#[test]
fn log_reads_stop_at_per_file_and_total_limits() {
    let source = tempfile::tempdir().unwrap();
    for day in 1..=6 {
        let file =
            fs::File::create(source.path().join(format!("server.2026-10-{day:02}.log"))).unwrap();
        file.set_len(9 * 1024 * 1024).unwrap();
    }
    let export = DiagnosticExport::prepare(source.path(), None).unwrap();
    let lengths: Vec<_> = fs::read_dir(export.path())
        .unwrap()
        .map(|entry| entry.unwrap())
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "log"))
        .map(|entry| entry.metadata().unwrap().len())
        .collect();
    assert_eq!(lengths.len(), 4);
    assert!(lengths.iter().all(|&length| length == 8 * 1024 * 1024));
    assert_eq!(lengths.iter().sum::<u64>(), 32 * 1024 * 1024);
}

#[test]
fn too_many_logs_export_at_most_32_files() {
    let source = tempfile::tempdir().unwrap();
    for month in [9, 10] {
        for day in 1..=20 {
            fs::write(
                source
                    .path()
                    .join(format!("tsf.2026-{month:02}-{day:02}.log")),
                "fixture\n",
            )
            .unwrap();
        }
    }
    let export = DiagnosticExport::prepare(source.path(), None).unwrap();
    let count = fs::read_dir(export.path())
        .unwrap()
        .map(|entry| entry.unwrap())
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "log"))
        .count();
    assert_eq!(count, 32);
}
