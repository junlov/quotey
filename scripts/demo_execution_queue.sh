#!/bin/bash
#
# Execution Queue Demo Script
# Demonstrates the resilient execution queue functionality
#

set -e

echo "=========================================="
echo "Quotey Execution Queue Demo"
echo "=========================================="
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check prerequisites
check_prerequisites() {
    echo -e "${YELLOW}Checking prerequisites...${NC}"
    
    if [ ! -f "quotey.db" ]; then
        echo -e "${RED}Error: quotey.db not found. Run 'cargo run -- migrate' first.${NC}"
        exit 1
    fi
    
    if ! command -v sqlite3 &> /dev/null; then
        echo -e "${RED}Error: sqlite3 not found. Please install sqlite3.${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}✓ Prerequisites met${NC}"
    echo ""
}

# Show current queue status
show_queue_status() {
    echo -e "${YELLOW}Current Queue Status:${NC}"
    echo "------------------------------------------"
    
    sqlite3 quotey.db <<EOF
.headers on
.mode column
SELECT 
    state,
    COUNT(*) as count,
    MAX(created_at) as latest_task
FROM execution_queue_task
GROUP BY state
ORDER BY count DESC;
EOF
    echo ""
}

# Create demo tasks
create_demo_tasks() {
    echo -e "${YELLOW}Creating demo tasks...${NC}"
    
    # Task 1: Will succeed
    sqlite3 quotey.db <<EOF
INSERT INTO execution_queue_task (
    id, quote_id, operation_kind, payload_json, idempotency_key, state,
    retry_count, max_retries, available_at, state_version, created_at, updated_at
) VALUES (
    'demo-task-001', 'Q-DEMO-001', 'send_slack_message',
    '{"channel": "#demo", "text": "Hello from demo!"}',
    'demo-op-001', 'queued', 0, 3, datetime('now'), 1, datetime('now'), datetime('now')
);

INSERT INTO execution_idempotency_ledger (
    operation_key, quote_id, operation_kind, payload_hash, state,
    attempt_count, first_seen_at, last_seen_at, correlation_id,
    created_by_component, updated_by_component
) VALUES (
    'demo-op-001', 'Q-DEMO-001', 'send_slack_message',
    'abc123hash', 'reserved', 1, datetime('now'), datetime('now'), 'demo-corr-001',
    'demo', 'demo'
);
EOF

    # Task 2: Will fail then retry
    sqlite3 quotey.db <<EOF
INSERT INTO execution_queue_task (
    id, quote_id, operation_kind, payload_json, idempotency_key, state,
    retry_count, max_retries, available_at, state_version, created_at, updated_at
) VALUES (
    'demo-task-002', 'Q-DEMO-002', 'generate_pdf',
    '{"template": "standard", "quote_id": "Q-DEMO-002"}',
    'demo-op-002', 'queued', 0, 3, datetime('now'), 1, datetime('now'), datetime('now')
);

INSERT INTO execution_idempotency_ledger (
    operation_key, quote_id, operation_kind, payload_hash, state,
    attempt_count, first_seen_at, last_seen_at, correlation_id,
    created_by_component, updated_by_component
) VALUES (
    'demo-op-002', 'Q-DEMO-002', 'generate_pdf',
    'def456hash', 'reserved', 1, datetime('now'), datetime('now'), 'demo-corr-002',
    'demo', 'demo'
);
EOF

    # Task 3: Already running (simulates in-progress)
    sqlite3 quotey.db <<EOF
INSERT INTO execution_queue_task (
    id, quote_id, operation_kind, payload_json, idempotency_key, state,
    retry_count, max_retries, available_at, claimed_by, claimed_at,
    state_version, created_at, updated_at
) VALUES (
    'demo-task-003', 'Q-DEMO-003', 'crm_sync',
    '{"action": "create_deal", "account_id": "ACC-123"}',
    'demo-op-003', 'running', 0, 3, datetime('now'),
    'worker-demo-001', datetime('now'), 1, datetime('now'), datetime('now')
);

INSERT INTO execution_idempotency_ledger (
    operation_key, quote_id, operation_kind, payload_hash, state,
    attempt_count, first_seen_at, last_seen_at, correlation_id,
    created_by_component, updated_by_component
) VALUES (
    'demo-op-003', 'Q-DEMO-003', 'crm_sync',
    'ghi789hash', 'running', 1, datetime('now'), datetime('now'), 'demo-corr-003',
    'demo', 'demo'
);
EOF

    echo -e "${GREEN}✓ Created 3 demo tasks${NC}"
    echo ""
}

# Show task details
show_task_details() {
    local task_id=$1
    echo -e "${YELLOW}Task Details: $task_id${NC}"
    echo "------------------------------------------"
    
    sqlite3 quotey.db <<EOF
.headers on
.mode column
SELECT 
    t.id,
    t.quote_id,
    t.operation_kind,
    t.state,
    t.retry_count || '/' || t.max_retries as retries,
    t.claimed_by,
    t.claimed_at,
    i.state as idempotency_state,
    i.correlation_id
FROM execution_queue_task t
LEFT JOIN execution_idempotency_ledger i ON t.idempotency_key = i.operation_key
WHERE t.id = '$task_id';
EOF
    echo ""
}

# Show transition audit log
show_audit_log() {
    local task_id=$1
    echo -e "${YELLOW}Audit Log: $task_id${NC}"
    echo "------------------------------------------"
    
    sqlite3 quotey.db <<EOF
.headers on
.mode column
SELECT 
    occurred_at,
    from_state,
    to_state,
    transition_reason,
    actor_id,
    correlation_id
FROM execution_queue_transition_audit
WHERE task_id = '$task_id'
ORDER BY occurred_at;
EOF
    echo ""
}

# Simulate claiming a task
simulate_claim() {
    local task_id=$1
    local worker_id=$2
    
    echo -e "${YELLOW}Simulating claim: $task_id by $worker_id${NC}"
    
    sqlite3 quotey.db <<EOF
UPDATE execution_queue_task
SET state = 'running',
    claimed_by = '$worker_id',
    claimed_at = datetime('now'),
    state_version = state_version + 1,
    updated_at = datetime('now')
WHERE id = '$task_id';

INSERT INTO execution_queue_transition_audit (
    id, task_id, quote_id, from_state, to_state,
    transition_reason, actor_type, actor_id, correlation_id,
    state_version, occurred_at
) SELECT 
    lower(hex(randomblob(16))),
    '$task_id',
    quote_id,
    'queued',
    'running',
    'task_claimed',
    'worker',
    '$worker_id',
    'demo-corr',
    state_version,
    datetime('now')
FROM execution_queue_task
WHERE id = '$task_id';
EOF

    echo -e "${GREEN}✓ Task claimed${NC}"
    echo ""
}

# Simulate completing a task
simulate_complete() {
    local task_id=$1
    local result=$2
    
    echo -e "${YELLOW}Simulating completion: $task_id${NC}"
    
    sqlite3 quotey.db <<EOF
UPDATE execution_queue_task
SET state = 'completed',
    result_fingerprint = '$result',
    claimed_by = NULL,
    claimed_at = NULL,
    state_version = state_version + 1,
    updated_at = datetime('now')
WHERE id = '$task_id';

UPDATE execution_idempotency_ledger
SET state = 'completed',
    result_snapshot_json = '$result',
    last_seen_at = datetime('now')
WHERE operation_key = (SELECT idempotency_key FROM execution_queue_task WHERE id = '$task_id');

INSERT INTO execution_queue_transition_audit (
    id, task_id, quote_id, from_state, to_state,
    transition_reason, actor_type, actor_id, correlation_id,
    state_version, occurred_at
) SELECT 
    lower(hex(randomblob(16))),
    '$task_id',
    quote_id,
    'running',
    'completed',
    'task_completed',
    'worker',
    'system',
    'demo-corr',
    state_version,
    datetime('now')
FROM execution_queue_task
WHERE id = '$task_id';
EOF

    echo -e "${GREEN}✓ Task completed${NC}"
    echo ""
}

# Simulate a failure with retry
simulate_fail_retryable() {
    local task_id=$1
    local error_msg=$2
    
    echo -e "${YELLOW}Simulating retryable failure: $task_id${NC}"
    
    sqlite3 quotey.db <<EOF
UPDATE execution_queue_task
SET state = 'retryable_failed',
    retry_count = retry_count + 1,
    last_error = '$error_msg',
    available_at = datetime('now', '+5 seconds'),
    claimed_by = NULL,
    claimed_at = NULL,
    state_version = state_version + 1,
    updated_at = datetime('now')
WHERE id = '$task_id';

UPDATE execution_idempotency_ledger
SET state = 'failed_retryable',
    error_snapshot_json = '$error_msg',
    last_seen_at = datetime('now')
WHERE operation_key = (SELECT idempotency_key FROM execution_queue_task WHERE id = '$task_id');

INSERT INTO execution_queue_transition_audit (
    id, task_id, quote_id, from_state, to_state,
    transition_reason, error_class, actor_type, actor_id, correlation_id,
    state_version, occurred_at
) SELECT 
    lower(hex(randomblob(16))),
    '$task_id',
    quote_id,
    'running',
    'retryable_failed',
    'task_failed_retryable',
    'NetworkError',
    'worker',
    'system',
    'demo-corr',
    state_version,
    datetime('now')
FROM execution_queue_task
WHERE id = '$task_id';
EOF

    echo -e "${GREEN}✓ Task marked for retry${NC}"
    echo ""
}

# Main demo flow
run_demo() {
    echo "=========================================="
    echo "Demo: Resilient Execution Queue"
    echo "=========================================="
    echo ""
    
    check_prerequisites
    
    # Show initial state
    show_queue_status
    
    # Create demo tasks
    create_demo_tasks
    show_queue_status
    
    # Demo 1: Claim and complete a task
    echo -e "${GREEN}=== Demo 1: Claim and Complete Task ===${NC}"
    show_task_details "demo-task-001"
    simulate_claim "demo-task-001" "worker-demo-001"
    simulate_complete "demo-task-001" "message-sent:ts=123456"
    show_task_details "demo-task-001"
    show_audit_log "demo-task-001"
    
    # Demo 2: Retryable failure
    echo -e "${GREEN}=== Demo 2: Retryable Failure ===${NC}"
    show_task_details "demo-task-002"
    simulate_claim "demo-task-002" "worker-demo-002"
    simulate_fail_retryable "demo-task-002" "Connection timeout after 30s"
    show_task_details "demo-task-002"
    show_audit_log "demo-task-002"
    
    # Demo 3: Show stuck task detection
    echo -e "${GREEN}=== Demo 3: Stuck Task Detection ===${NC}"
    echo "Checking for tasks claimed > 5 minutes ago:"
    sqlite3 quotey.db <<EOF
SELECT 
    id,
    quote_id,
    operation_kind,
    claimed_by,
    claimed_at,
    (julianday('now') - julianday(claimed_at)) * 24 * 60 as minutes_claimed
FROM execution_queue_task
WHERE state = 'running'
  AND claimed_at < datetime('now', '-5 minutes');
EOF
    echo ""
    
    # Final status
    show_queue_status
    
    # Summary
    echo "=========================================="
    echo -e "${GREEN}Demo Complete!${NC}"
    echo "=========================================="
    echo ""
    echo "Key Takeaways:"
    echo "  • Tasks move through states: queued → running → completed/failed"
    echo "  • Retryable failures are automatically retried with backoff"
    echo "  • All transitions are logged in the audit table"
    echo "  • Idempotency prevents duplicate execution"
    echo "  • Stale claims can be detected and recovered"
    echo ""
    echo "Next steps:"
    echo "  • Run 'cargo test execution_engine' to see unit tests"
    echo "  • Check runbook: .planning/W1_REL_EXECUTION_QUEUE_RUNBOOK.md"
    echo ""
}

# Cleanup demo data
cleanup() {
    echo -e "${YELLOW}Cleaning up demo data...${NC}"
    
    sqlite3 quotey.db <<EOF
DELETE FROM execution_queue_transition_audit WHERE task_id LIKE 'demo-task-%';
DELETE FROM execution_queue_task WHERE id LIKE 'demo-task-%';
DELETE FROM execution_idempotency_ledger WHERE operation_key LIKE 'demo-op-%';
EOF

    echo -e "${GREEN}✓ Demo data cleaned${NC}"
}

# Help
show_help() {
    echo "Usage: $0 [command]"
    echo ""
    echo "Commands:"
    echo "  demo      Run the full demo (default)"
    echo "  status    Show current queue status"
    echo "  cleanup   Remove demo data"
    echo "  help      Show this help"
    echo ""
}

# Main
main() {
    case "${1:-demo}" in
        demo)
            run_demo
            ;;
        status)
            check_prerequisites
            show_queue_status
            ;;
        cleanup)
            cleanup
            ;;
        help|--help|-h)
            show_help
            ;;
        *)
            echo "Unknown command: $1"
            show_help
            exit 1
            ;;
    esac
}

main "$@"
