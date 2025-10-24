#!/bin/bash

# Helper script to run K6 load tests with proper configuration

set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$SCRIPT_DIR"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Print colored message
print_message() {
    local color=$1
    local message=$2
    echo -e "${color}${message}${NC}"
}

# Check if K6 is installed
if ! command -v k6 &> /dev/null; then
    print_message "$RED" "ERROR: K6 is not installed"
    print_message "$YELLOW" "Please install K6: https://k6.io/docs/getting-started/installation/"
    exit 1
fi

# Load environment variables if .env exists
if [ -f ".env" ]; then
    print_message "$GREEN" "✓ Loading configuration from .env file"
    export $(grep -v '^#' .env | xargs)
else
    print_message "$YELLOW" "⚠ No .env file found, using default/environment variables"
    print_message "$BLUE" "  You can copy .env.example to .env and configure it"
fi

# Default values
IMGFORGE_URL=${IMGFORGE_URL:-"http://localhost:3000"}
TEST_IMAGE_URL=${TEST_IMAGE_URL:-"https://picsum.photos/800/600"}

# Show usage
usage() {
    cat << EOF
Usage: $0 [OPTIONS] [TEST_TYPE]

Run K6 load tests against imgforge image processing service

TEST_TYPES:
    smoke       Quick validation test (default, 3 VUs for 30s)
    load        Standard load test (up to 20 VUs for ~4 min)
    stress      Stress test to find limits (up to 200 VUs for ~24 min)
    cache       Cache performance test (measures cache effectiveness)

OPTIONS:
    -h, --help              Show this help message
    -u, --url URL           imgforge server URL (default: $IMGFORGE_URL)
    -i, --image URL         Test image URL (default: $TEST_IMAGE_URL)
    -k, --key KEY           HMAC key for URL signing
    -s, --salt SALT         HMAC salt for URL signing
    --unsigned              Use unsigned URLs (dev only)
    --vus N                 Override number of virtual users
    --duration DURATION     Override test duration (e.g., 30s, 5m)
    --output FILE           Save results to JSON file
    --quiet                 Suppress K6 progress output

EXAMPLES:
    # Run smoke test with default settings
    $0 smoke

    # Run load test against remote server
    $0 -u https://imgforge.example.com load

    # Run stress test with custom image
    $0 -i https://example.com/large-image.jpg stress

    # Quick test with unsigned URLs (development)
    $0 --unsigned --vus 5 --duration 30s

    # Save results to file
    $0 --output results.json load

ENVIRONMENT VARIABLES:
    IMGFORGE_URL            Server URL
    IMGFORGE_KEY            HMAC key
    IMGFORGE_SALT           HMAC salt
    IMGFORGE_ALLOW_UNSIGNED Use unsigned URLs
    TEST_IMAGE_URL          Test image URL

EOF
    exit 0
}

# Parse arguments
TEST_TYPE="smoke"
K6_ARGS=""
CUSTOM_VUS=""
CUSTOM_DURATION=""
OUTPUT_FILE=""

while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            usage
            ;;
        -u|--url)
            IMGFORGE_URL="$2"
            shift 2
            ;;
        -i|--image)
            TEST_IMAGE_URL="$2"
            shift 2
            ;;
        -k|--key)
            IMGFORGE_KEY="$2"
            shift 2
            ;;
        -s|--salt)
            IMGFORGE_SALT="$2"
            shift 2
            ;;
        --unsigned)
            IMGFORGE_ALLOW_UNSIGNED="true"
            shift
            ;;
        --vus)
            CUSTOM_VUS="$2"
            shift 2
            ;;
        --duration)
            CUSTOM_DURATION="$2"
            shift 2
            ;;
        --output)
            OUTPUT_FILE="$2"
            shift 2
            ;;
        --quiet)
            K6_ARGS="$K6_ARGS --quiet"
            shift
            ;;
        smoke|load|stress|cache)
            TEST_TYPE="$1"
            shift
            ;;
        *)
            print_message "$RED" "Unknown option: $1"
            usage
            ;;
    esac
done

# Select test script based on type
case $TEST_TYPE in
    smoke)
        TEST_SCRIPT="smoke-test.js"
        print_message "$BLUE" "Running smoke test (quick validation)"
        ;;
    load)
        TEST_SCRIPT="processing-endpoint.js"
        print_message "$BLUE" "Running load test (standard performance test)"
        ;;
    stress)
        TEST_SCRIPT="stress-test.js"
        print_message "$YELLOW" "Running stress test (finding performance limits)"
        print_message "$YELLOW" "WARNING: This will generate significant load!"
        ;;
    cache)
        TEST_SCRIPT="cache-performance.js"
        print_message "$BLUE" "Running cache performance test"
        print_message "$BLUE" "Ensure caching is enabled in imgforge configuration"
        ;;
    *)
        print_message "$RED" "Unknown test type: $TEST_TYPE"
        usage
        ;;
esac

# Check if test script exists
if [ ! -f "$TEST_SCRIPT" ]; then
    print_message "$RED" "ERROR: Test script not found: $TEST_SCRIPT"
    exit 1
fi

# Check server availability
print_message "$BLUE" "Checking server availability at $IMGFORGE_URL..."
if ! curl -s -f "${IMGFORGE_URL}/status" > /dev/null; then
    print_message "$RED" "ERROR: Server is not responding at ${IMGFORGE_URL}/status"
    print_message "$YELLOW" "Please ensure imgforge server is running"
    exit 1
fi
print_message "$GREEN" "✓ Server is available"

# Add custom VUs/duration if specified
if [ -n "$CUSTOM_VUS" ]; then
    K6_ARGS="$K6_ARGS --vus $CUSTOM_VUS"
fi
if [ -n "$CUSTOM_DURATION" ]; then
    K6_ARGS="$K6_ARGS --duration $CUSTOM_DURATION"
fi

# Add output file if specified
if [ -n "$OUTPUT_FILE" ]; then
    K6_ARGS="$K6_ARGS --out json=$OUTPUT_FILE"
    print_message "$BLUE" "Results will be saved to: $OUTPUT_FILE"
fi

# Display configuration
print_message "$GREEN" "=== Test Configuration ==="
print_message "$GREEN" "Test Type: $TEST_TYPE"
print_message "$GREEN" "Server URL: $IMGFORGE_URL"
print_message "$GREEN" "Test Image: $TEST_IMAGE_URL"
if [ "${IMGFORGE_ALLOW_UNSIGNED}" = "true" ]; then
    print_message "$YELLOW" "Using unsigned URLs (development mode)"
else
    if [ -n "$IMGFORGE_KEY" ] && [ -n "$IMGFORGE_SALT" ]; then
        print_message "$GREEN" "Using HMAC-signed URLs"
    else
        print_message "$YELLOW" "WARNING: No HMAC key/salt provided"
        print_message "$YELLOW" "Set IMGFORGE_KEY and IMGFORGE_SALT or use --unsigned"
    fi
fi
print_message "$GREEN" "=========================="

# Export environment variables for K6
export IMGFORGE_URL
export IMGFORGE_KEY
export IMGFORGE_SALT
export IMGFORGE_ALLOW_UNSIGNED
export TEST_IMAGE_URL

# Run the test
print_message "$BLUE" "\nStarting K6 test...\n"
k6 run $K6_ARGS "$TEST_SCRIPT"

# Test completed
print_message "$GREEN" "\n✓ Test completed successfully"

if [ -n "$OUTPUT_FILE" ]; then
    print_message "$GREEN" "Results saved to: $OUTPUT_FILE"
fi
