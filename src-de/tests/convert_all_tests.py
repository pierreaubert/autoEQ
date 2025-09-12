#!/usr/bin/env python3

import os
import re
import glob

def convert_test_file(filepath):
    """Convert a test file to use run_recorded_differential_evolution."""
    
    with open(filepath, 'r') as f:
        content = f.read()
    
    # Check if already fully converted
    if 'differential_evolution(' not in content:
        return False
    
    if content.count('differential_evolution(') == content.count('run_recorded_differential_evolution('):
        return False  # Already fully converted
    
    filename = os.path.basename(filepath)
    print(f"Converting {filename}...")
    
    # Step 1: Fix imports
    # Remove 'differential_evolution' from imports if present
    content = re.sub(
        r'use autoeq_de::\{([^}]*?)differential_evolution,\s*([^}]*?)\}',
        r'use autoeq_de::{\1\2}',
        content
    )
    
    # Add run_recorded_differential_evolution if not present
    if 'run_recorded_differential_evolution' not in content:
        content = re.sub(
            r'use autoeq_de::\{([^}]+)\}',
            lambda m: f"use autoeq_de::{{{m.group(1).rstrip()}, run_recorded_differential_evolution}}",
            content,
            count=1
        )
    
    # Step 2: Convert all differential_evolution calls
    # Find test function names to generate appropriate recording names
    test_matches = list(re.finditer(r'fn\s+(test_\w+)\s*\(\)', content))
    
    # Track which test we're in based on position
    def get_test_name(pos):
        for i, match in enumerate(test_matches):
            if i + 1 < len(test_matches):
                if match.start() <= pos < test_matches[i + 1].start():
                    return match.group(1)
            else:
                if match.start() <= pos:
                    return match.group(1)
        return "unknown"
    
    # Count DE calls per test to generate unique names
    de_call_counts = {}
    
    def replace_de_call(match):
        pos = match.start()
        test_name = get_test_name(pos)
        
        # Get a simplified test name for recording
        record_name = test_name.replace('test_de_', '').replace('test_', '')
        
        # Add suffix if multiple calls in same test
        if test_name not in de_call_counts:
            de_call_counts[test_name] = 0
        de_call_counts[test_name] += 1
        
        if de_call_counts[test_name] > 1:
            record_name += f"_{de_call_counts[test_name]}"
        
        full_match = match.group(0)
        func_arg = match.group(1)
        bounds_arg = match.group(2)
        config_arg = match.group(3)
        
        # Check if bounds need to be converted to vec
        if not bounds_arg.startswith('&'):
            bounds_ref = f"&{bounds_arg}"
        else:
            bounds_ref = bounds_arg
        
        # Build the replacement
        return f'''run_recorded_differential_evolution(
        "{record_name}", {func_arg}, {bounds_ref}, {config_arg}, "./data_generated/records"
    )'''
    
    # Replace differential_evolution calls
    pattern = r'differential_evolution\s*\(\s*&?(\w+),\s*&?(\w+),\s*(\w+)\s*\)'
    content = re.sub(pattern, replace_de_call, content)
    
    # Step 3: Update result handling
    # Replace patterns like "let result = differential_evolution..." with proper error handling
    content = re.sub(
        r'(\s+)let\s+(\w+)\s*=\s*run_recorded_differential_evolution\(',
        r'\1let result = run_recorded_differential_evolution(',
        content
    )
    
    # Add error handling after run_recorded_differential_evolution calls
    lines = content.split('\n')
    new_lines = []
    i = 0
    while i < len(lines):
        line = lines[i]
        new_lines.append(line)
        
        # Check if this line starts a run_recorded_differential_evolution call
        if 'let result = run_recorded_differential_evolution(' in line:
            # Find the end of the call (matching parenthesis)
            j = i
            paren_count = line.count('(') - line.count(')')
            while paren_count > 0 and j + 1 < len(lines):
                j += 1
                new_lines.append(lines[j])
                paren_count += lines[j].count('(') - lines[j].count(')')
                i = j
            
            # Add error handling
            indent = len(line) - len(line.lstrip())
            new_lines.append(' ' * indent + 'assert!(result.is_ok());')
            new_lines.append(' ' * indent + 'let (report, _csv_path) = result.unwrap();')
        
        i += 1
    
    content = '\n'.join(new_lines)
    
    # Step 4: Update all references from result.fun to report.fun and result.x to report.x
    content = re.sub(r'\bresult\.fun\b', 'report.fun', content)
    content = re.sub(r'\bresult\.x\b', 'report.x', content)
    content = re.sub(r'\bresult\.nit\b', 'report.nit', content)
    content = re.sub(r'\bresult\.nfev\b', 'report.nfev', content)
    
    # Clean up any double error handling
    content = re.sub(
        r'assert!\(result\.is_ok\(\)\);\s*\n\s*let \(report, _csv_path\) = result\.unwrap\(\);\s*\n\s*assert!\(result\.is_ok\(\)\);',
        'assert!(result.is_ok());\n    let (report, _csv_path) = result.unwrap();',
        content
    )
    
    return content

def main():
    test_dir = '/Users/pierrre/src/autoeq/src-de/tests'
    
    # Process all optde_*.rs files
    test_files = glob.glob(os.path.join(test_dir, 'optde_*.rs'))
    
    converted = 0
    for filepath in sorted(test_files):
        new_content = convert_test_file(filepath)
        
        if new_content:
            # Write back the converted content
            with open(filepath, 'w') as f:
                f.write(new_content)
            converted += 1
            print(f"  ✓ Converted {os.path.basename(filepath)}")
    
    print(f"\nConverted {converted} files")

if __name__ == '__main__':
    main()
