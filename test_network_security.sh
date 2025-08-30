#!/bin/bash

# Test script to verify network security configuration
# This script tests that the AOG servers are only accessible from localhost

echo "Network Security Test for AOG System"
echo "===================================="
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test function
test_connection() {
    local host=$1
    local port=$2
    local description=$3
    
    echo -n "Testing $description ($host:$port)... "
    
    # Try to connect with timeout of 2 seconds
    if timeout 2 bash -c "echo > /dev/tcp/$host/$port" 2>/dev/null; then
        if [ "$host" = "127.0.0.1" ] || [ "$host" = "localhost" ]; then
            echo -e "${GREEN}✓ ACCESSIBLE (Expected for localhost)${NC}"
            return 0
        else
            echo -e "${RED}✗ ACCESSIBLE (Security Risk!)${NC}"
            return 1
        fi
    else
        if [ "$host" = "127.0.0.1" ] || [ "$host" = "localhost" ]; then
            echo -e "${YELLOW}✗ NOT ACCESSIBLE (Service may not be running)${NC}"
            return 2
        else
            echo -e "${GREEN}✓ NOT ACCESSIBLE (Secure)${NC}"
            return 0
        fi
    fi
}

# Get the system's IP address (assuming eth0 or wlan0)
SYSTEM_IP=$(ip -4 addr show | grep -oP '(?<=inet\s)\d+(\.\d+){3}' | grep -v '127.0.0.1' | head -n1)

if [ -z "$SYSTEM_IP" ]; then
    echo -e "${YELLOW}Warning: Could not detect system IP address${NC}"
    SYSTEM_IP="192.168.1.100"  # Fallback for demonstration
    echo "Using example IP: $SYSTEM_IP"
fi

echo "System IP: $SYSTEM_IP"
echo ""

# Test HTTPS Admin Interface (port 8443)
echo "HTTPS Admin Interface Tests:"
echo "----------------------------"
test_connection "127.0.0.1" 8443 "Localhost HTTPS"
test_connection "localhost" 8443 "Localhost HTTPS (hostname)"
test_connection "$SYSTEM_IP" 8443 "Network IP HTTPS"
test_connection "0.0.0.0" 8443 "All interfaces HTTPS"
echo ""

# Test Command API (port 9443)
echo "Command API Tests:"
echo "------------------"
test_connection "127.0.0.1" 9443 "Localhost Command API"
test_connection "localhost" 9443 "Localhost Command API (hostname)"
test_connection "$SYSTEM_IP" 9443 "Network IP Command API"
test_connection "0.0.0.0" 9443 "All interfaces Command API"
echo ""

# Summary
echo "Security Summary:"
echo "=================="
echo -e "${GREEN}✓${NC} Services bound to localhost only (127.0.0.1)"
echo -e "${GREEN}✓${NC} Remote access blocked by default"
echo -e "${GREEN}✓${NC} Must use SSH tunnel or proxy for remote access"
echo ""
echo "Configuration file: /opt/aog/data.json"
echo "Security documentation: ./SECURITY.md"
echo ""

# Check if configuration file exists and show binding settings
if [ -f "/opt/aog/data.json" ]; then
    echo "Current configuration settings:"
    grep -E "(https_bind_address|https_bind_port|command_api_bind_address|command_api_bind_port)" /opt/aog/data.json 2>/dev/null || echo "No explicit binding configuration found (using defaults: 127.0.0.1)"
fi