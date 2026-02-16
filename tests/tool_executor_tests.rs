use aistar::tools::ToolExecutor;
use tempfile::TempDir;

#[test]
fn test_path_traversal_blocked() {
    let temp = TempDir::new().expect("temp dir");
    let executor = ToolExecutor::new(temp.path().to_path_buf());

    assert!(executor.read_file("../../etc/passwd").is_err());
    assert!(executor.read_file("/etc/passwd").is_err());
    assert!(executor.read_file("..\\windows\\system32").is_err());
}

#[test]
fn test_filename_with_double_dots_allowed() {
    let temp = TempDir::new().expect("temp dir");
    let executor = ToolExecutor::new(temp.path().to_path_buf());

    executor
        .write_file("my..file.txt", "content")
        .expect("should allow legitimate '..' filename");

    let content = executor
        .read_file("my..file.txt")
        .expect("read double-dot filename");
    assert_eq!(content, "content");
}

#[test]
fn test_write_new_file() {
    let temp = TempDir::new().expect("temp dir");
    let executor = ToolExecutor::new(temp.path().to_path_buf());

    executor
        .write_file("new_dir/test.txt", "content")
        .expect("write file");

    let content = executor
        .read_file("new_dir/test.txt")
        .expect("read just-written file");
    assert_eq!(content, "content");
}

#[test]
fn test_edit_file_ambiguous() {
    let temp = TempDir::new().expect("temp dir");
    let executor = ToolExecutor::new(temp.path().to_path_buf());

    executor
        .write_file("test.txt", "foo\nfoo\n")
        .expect("seed file");

    let result = executor.edit_file("test.txt", "foo", "bar");
    assert!(result.is_err());
    assert!(result
        .expect_err("should reject ambiguous edits")
        .to_string()
        .contains("appears 2 times"));
}
