use aistar::config::Config;

#[test]
fn test_config_validation_rejects_invalid_models() {
    let config = Config {
        api_key: "test-key".to_string(),
        model: "local/mock-model".to_string(),
        working_dir: std::env::current_dir().expect("cwd"),
    };

    assert!(config.validate().is_err());
}
