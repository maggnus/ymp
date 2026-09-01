use l4_config_fusion::release_plan;

#[test]
fn visible_happy_path_is_sorted() {
    assert_eq!(
        release_plan(&["api=7", "worker=3"], &["jobs->worker", "web->api"]).unwrap(),
        ["jobs@worker#3", "web@api#7"]
    );
}
