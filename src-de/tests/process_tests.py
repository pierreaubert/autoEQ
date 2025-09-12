#!/usr/bin/env python3

import os
import re
import sys

def process_test_file(filepath):
    """Process a single test file to add recording capabilities."""
    
    with open(filepath, 'r') as f:
        content = f.read()
    
    filename = os.path.basename(filepath)
    
    # Check if file already has recorded tests
    has_recorded = 'run_recorded_differential_evolution' in content
    has_recorded_test = re.search(r'fn\s+test_.*_recorded\s*\(\)', content)
    
    if not has_recorded and not has_recorded_test:
        # File needs recording capabilities added
        print(f"Adding recording to: {filename}")
        
        # Add import if needed
        if 'run_recorded_differential_evolution' not in content:
            # Find existing use statement
            use_match = re.search(r'use autoeq_de::\{([^}]+)\};', content)
            if use_match:
                imports = use_match.group(1)
                if 'run_recorded_differential_evolution' not in imports:
                    new_imports = imports.rstrip() + ', run_recorded_differential_evolution'
                    content = content.replace(
                        f'use autoeq_de::{{{imports}}};',
                        f'use autoeq_de::{{{new_imports}}};'
                    )
        
        # Find all test functions that use differential_evolution
        test_pattern = r'(#\[test\]\s*fn\s+(test_[^(]+)\s*\(\)\s*\{[^}]+differential_evolution\([^}]+\})'
        
        def convert_test(match):
            full_test = match.group(0)
            test_name = match.group(2)
            
            # Extract the differential_evolution call
            de_call_match = re.search(
                r'let\s+(\w+)\s*=\s*differential_evolution\s*\(\s*&?(\w+),\s*&?(\w+),\s*(\w+)\s*\)',
                full_test
            )
            
            if de_call_match:
                result_var = de_call_match.group(1)
                func_name = de_call_match.group(2)
                bounds_var = de_call_match.group(3)
                config_var = de_call_match.group(4)
                
                # Get the test name for recording
                test_short_name = test_name.replace('test_de_', '').replace('test_', '')
                
                # Replace differential_evolution with run_recorded_differential_evolution
                new_call = f'''let result = run_recorded_differential_evolution(
        "{test_short_name}", {func_name}, &{bounds_var}, {config_var}, "./data_generated/records"
    );

    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    let {result_var} = report'''
                
                # Replace the old call
                full_test = re.sub(
                    r'let\s+\w+\s*=\s*differential_evolution\s*\([^)]+\)',
                    new_call,
                    full_test
                )
                
                # Update assertions to use report.fun and report.x
                full_test = re.sub(f'{result_var}\\.fun', 'report.fun', full_test)
                full_test = re.sub(f'{result_var}\\.x', 'report.x', full_test)
            
            return full_test
        
        # Apply conversion to all matching tests
        content = re.sub(test_pattern, convert_test, content)
        
        return content
    
    elif has_recorded_test:
        # File has _recorded tests that need to be merged
        print(f"Merging recorded tests in: {filename}")
        
        # Find and rename _recorded tests
        content = re.sub(
            r'fn\s+test_(\w+)_recorded\s*\(\)',
            r'fn test_\1()',
            content
        )
        
        return content
    
    return None

def main():
    test_dir = '/Users/pierrre/src/autoeq/src-de/tests'
    
    # Get all test files
    test_files = [f for f in os.listdir(test_dir) if f.startswith('optde_') and f.endswith('.rs')]
    
    for test_file in sorted(test_files):
        filepath = os.path.join(test_dir, test_file)
        new_content = process_test_file(filepath)
        
        if new_content:
            # Write the modified content back
            with open(filepath, 'w') as f:
                f.write(new_content)
            print(f"Updated: {test_file}")

if __name__ == '__main__':
    main()
