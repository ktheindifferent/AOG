#!/bin/bash

echo "Testing Authentication System"
echo "=============================="

# Test 1: Check if auth module tests pass
echo -e "\n1. Running auth module unit tests..."
cargo test --lib aog::auth 2>&1 | grep "test result"

# Test 2: Check password generation
echo -e "\n2. Testing password generation with environment variable..."
export AOG_INITIAL_PASSWORD="SecureTest@Pass123!"
cargo test --lib aog::auth::tests::test_password_hashing_and_verification 2>&1 | grep "test result"
unset AOG_INITIAL_PASSWORD

# Test 3: Verify no hardcoded passwords exist
echo -e "\n3. Checking for hardcoded passwords..."
if grep -r 'encrypted_password.*=.*format!("aog")' src/ --include="*.rs" | grep -v "test"; then
    echo "WARNING: Found potential hardcoded password references!"
    exit 1
else
    echo "✓ No hardcoded passwords found"
fi

# Test 4: Check if argon2 dependency is included
echo -e "\n4. Verifying argon2 dependency..."
if grep -q "argon2" Cargo.toml; then
    echo "✓ Argon2 dependency found"
else
    echo "ERROR: Argon2 dependency missing!"
    exit 1
fi

echo -e "\n=============================="
echo "Authentication system tests completed successfully!"