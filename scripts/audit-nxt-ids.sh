#!/usr/bin/env bash
set -euo pipefail

# Deterministically audit expected quotey-nxt-* IDs after JSONL normalization.
# Usage:
#   scripts/audit-nxt-ids.sh
#   scripts/audit-nxt-ids.sh --json

if ! command -v jq >/dev/null 2>&1; then
    echo "error: jq is required but not found in PATH" >&2
    exit 1
fi

json_mode=false
if [[ "${1:-}" == "--json" ]]; then
    json_mode=true
fi

expected_ids_json='[
  "quotey-nxt-7.1",
  "quotey-nxt-9.1",
  "quotey-nxt-9.2",
  "quotey-nxt-10.1",
  "quotey-nxt-10.2",
  "quotey-nxt-11.1",
  "quotey-nxt-12.1",
  "quotey-nxt-mcp-esc"
]'

issues_jsonl_path=".beads/issues.jsonl"
if [[ ! -f "${issues_jsonl_path}" ]]; then
    echo "error: expected JSONL file not found at ${issues_jsonl_path}" >&2
    exit 1
fi

if ! issues_json="$(jq -cs 'map(select(type == "object"))' "${issues_jsonl_path}")"; then
    echo "error: failed to parse ${issues_jsonl_path} as JSONL" >&2
    exit 1
fi

report_json="$(
    jq \
        --argjson expected_ids "${expected_ids_json}" \
        '
        [ $expected_ids[] as $id |
            {
                expected_id: $id,
                canonical_ids: [ .[] | select(.id == $id) | .id ],
                mapped_ids: [ .[] | select((.external_ref // "") == $id) | .id ]
            }
            | .canonical_present = ((.canonical_ids | length) > 0)
            | .mapped_present = ((.mapped_ids | length) > 0)
            | .recommended_action = (
                if .canonical_present then
                    "keep_canonical"
                elif .mapped_present then
                    "map_to_existing"
                else
                    "reintroduce_canonical"
                end
            )
        ]
        ' <<<"${issues_json}"
)"

if [[ "${json_mode}" == "true" ]]; then
    printf '%s\n' "${report_json}"
    exit 0
fi

echo "expected_id|canonical_present|mapped_present|canonical_ids|mapped_ids|recommended_action"
jq -r '
    .[]
    | [
        .expected_id,
        .canonical_present,
        .mapped_present,
        (.canonical_ids | join(",")),
        (.mapped_ids | join(",")),
        .recommended_action
      ]
    | join("|")
' <<<"${report_json}"

echo
echo "Summary:"
jq -r '
    {
      keep_canonical: ([ .[] | select(.recommended_action == "keep_canonical") ] | length),
      map_to_existing: ([ .[] | select(.recommended_action == "map_to_existing") ] | length),
      reintroduce_canonical: ([ .[] | select(.recommended_action == "reintroduce_canonical") ] | length)
    }
    | to_entries[]
    | "  \(.key): \(.value)"
' <<<"${report_json}"
