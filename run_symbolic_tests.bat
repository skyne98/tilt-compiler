@echo off
setlocal enabledelayedexpansion

REM TILT Symbolic Test Runner (Windows Batch)
REM Runs all .tilt files in symbolic_tests and compares output with expected .txt files

echo 🚀 TILT Symbolic Test Runner
echo =============================

set test_dir=symbolic_tests
set compiler=.\target\debug\tiltc.exe
set passed=0
set failed=0
set total=0

REM Build the compiler first
echo Building compiler...
cargo build --quiet

if %errorlevel% neq 0 (
    echo ❌ Failed to build compiler
    exit /b 1
)

echo.

REM Run tests
for %%f in ("%test_dir%\*.tilt") do (
    set "tilt_file=%%f"
    set "base_name=%%~nf"
    set "txt_file=%test_dir%\!base_name!.txt"
    
    echo|set /p="Testing !base_name!... "
    set /a total+=1
    
    if not exist "!txt_file!" (
        echo ❌ Missing expected output file: !txt_file!
        set /a failed+=1
    ) else (
        REM Run the TILT compiler and capture output for both VM and JIT
        "!compiler!" "!tilt_file!" --vm > temp_vm_output.txt 2>&1
        set "vm_exit_code=!errorlevel!"
        
        "!compiler!" "!tilt_file!" --jit > temp_jit_output.txt 2>&1
        set "jit_exit_code=!errorlevel!"
        
        if !vm_exit_code! neq 0 (
            for /f "delims=" %%i in (temp_vm_output.txt) do set "vm_output=%%i"
            echo ❌ VM execution failed
            echo    VM Output: !vm_output!
            set /a failed+=1
        ) else if !jit_exit_code! neq 0 (
            for /f "delims=" %%i in (temp_jit_output.txt) do set "jit_output=%%i"
            echo ❌ JIT execution failed
            echo    JIT Output: !jit_output!
            set /a failed+=1
        ) else (
            REM Extract results from both outputs
            for /f "tokens=* delims=" %%i in ('findstr "Final result: I32" temp_vm_output.txt') do (
                set "vm_result_line=%%i"
                for /f "tokens=2 delims=()" %%j in ("!vm_result_line!") do set "vm_result=%%j"
            )
            
            for /f "tokens=* delims=" %%i in ('findstr "Final result: I32" temp_jit_output.txt') do (
                set "jit_result_line=%%i"
                for /f "tokens=2 delims=()" %%j in ("!jit_result_line!") do set "jit_result=%%j"
            )
            
            if "!vm_result!"=="" (
                for /f "delims=" %%i in (temp_vm_output.txt) do set "vm_output=%%i"
                echo ❌ Could not extract VM result from output
                echo    VM Output: !vm_output!
                set /a failed+=1
            ) else if "!jit_result!"=="" (
                for /f "delims=" %%i in (temp_jit_output.txt) do set "jit_output=%%i"
                echo ❌ Could not extract JIT result from output
                echo    JIT Output: !jit_output!
                set /a failed+=1
            ) else if "!vm_result!" neq "!jit_result!" (
                echo ❌ VM/JIT mismatch ^(VM: !vm_result!, JIT: !jit_result!^)
                set /a failed+=1
            ) else (
                REM Read expected result from txt file
                for /f "tokens=3" %%e in ('findstr "Expected Output:" "!txt_file!"') do set "expected=%%e"
                
                if "!expected!"=="" (
                    echo ❌ Could not parse expected output from !txt_file!
                    set /a failed+=1
                ) else (
                    REM Compare results with expected
                    if "!vm_result!"=="!expected!" (
                        echo ✅ PASS ^(VM/JIT: !vm_result!^)
                        set /a passed+=1
                    ) else (
                        echo ❌ FAIL ^(expected !expected!, got !vm_result!^)
                        set /a failed+=1
                    )
                )
            )
        )
    )
)

echo.
echo =============================
echo Test Results:
echo   Total:  !total!
echo   Passed: !passed!
echo   Failed: !failed!

REM Clean up temporary files
if exist temp_vm_output.txt del temp_vm_output.txt
if exist temp_jit_output.txt del temp_jit_output.txt

if !failed! equ 0 (
    echo 🎉 All tests passed!
    exit /b 0
) else (
    echo 💥 Some tests failed
    exit /b 1
)
