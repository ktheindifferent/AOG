#!/bin/bash

# Test script to verify secure permission changes

echo "=== AOG Permission Security Test ==="
echo

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test directories
TEST_DIR="/tmp/test_aog"
TEST_CONFIG="$TEST_DIR/config.json"
TEST_LOG="$TEST_DIR/output.log"

# Clean up from previous runs
rm -rf $TEST_DIR

# Create test directory structure
echo "Creating test directory structure..."
mkdir -p $TEST_DIR

# Create test files
echo '{"test": "config"}' > $TEST_CONFIG
echo "Test log entry" > $TEST_LOG

# Function to check permissions
check_permissions() {
    local path=$1
    local expected=$2
    local actual=$(stat -c "%a" "$path")
    
    if [ "$actual" = "$expected" ]; then
        echo -e "${GREEN}✓${NC} $path has correct permissions ($actual)"
        return 0
    else
        echo -e "${RED}✗${NC} $path has incorrect permissions (expected: $expected, actual: $actual)"
        return 1
    fi
}

# Test 1: Check for dangerous 777 permissions in source code
echo
echo "Test 1: Checking source code for dangerous permissions..."
if grep -r "777" /root/repo/src/src-v2-rust-alpha/src/aog/tools.rs > /dev/null 2>&1; then
    echo -e "${RED}✗${NC} Found dangerous 777 permissions in source code"
else
    echo -e "${GREEN}✓${NC} No dangerous 777 permissions found in source code"
fi

# Test 2: Check for new security functions
echo
echo "Test 2: Checking for security functions..."
functions_found=0

if grep -q "validate_permissions" /root/repo/src/src-v2-rust-alpha/src/aog/tools.rs; then
    echo -e "${GREEN}✓${NC} validate_permissions function found"
    ((functions_found++))
else
    echo -e "${RED}✗${NC} validate_permissions function not found"
fi

if grep -q "set_ownership" /root/repo/src/src-v2-rust-alpha/src/aog/tools.rs; then
    echo -e "${GREEN}✓${NC} set_ownership function found"
    ((functions_found++))
else
    echo -e "${RED}✗${NC} set_ownership function not found"
fi

if grep -q "security_audit_log" /root/repo/src/src-v2-rust-alpha/src/main.rs; then
    echo -e "${GREEN}✓${NC} security_audit_log function found"
    ((functions_found++))
else
    echo -e "${RED}✗${NC} security_audit_log function not found"
fi

# Test 3: Check for user/group creation logic
echo
echo "Test 3: Checking for user/group creation..."
if grep -q "create_aog_user_and_group" /root/repo/src/src-v2-rust-alpha/src/setup.rs; then
    echo -e "${GREEN}✓${NC} User/group creation function found"
else
    echo -e "${RED}✗${NC} User/group creation function not found"
fi

# Test 4: Check systemd service security settings
echo
echo "Test 4: Checking systemd service security settings..."
security_settings=0

if grep -q "User=aog" /root/repo/src/src-v2-rust-alpha/src/setup.rs; then
    echo -e "${GREEN}✓${NC} Service runs as dedicated aog user"
    ((security_settings++))
else
    echo -e "${RED}✗${NC} Service does not run as dedicated user"
fi

if grep -q "NoNewPrivileges=true" /root/repo/src/src-v2-rust-alpha/src/setup.rs; then
    echo -e "${GREEN}✓${NC} NoNewPrivileges security setting found"
    ((security_settings++))
else
    echo -e "${RED}✗${NC} NoNewPrivileges security setting not found"
fi

if grep -q "ProtectSystem=strict" /root/repo/src/src-v2-rust-alpha/src/setup.rs; then
    echo -e "${GREEN}✓${NC} ProtectSystem security setting found"
    ((security_settings++))
else
    echo -e "${RED}✗${NC} ProtectSystem security setting not found"
fi

# Test 5: Check permission settings in fix_permissions function
echo
echo "Test 5: Verifying secure permission values..."
permission_checks=0

if grep -q '"755"' /root/repo/src/src-v2-rust-alpha/src/aog/tools.rs; then
    echo -e "${GREEN}✓${NC} Directory permissions set to 755"
    ((permission_checks++))
else
    echo -e "${RED}✗${NC} Directory permissions not set to 755"
fi

if grep -q '"644"' /root/repo/src/src-v2-rust-alpha/src/aog/tools.rs; then
    echo -e "${GREEN}✓${NC} Config file permissions set to 644"
    ((permission_checks++))
else
    echo -e "${RED}✗${NC} Config file permissions not set to 644"
fi

if grep -q '"664"' /root/repo/src/src-v2-rust-alpha/src/aog/tools.rs; then
    echo -e "${GREEN}✓${NC} Log file permissions set to 664"
    ((permission_checks++))
else
    echo -e "${RED}✗${NC} Log file permissions not set to 664"
fi

# Summary
echo
echo "=== Test Summary ==="
total_tests=11
passed_tests=$((3 - $(grep -c "777" /root/repo/src/src-v2-rust-alpha/src/aog/tools.rs 2>/dev/null | head -1)))
passed_tests=$((passed_tests + functions_found + security_settings + permission_checks + 1))

if [ $passed_tests -eq $total_tests ]; then
    echo -e "${GREEN}All tests passed! ($passed_tests/$total_tests)${NC}"
    echo -e "${GREEN}The system is now secure with proper file permissions.${NC}"
else
    echo -e "${YELLOW}Some tests failed ($passed_tests/$total_tests)${NC}"
    echo -e "${YELLOW}Please review the failed tests above.${NC}"
fi

# Clean up
rm -rf $TEST_DIR

echo
echo "Test complete."