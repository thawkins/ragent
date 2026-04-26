#[cfg(test)]
mod tests {
    use ragent_config::InternalLlmConfig;
    use ragent_llm::embedded::{EmbeddedRuntime, known_model_manifest};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_really_load_smollm() {
        let mut config = InternalLlmConfig::default();
        config.enabled = true;
        config.model_id = "smollm2-360m-instruct-q4".to_string();
        config.threads = 4;
        config.context_window = 4096;
        config.max_output_tokens = 128;
        config.timeout_ms = 30000;

        let runtime = EmbeddedRuntime::from_config(config.clone())
            .expect("create runtime")
            .expect("runtime enabled");

        println!("Model dir: {:?}", runtime.model_dir());
        println!(
            "Files: {:?}",
            std::fs::read_dir(runtime.model_dir())
                .unwrap()
                .map(|e| e.unwrap().path())
                .collect::<Vec<_>>()
        );

        match runtime.try_init_from_cache() {
            Ok(()) => println!("MODEL LOADED OK"),
            Err(e) => println!("MODEL LOAD FAILED: {:#}", e),
        }
    }
}
