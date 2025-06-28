#!/bin/bash

# TILT Symbolic Test Runner
# Runs all .tilt files in symbolic_tests and compares output with expected .txt files

echo "🚀 TILT Symbolic Test Runner"
echo "============================="

test_dir="symbolic_tests"
compiler="./target/debug/tiltc.exe"
passed=0
failed=0
total=0

# Build the compiler first (skip if already built)
if [ ! -f "$compiler" ]; then
    echo "Building compiler..."
    if command -v cargo &> /dev/null; then
        cargo build --quiet
        if [ $? -ne 0 ]; then
            echo "❌ Failed to build compiler"
            exit 1
        fi
    else
        echo "⚠️  Cargo not found, assuming pre-built binary exists"
        if [ ! -f "$compiler" ]; then
            echo "❌ Neither cargo nor pre-built binary found"
            exit 1
        fi
    fi
else
    echo "Using pre-built compiler binary"
fi

echo ""

# Run tests
for tilt_file in "$test_dir"/*.tilt; do
    if [ -f "$tilt_file" ]; then
        base_name=$(basename "$tilt_file" .tilt)
        txt_file="$test_dir/$base_name.txt"
        
        echo -n "Testing $base_name... "
        total=$((total + 1))
        
        if [ ! -f "$txt_file" ]; then
            echo "❌ Missing expected output file: $txt_file"
            failed=$((failed + 1))
            continue
        fi
        
        # Run the TILT compiler and capture output for both VM and JIT
        vm_output=$($compiler "$tilt_file" --vm 2>&1)
        vm_exit_code=$?
        
        jit_output=$($compiler "$tilt_file" --jit 2>&1)
        jit_exit_code=$?
        
        if [ $vm_exit_code -ne 0 ]; then
            echo "❌ VM execution failed"
            echo "   VM Output: $vm_output"
            failed=$((failed + 1))
            continue
        elif [ $jit_exit_code -ne 0 ]; then
            echo "❌ JIT execution failed"
            echo "   JIT Output: $jit_output"
            failed=$((failed + 1))
            continue
        fi
        
        # Extract the results from both outputs
        vm_result=$(echo "$vm_output" | grep -o "Final result: I32([0-9-]*)" | sed 's/Final result: I32(\([0-9-]*\))/\1/')
        jit_result=$(echo "$jit_output" | grep -o "Final result: I32([0-9-]*)" | sed 's/Final result: I32(\([0-9-]*\))/\1/')
        jit_result=$(echo "$jit_output" | grep -o "Final result: I32([0-9-]*)" | grep -o "([0-9-]*)" | tr -d "()")
        
        if [ -z "$vm_result" ]; then
            echo "❌ Could not extract VM result from output"
            echo "   VM Output: $vm_output"
            failed=$((failed + 1))
            continue
        elif [ -z "$jit_result" ]; then
            echo "❌ Could not extract JIT result from output"
            echo "   JIT Output: $jit_output"
            failed=$((failed + 1))
            continue
        elif [ "$vm_result" != "$jit_result" ]; then
            echo "❌ VM/JIT mismatch (VM: $vm_result, JIT: $jit_result)"
            failed=$((failed + 1))
            continue
        fi
        
        # Read expected result from txt file
        expected=$(grep "Expected Output:" "$txt_file" | cut -d' ' -f3 | tr -d '\r\n')
        
        if [ -z "$expected" ]; then
            echo "❌ Could not parse expected output from $txt_file"
            failed=$((failed + 1))
            continue
        fi
        
        # Compare results
        if [ "$vm_result" = "$expected" ]; then
            echo "✅ PASS (VM/JIT: $vm_result)"
            passed=$((passed + 1))
        else
            echo "❌ FAIL (expected $expected, got $vm_result)"
            failed=$((failed + 1))
        fi
    fi
done

echo ""
echo "============================="
echo "Test Results:"
echo "  Total:  $total"
echo "  Passed: $passed"
echo "  Failed: $failed"

if [ $failed -eq 0 ]; then
    echo "🎉 All tests passed!"
    exit 0
else
    echo "💥 Some tests failed"
    exit 1
fi
