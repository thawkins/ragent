# Parallel Tool Execution with Concurrency Limit

## Overview

The system now enforces a maximum concurrency limit of 5 parallel tool executions to prevent overwhelming the system with too many concurrent operations.

## Implementation

### Location
- **File**: `crates/ragent-core/src/session/processor.rs`
- **Constant**: `MAX_PARALLEL_TOOLS = 5`

### Behavior

When the LLM requests multiple tool calls in a single turn:

1. **Chunked Execution**: Tools are executed in chunks of up to 5 concurrent operations
2. **Sequential Chunks**: Each chunk must complete before the next chunk begins
3. **Parallel Within Chunk**: Within each chunk, tools execute concurrently using `tokio::spawn`
4. **Order Preservation**: Results are collected and processed in the original order

### Example

If the LLM requests 12 tool calls:
- **Chunk 1**: Tools 1-5 execute in parallel
- **Chunk 2**: Tools 6-10 execute in parallel (after chunk 1 completes)
- **Chunk 3**: Tools 11-12 execute in parallel (after chunk 2 completes)

## Agent Switch Handling

If a tool in a chunk requests an agent switch (via metadata):
- The current chunk completes execution
- Subsequent chunks are skipped
- Processing returns control to handle the agent switch

## Benefits

1. **Resource Control**: Prevents system overload from excessive concurrent operations
2. **Predictable Performance**: Bounded resource usage regardless of LLM request size
3. **Fault Isolation**: Panics in spawned tasks are caught and logged without crashing the session
4. **Throughput Improvement**: 5 concurrent tools execute faster than sequential execution

## Testing

Tests are located in `crates/ragent-core/tests/test_parallel_tool_execution.rs`:

- `test_max_5_parallel_tools`: Validates setup for concurrency testing
- `test_sequential_execution_with_agent_switch`: Documents agent switch behavior
- `test_max_parallel_tools_constant`: Verifies the constant value

Run tests:
```bash
cargo test --test test_parallel_tool_execution
```

## Configuration

The limit is currently hardcoded as a constant. To change it:

1. Edit `MAX_PARALLEL_TOOLS` in `crates/ragent-core/src/session/processor.rs`
2. Recommended range: 3-10 (balance between parallelism and resource usage)
3. Consider system resources (CPU cores, memory, open file limits)

## Future Enhancements

Potential improvements for future consideration:

1. **Configurable Limit**: Make the limit configurable via `ragent.json` or environment variable
2. **Adaptive Concurrency**: Dynamically adjust based on system load
3. **Per-Tool Categories**: Different limits for different tool types (e.g., higher for reads, lower for writes)
4. **Metrics**: Track actual concurrency levels and wait times
