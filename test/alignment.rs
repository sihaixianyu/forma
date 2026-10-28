use json_lens::alignment::align;

#[test]
fn inserted_and_deleted_blocks_leave_following_lines_aligned() {
    let left = "{\n  \"a\": 1,\n  \"removed\": {\n    \"nested\": true\n  },\n  \"last\": 2\n}\n";
    let right = "{\n  \"a\": 1,\n  \"last\": 2\n}\n";
    let aligned = align(left, right);
    assert_eq!(aligned.left.rows[5], aligned.right.rows[2]);
    assert_eq!(aligned.left.rows[6], aligned.right.rows[3]);
    assert_eq!(aligned.left.rows.len(), left.split('\n').count());
    assert_eq!(aligned.right.rows.len(), right.split('\n').count());
    assert!(aligned.right.rows[2] - aligned.right.rows[1] > 1);
    assert!(aligned.left.changed[2]);
    assert!(!aligned.right.changed[2]);
    let reverse = align(right, left);
    assert_eq!(reverse.right.rows, aligned.left.rows);
    assert_eq!(reverse.left.rows, aligned.right.rows);
}

#[test]
fn replacement_blocks_share_height_without_mutating_text_or_line_numbers() {
    let left = "{\n\"value\": \"中文\",\n\"last\": true\n}";
    let right = "{\n\"value\": [\n  1,\n  2\n],\n\"last\": true\n}";
    let aligned = align(left, right);
    assert_eq!(aligned.left.source, left);
    assert_eq!(aligned.right.source, right);
    assert_eq!(aligned.left.rows[1], aligned.right.rows[1]);
    assert_eq!(aligned.left.rows[2], aligned.right.rows[5]);
    assert_eq!(aligned.left.rows[3], aligned.right.rows[6]);
    assert_eq!(aligned.height, 7);
}

#[test]
fn indentation_and_empty_documents_keep_valid_row_maps() {
    let aligned = align("{\n  \"a\": 1\n}", "{\n    \"a\": 1\n}");
    assert_eq!(aligned.left.rows, aligned.right.rows);
    assert!(aligned.left.changed.iter().all(|changed| !changed));
    for (left, right) in [("", ""), ("", "a\nb\n"), ("a\nb", "")] {
        let aligned = align(left, right);
        assert_eq!(aligned.left.rows.len(), left.split('\n').count());
        assert_eq!(aligned.right.rows.len(), right.split('\n').count());
        for rows in [&aligned.left.rows, &aligned.right.rows] {
            assert!(rows.windows(2).all(|pair| pair[0] < pair[1]));
            assert!(rows.iter().all(|row| *row < aligned.height));
        }
    }
}
