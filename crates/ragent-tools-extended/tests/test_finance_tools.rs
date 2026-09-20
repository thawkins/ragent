//! Integration tests for the yfinance tool registration and visibility.

use ragent_tools_extended::create_extended_registry;

#[test]
fn test_all_finance_tools_registered_with_network_fetch_permission() {
    let registry = create_extended_registry();
    let names = [
        "stock_quote",
        "stock_history",
        "stock_fundamentals",
        "currency_rate",
        "currency_history",
        "stock_search",
        "stock_options",
        "stock_recommendations",
    ];

    for name in names {
        let tool = registry
            .get(name)
            .unwrap_or_else(|| panic!("tool {} should be registered", name));
        assert_eq!(
            tool.permission_category(),
            "network:fetch",
            "{} should report network:fetch permission category",
            name
        );
    }
}

#[test]
fn test_stock_recommendations_tool_registered() {
    let registry = create_extended_registry();
    let tool = registry
        .get("stock_recommendations")
        .expect("stock_recommendations should be registered");
    assert_eq!(
        tool.permission_category(),
        "network:fetch",
        "stock_recommendations should report network:fetch permission category"
    );
    assert!(
        tool.description()
            .to_ascii_lowercase()
            .contains("recommendation"),
        "stock_recommendations description should mention recommendations"
    );
}
