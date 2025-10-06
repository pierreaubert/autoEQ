#!/bin/bash

# qa_check.sh - Validate AutoEQ QA output
#
# This script takes the QA output from autoeq --qa and reports OK if:
# - converge is true
# - spacing is ok
# - post value > pre-value + 0.4
#
# Usage:
#   ./autoeq --qa [args] | ./qa_check.sh
#   or
#   ./qa_check.sh "Converge: true | Spacing: ok | Pre: 1.5 | Post: 2.2"

set -euo pipefail

# Read input from stdin if no arguments, otherwise use first argument
if [ $# -eq 0 ]; then
    input=$(cat)
else
    input="$1"
fi

# Parse the QA output line
# Expected format: "Converge: true/false | Spacing: ok/ko | Pre: value | Post: value"
if ! echo "$input" | grep -q "Converge:.*|.*Spacing:.*|.*Pre:.*|.*Post:"; then
    echo "ERROR: Invalid QA output format"
    echo "Expected: 'Converge: true/false | Spacing: ok/ko | Pre: value | Post: value'"
    echo "Got: '$input'"
    exit 1
fi

# Extract values using parameter expansion and pattern matching
converge=$(echo "$input" | sed -n 's/.*Converge: \([^|]*\).*/\1/p' | tr -d ' ')
spacing=$(echo "$input" | sed -n 's/.*Spacing: \([^|]*\).*/\1/p' | tr -d ' ')
pre_value=$(echo "$input" | sed -n 's/.*Pre: \([^|]*\).*/\1/p' | tr -d ' ')
post_value=$(echo "$input" | sed -n 's/.*Post: \([^|]*\).*/\1/p' | tr -d ' ')

# Validate extracted values
if [ -z "$converge" ] || [ -z "$spacing" ] || [ -z "$pre_value" ] || [ -z "$post_value" ]; then
    echo "ERROR: Failed to parse QA output"
    echo "Extracted: converge='$converge', spacing='$spacing', pre='$pre_value', post='$post_value'"
    exit 1
fi

# Check if pre and post values are valid numbers
if ! echo "$pre_value" | grep -qE '^-?[0-9]+\.?[0-9]*$'; then
    echo "ERROR: Pre value '$pre_value' is not a valid number"
    exit 1
fi

if ! echo "$post_value" | grep -qE '^-?[0-9]+\.?[0-9]*$'; then
    echo "ERROR: Post value '$post_value' is not a valid number"
    exit 1
fi

# Initialize check results
converge_ok=false
spacing_ok=false
improvement_ok=false

# Check convergence (case insensitive)
if echo "$converge" | grep -iq "^true$"; then
    converge_ok=true
fi

# Check spacing (case insensitive)
if echo "$spacing" | grep -iq "^ok$"; then
    spacing_ok=true
fi

# Check improvement: post > pre + 0.4
# Note: This assumes "improvement" means post > pre + 0.4
# For headphone/CEA2034 preference scores: higher is better (improvement)
# For objective function values: lower is better, but this script checks for post > pre + 0.4
# If you need the opposite direction (lower is better), modify the condition below
improvement_threshold=$(echo "$pre_value + 0.4" | bc -l)
if echo "$post_value > $improvement_threshold" | bc -l | grep -q "1"; then
    improvement_ok=true
fi

# Output detailed status (for debugging)
echo "Parsed values:"
echo "  Converge: $converge ($( [ "$converge_ok" = true ] && echo "✓" || echo "✗" ))"
echo "  Spacing:  $spacing ($( [ "$spacing_ok" = true ] && echo "✓" || echo "✗" ))"
echo "  Pre:      $pre_value"
echo "  Post:     $post_value"
echo "  Improvement: $post_value > $pre_value + 0.4 = $improvement_threshold ($( [ "$improvement_ok" = true ] && echo "✓" || echo "✗" ))"
echo

# Final result
if [ "$converge_ok" = true ] && [ "$spacing_ok" = true ] && [ "$improvement_ok" = true ]; then
    echo "OK"
    exit 0
else
    echo "FAIL"
    exit 1
fi
