#!/usr/bin/env python3

import os
import re
import sys

def process_file(filepath):
    """Process a single test file to convert all differential_evolution calls."""
    
    with open(filepath, 'r') as f:
        content = f.read()
    
    filename = os.path.basename(filepath)
    
    # Count existing calls
    de_count = content.count('differential_evolution(')
    recorded_count = content.count('run_recorded_differential_evolution(')
    
    if de_count == recorded_count:
        return None  # Already fully converted
    
    print(f"Processing {filename}: {de_count - recorded_count} calls to convert")
    
    # Fix imports
    content = re.sub(
        r'use autoeq_de::\{[^}]*differential_evolution,\s*',
        'use autoeq_de::{',
        content
    )
    
    # Ensure run_recorded_differential_evolution is imported
    if 'run_recorded_differential_evolution' not in content:
        content = re.sub(
            r'(use autoeq_de::\{)([^}]+)(\})',
            r'\1run_recorded_differential_evolution, \2\3',
            content,
            count=1
        )
    
    # Process each test function
    test_functions = re.findall(r'fn\s+(test_\w+)\s*\(\)', content)
    
    for test_name in test_functions:
        # Find the test function body
        test_pattern = rf'fn\s+{test_name}\s*\(\)\s*\{{.*?\n\}}'
        test_match = re.search(test_pattern, content, re.DOTALL)
        
        if not test_match:
            continue
            
        test_body = test_match.group(0)
        original_test_body = test_body
        
        # Find all differential_evolution calls in this test
        de_calls = list(re.finditer(
            r'((?:let\s+)?(?:result|r|res)\s*=\s*)?differential_evolution\s*\(\s*&?(\w+),\s*&?(\w+),\s*(\w+)\s*\)',
            test_body
        ))
        
        if not de_calls:
            continue
        
        # Process each call
        call_num = 0
        for match in reversed(de_calls):  # Process in reverse to maintain positions
            call_num += 1
            
            assignment = match.group(1) or ''
            func_arg = match.group(2)
            bounds_arg = match.group(3)
            config_arg = match.group(4)
            
            # Generate recording name
            record_name = test_name.replace('test_de_', '').replace('test_', '')
            if len(de_calls) > 1:
                record_name += f"_{call_num}"
            
            # Build replacement
            replacement = f'''{assignment}run_recorded_differential_evolution(
        "{record_name}", {func_arg}, &{bounds_arg}, {config_arg}, "./data_generated/records"
    )'''
            
            # If this was a direct assertion, wrap it properly
            if not assignment:
                # Look for the assertion that follows
                pos = match.end()
                rest_of_test = test_body[pos:]
                if rest_of_test.lstrip().startswith('.fun'):
                    # Direct property access - need to wrap
                    replacement = f'''run_recorded_differential_evolution(
        "{record_name}", {func_arg}, &{bounds_arg}, {config_arg}, "./data_generated/records"
    ).unwrap().0'''
            
            test_body = test_body[:match.start()] + replacement + test_body[match.end():]
        
        # Add error handling if needed
        if 'let result = run_recorded_differential_evolution' in test_body:
            # Find positions where we need to add error handling
            lines = test_body.split('\n')
            new_lines = []
            i = 0
            while i < len(lines):
                new_lines.append(lines[i])
                if 'let result = run_recorded_differential_evolution' in lines[i]:
                    # Find the end of the call
                    j = i
                    while j < len(lines) and ')' not in lines[j]:
                        j += 1
                        if j < len(lines):
                            new_lines.append(lines[j])
                    
                    # Add error handling
                    indent = len(lines[i]) - len(lines[i].lstrip())
                    if not any('assert!(result.is_ok())' in lines[k] for k in range(j+1, min(j+3, len(lines)))):
                        new_lines.append(' ' * indent + 'assert!(result.is_ok());')
                        new_lines.append(' ' * indent + 'let (report, _csv_path) = result.unwrap();')
                    i = j
                i += 1
            test_body = '\n'.join(new_lines)
        
        # Update result references to report
        test_body = re.sub(r'\bresult\.fun\b', 'report.fun', test_body)
        test_body = re.sub(r'\bresult\.x\b', 'report.x', test_body)
        test_body = re.sub(r'\bresult\.nit\b', 'report.nit', test_body)
        
        # Replace the test in the content
        content = content.replace(original_test_body, test_body)
    
    # Fix any bounds that aren't vec!
    content = re.sub(r'let\s+(\w+)\s*=\s*\[([^\]]+)\];(\s*//[^\n]*)?\n(\s+)let\s+(\w+)\s*=', 
                     lambda m: f'let {m.group(1)} = vec![{m.group(2)}];{m.group(3) or ""}\n{m.group(4)}let {m.group(5)} =',
                     content)
    
    return content

def main():
    test_dir = '/Users/pierrre/src/autoeq/src-de/tests'
    
    # Get all test files
    test_files = [f for f in os.listdir(test_dir) if f.startswith('optde_') and f.endswith('.rs')]
    
    converted_count = 0
    for filename in sorted(test_files):
        filepath = os.path.join(test_dir, filename)
        
        new_content = process_file(filepath)
        if new_content:
            with open(filepath, 'w') as f:
                f.write(new_content)
            converted_count += 1
            print(f"  ✓ Converted {filename}")
    
    print(f"\nTotal files converted: {converted_count}")

if __name__ == '__main__':
    main()
