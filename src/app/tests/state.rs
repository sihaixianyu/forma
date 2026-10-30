use super::*;

#[test]
fn imported_browser_text_uses_the_same_json_and_csv_processing() {
    let json: Arc<String> = Arc::new("{\"中文\":123456789012345678901234567890}".into());
    let Output::Document(doc, _) =
        execute(Job::Import(json.clone(), "example.json".into())).unwrap()
    else {
        panic!("document expected");
    };
    assert_eq!(*doc.text, *json);
    let Output::Document(doc, _) = execute(Job::Import(
        Arc::new("name,value\n中文,42\n".into()),
        "example.CSV".into(),
    ))
    .unwrap() else {
        panic!("document expected");
    };
    assert_eq!(doc.value[0]["name"], "中文");
    assert_eq!(doc.value[0]["value"], "42");
    assert!(matches!(
        execute(Job::Import(Arc::new("{bad}".into()), "bad.json".into())).unwrap(),
        Output::Invalid(..)
    ));
}

#[test]
fn formatting_reuses_graph_and_retains_navigation() {
    let doc = Arc::new(process(Arc::new("{\"b\":2,\"a\":1}".into()), Format::Keep).unwrap());
    let graph = Arc::new(json_lens::graph::build_graph(&doc.value));
    let Output::Document(formatted, reused) =
        execute(Job::Transform(doc.clone(), Format::Pretty, graph.clone())).unwrap()
    else {
        panic!("document expected");
    };
    assert!(Arc::ptr_eq(&graph, &reused));
    assert!(Arc::ptr_eq(&doc.value, &formatted.value));
    let Output::Document(_, sorted) =
        execute(Job::Transform(doc, Format::SortAscending, graph.clone())).unwrap()
    else {
        panic!("document expected");
    };
    assert!(!Arc::ptr_eq(&graph, &sorted));
}

#[test]
fn sort_state_machine_restores_the_exact_original_source() {
    assert!(matches!(
        SortState::Original.after_single_click(),
        SortState::Ascending
    ));
    assert!(matches!(
        SortState::Ascending.after_single_click(),
        SortState::Descending
    ));
    assert!(matches!(
        SortState::Descending.after_single_click(),
        SortState::Original
    ));

    let mut app = ready_app();
    let (sender, receiver) = mpsc::channel();
    app.jobs = sender.into();
    let original = "{\n  \"z\": 0, \"a\": 1,\n  \"nested\": { \"b\": 2, \"a\": 1 }\n}\n";
    app.editor = original.into();
    app.full_text = Arc::new(original.into());
    app.search_source = app.full_text.clone();
    app.dirty = true;

    app.parse(Format::Keep);
    finish(&mut app, receiver.recv().unwrap());
    let initial_graph = serde_json::to_value(app.graph.as_ref()).unwrap();

    app.apply_sort_state(SortState::Ascending);
    finish(&mut app, receiver.recv().unwrap());
    assert!(matches!(app.sort_state, SortState::Ascending));
    assert_ne!(app.editor, original);

    app.apply_sort_state(SortState::Descending);
    finish(&mut app, receiver.recv().unwrap());
    assert!(matches!(app.sort_state, SortState::Descending));
    assert_ne!(app.editor, original);

    app.apply_sort_state(SortState::Original);
    finish(&mut app, receiver.recv().unwrap());
    assert!(matches!(app.sort_state, SortState::Original));
    assert_eq!(app.editor, original);
    assert!(app.sort_original.is_none());
    assert_eq!(
        serde_json::to_value(app.graph.as_ref()).unwrap(),
        initial_graph
    );
}

#[test]
fn stale_auto_parse_cannot_overwrite_more_recent_input() {
    let mut app = ready_app();
    app.revision = 2;
    app.editor = "{\"new\":true}".into();
    app.search_source = Arc::new(app.editor.clone());
    app.dirty = true;
    let (sender, receiver) = mpsc::channel();
    sender
        .send(Completed {
            label: "Validate",
            user_action: false,
            target: None,
            revision: 1,
            reset_view: false,
            result: execute(Job::Process(
                Arc::new("{\"old\":true}".into()),
                Format::Keep,
            )),
            elapsed: Duration::ZERO,
        })
        .unwrap();
    app.completed = receiver;
    app.receive();
    assert_eq!(app.editor, "{\"new\":true}");
    assert!(app.dirty);
}

#[test]
fn pasted_document_over_one_mib_remains_editable_and_updates_the_graph() {
    let mut app = ready_app();
    let (sender, receiver) = mpsc::channel();
    app.jobs = sender.into();
    let text = serde_json::to_string(
        &serde_json::json!({"name": "new document", "padding": "中".repeat(400_000)}),
    )
    .unwrap();
    assert!(text.len() > 1024 * 1024);
    app.editor = text.clone();
    app.search_source = Arc::new(text);
    app.dirty = true;
    app.parse(Format::Keep);
    finish(&mut app, receiver.recv().unwrap());
    assert!(app.editable);
    assert_eq!(app.editor.as_str(), app.full_text.as_str());
    assert_eq!(app.graph.nodes[0].rows[0].value, "\"new document\"");
    app.editor = "{bad}".into();
    app.search_source = Arc::new(app.editor.clone());
    app.dirty = true;
    app.parse(Format::Keep);
    finish(&mut app, receiver.recv().unwrap());
    assert!(app.editable);
}

#[test]
fn new_document_clears_a_removed_selection() {
    let mut app = ready_app();
    app.pointer = "/controls".into();
    let (sender, receiver) = mpsc::channel();
    app.jobs = sender.into();
    app.editor = "{\"replacement\":[1,2]}".into();
    app.search_source = Arc::new(app.editor.clone());
    app.dirty = true;
    app.parse(Format::Keep);
    finish(&mut app, receiver.recv().unwrap());
    assert!(app.pointer.is_empty());
    assert!(!app.graph.nodes.is_empty());
}

#[test]
fn actions_clicked_during_auto_refresh_run_in_order_on_the_latest_document() {
    let mut app = ready_app();
    let (sender, receiver) = mpsc::channel();
    app.jobs = sender.into();
    app.busy = true;
    app.automatic_job = true;
    app.parse(Format::SortAscending);
    app.parse(Format::Compact);
    assert_eq!(app.queued_actions.len(), 2);
    assert!(app.feedback.as_ref().unwrap().text.contains("queued"));
    app.busy = false;
    app.automatic_job = false;
    app.dispatch_next();
    let sort = receiver.recv().unwrap();
    assert_eq!(sort.label, "Sort ascending");
    finish(&mut app, sort);
    let sorted_keys: Vec<_> = app
        .document
        .as_ref()
        .unwrap()
        .value
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    let minify = receiver.recv().unwrap();
    assert_eq!(minify.label, "Minify");
    finish(&mut app, minify);
    let keys: Vec<_> = app
        .document
        .as_ref()
        .unwrap()
        .value
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    assert_eq!(keys, sorted_keys);
    assert!(!app.full_text.contains('\n'));
    assert_eq!(app.feedback.as_ref().unwrap().kind, feedback::Kind::Success);
}

#[test]
fn formatting_unchanged_content_still_reports_success() {
    let mut app = ready_app();
    let (sender, receiver) = mpsc::channel();
    app.jobs = sender.into();
    app.parse(Format::Pretty);
    finish(&mut app, receiver.recv().unwrap());
    assert!(
        app.feedback
            .as_ref()
            .unwrap()
            .text
            .contains("Already formatted")
    );
}

#[test]
fn failed_validation_reports_a_visible_error() {
    let mut app = ready_app();
    let (sender, receiver) = mpsc::channel();
    app.jobs = sender.into();
    app.editor = "{bad}".into();
    app.dirty = true;
    app.parse(Format::Keep);
    finish(&mut app, receiver.recv().unwrap());
    let feedback = app.feedback.as_ref().unwrap();
    assert_eq!(feedback.kind, feedback::Kind::Error);
    assert!(feedback.text.contains("line"));
}

#[test]
fn automatic_comparison_discards_stale_results_and_recovers_from_invalid_input() {
    let mut app = ready_app();
    let (sender, receiver) = mpsc::channel();
    app.jobs = sender.into();
    app.view = View::Compare;
    app.compare_text = "{}".into();
    app.clear_comparison();
    app.tick_automatic_jobs(app.compare_due.unwrap());
    let stale = receiver.recv().unwrap();
    app.revision += 1;
    app.compare_text = "{bad}".into();
    app.clear_comparison();
    finish(&mut app, stale);
    assert!(!app.comparison_done && app.compare_right_range.is_none());
    app.tick_automatic_jobs(app.compare_due.unwrap());
    finish(&mut app, receiver.recv().unwrap());
    assert!(app.error && app.status.starts_with("Right JSON:"));
    assert!(app.compare_due.is_none());
    app.revision += 1;
    app.compare_text = app.editor.clone();
    app.clear_comparison();
    app.tick_automatic_jobs(app.compare_due.unwrap());
    finish(&mut app, receiver.recv().unwrap());
    assert!(app.comparison_done && !app.error && app.differences.is_empty());
}

#[test]
fn left_edits_parse_before_refreshing_automatic_comparison() {
    let mut app = ready_app();
    let (sender, receiver) = mpsc::channel();
    app.jobs = sender.into();
    app.view = View::Compare;
    app.editor = "{\"value\":2}".into();
    app.search_source = Arc::new(app.editor.clone());
    app.dirty = true;
    app.revision += 1;
    app.compare_text = "{\"value\":3}".into();
    app.clear_comparison();
    app.tick_automatic_jobs(app.compare_due.unwrap());
    let parse = receiver.recv().unwrap();
    assert!(matches!(parse.job, Job::Process(..)));
    finish(&mut app, parse);
    assert!(!app.dirty && !app.comparison_done);
    app.tick_automatic_jobs(app.compare_due.unwrap());
    finish(&mut app, receiver.recv().unwrap());
    assert!(app.comparison_done);
    assert_eq!(&app.editor[app.compare_left_range.clone().unwrap()], "2");
    assert_eq!(
        &app.compare_text[app.compare_right_range.clone().unwrap()],
        "3"
    );
    app.submit(Job::FormatRight(Arc::new(app.compare_text.clone())));
    finish(&mut app, receiver.recv().unwrap());
    app.tick_automatic_jobs(app.compare_due.unwrap());
    finish(&mut app, receiver.recv().unwrap());
    assert_eq!(
        &app.compare_text[app.compare_right_range.clone().unwrap()],
        "3"
    );
}

#[test]
fn pasted_json_comparison_returns_source_ranges_on_both_sides() {
    let left: Arc<String> = Arc::new("{\"items\":[{\"name\":\"中文\"}],\"removed\":true}".into());
    let right: Arc<String> = Arc::new("{\"items\":[{\"name\":\"updated\"}],\"added\":null}".into());
    let Output::Comparison(diffs, _, a, b) =
        execute(Job::Compare(left.clone(), right.clone())).unwrap()
    else {
        panic!("comparison expected");
    };
    assert_eq!(diffs.len(), 3);
    assert_eq!(&left[a.unwrap()], "\"中文\"");
    assert_eq!(&right[b.unwrap()], "\"updated\"");
    let (a, b) = diff_ranges(&left, &right, "/removed");
    assert_eq!(&left[a.unwrap()], "true");
    assert!(b.is_none());
}

#[test]
fn pasted_json_comparison_reports_equality_and_which_input_is_invalid() {
    let Output::Comparison(diffs, _, _, _) = execute(Job::Compare(
        Arc::new("{\"a\":1,\"b\":2}".into()),
        Arc::new("{\"b\":2,\"a\":1}".into()),
    ))
    .unwrap() else {
        panic!("comparison expected");
    };
    assert!(diffs.is_empty());
    for (left, right, invalid_left, invalid_right) in [
        ("{}", "{bad}", false, true),
        ("{bad}", "{}", true, false),
        ("{bad}", "{bad}", true, true),
    ] {
        let Output::InvalidComparison(a, b) =
            execute(Job::Compare(Arc::new(left.into()), Arc::new(right.into()))).unwrap()
        else {
            panic!("syntax diagnostics expected")
        };
        assert_eq!(a.is_some(), invalid_left);
        assert_eq!(b.is_some(), invalid_right);
    }
}

#[test]
fn right_editor_formatting_does_not_replace_left_document() {
    let mut app = ready_app();
    let left = app.full_text.clone();
    let (sender, receiver) = mpsc::channel();
    app.jobs = sender.into();
    app.submit(Job::FormatRight(Arc::new("{\"right\":true}".into())));
    finish(&mut app, receiver.recv().unwrap());
    assert_eq!(*app.full_text, *left);
    assert!(app.compare_text.contains('\n'));
    assert!(app.compare_text.contains("right"));
}

#[test]
fn source_click_selects_graph_without_requesting_a_source_reveal_loop() {
    let mut app = ready_app();
    let (sender, receiver) = mpsc::channel();
    sender
        .send(Completed {
            target: None,
            revision: app.revision,
            reset_view: false,
            result: Ok(Output::SourcePointer(Some("/name".into()))),
            elapsed: Duration::ZERO,
            label: "Locate graph node",
            user_action: false,
        })
        .unwrap();
    app.completed = receiver;
    app.receive();
    assert!(matches!(app.view, View::Graph));
    assert_eq!(app.pointer, "/name");
    assert!(app.pending_reveal.is_none());
}

#[test]
fn invalid_json_tracks_its_source_and_recovers_after_validation() {
    let mut app = ready_app();
    let text = Arc::new("{\n  \"中文\": }".to_owned());
    let (sender, receiver) = mpsc::channel();
    app.jobs = sender.into();
    app.submit(Job::Process(text.clone(), Format::Keep));
    finish(&mut app, receiver.recv().unwrap());
    let expected = serde_json::from_str::<Value>(&text).unwrap_err();
    let diagnostic = app.syntax_error.as_ref().unwrap();
    assert_eq!(
        diagnostic.location,
        Some((expected.line(), expected.column()))
    );
    assert!(app.document.is_none());
    assert!(app.right_syntax_error.is_none());

    app.submit(Job::Process(
        Arc::new("{\"中文\": true}".into()),
        Format::Keep,
    ));
    finish(&mut app, receiver.recv().unwrap());
    assert!(app.syntax_error.is_none());
    assert!(app.document.is_some());

    app.compare_text = "{\n  \"right\": }".into();
    app.submit(Job::Compare(
        app.full_text.clone(),
        Arc::new(app.compare_text.clone()),
    ));
    finish(&mut app, receiver.recv().unwrap());
    assert!(app.syntax_error.is_none());
    assert_eq!(
        app.right_syntax_error.as_ref().unwrap().location.unwrap().0,
        2
    );
    assert!(!app.comparison_done);

    app.compare_text = "{\"right\": true}".into();
    app.submit(Job::Compare(
        app.full_text.clone(),
        Arc::new(app.compare_text.clone()),
    ));
    finish(&mut app, receiver.recv().unwrap());
    assert!(app.right_syntax_error.is_none());
    assert!(app.comparison_done);
}

#[test]
fn stale_invalid_result_does_not_mark_newer_json() {
    let mut app = ready_app();
    let (sender, receiver) = mpsc::channel();
    app.jobs = sender.into();
    app.submit(Job::Process(Arc::new("{".into()), Format::Keep));
    let work = receiver.recv().unwrap();
    app.revision += 1;
    finish(&mut app, work);
    assert!(app.syntax_error.is_none());
    assert!(app.document.is_some());
}

#[test]
fn invalid_large_import_previews_the_error_beyond_the_initial_window() {
    let mut app = ready_app();
    let input = Arc::new(format!(
        "{{\"large\":\"{}\",\n\"broken\":}}",
        "a".repeat(EDIT_LIMIT)
    ));
    let (sender, receiver) = mpsc::channel();
    app.jobs = sender.into();
    app.submit(Job::Import(input, "large.json".into()));
    finish(&mut app, receiver.recv().unwrap());
    assert!(!app.editable);
    assert!(app.preview_start > PREVIEW_LIMIT);
    let error = app.syntax_error.as_ref().unwrap();
    let offset = error.offset.unwrap();
    assert_eq!(error.location.unwrap().0, 2);
    assert!(offset >= app.preview_start && offset <= app.preview_start + app.editor.len());
    assert!(app.editor.contains("broken"));
}
