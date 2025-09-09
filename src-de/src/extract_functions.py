#!/usr/bin/env python3
"""
Script to help extract functions from large mod.rs file into separate files.
Usage: python3 extract_functions.py
"""

import re
import os

def extract_function(content, func_name, start_line):
    """Extract a complete function from content starting at start_line."""
    lines = content.split('\n')
    if start_line >= len(lines):
        return None
    
    func_lines = []
    brace_count = 0
    in_function = False
    
    for i in range(start_line, len(lines)):
        line = lines[i]
        func_lines.append(line)
        
        # Count braces to find function end
        for char in line:
            if char == '{':
                brace_count += 1
                in_function = True
            elif char == '}':
                brace_count -= 1
        
        # Function ends when braces balance
        if in_function and brace_count == 0:
            break
    
    return '\n'.join(func_lines)

def find_functions(content):
    """Find all function definitions in the content."""
    # Pattern to match function definitions
    pattern = r'^((?:pub\s+)?fn\s+\w+.*?)$'
    functions = []
    
    lines = content.split('\n')
    for i, line in enumerate(lines):
        if re.match(pattern, line.strip()):
            # Extract function name
            func_match = re.search(r'fn\s+(\w+)', line)
            if func_match:
                func_name = func_match.group(1)
                functions.append((func_name, i, line.strip()))
    
    return functions

def extract_imports(content):
    """Extract necessary imports for the function."""
    import_lines = []
    for line in content.split('\n'):
        line = line.strip()
        if line.startswith('use ') and not line.startswith('use crate::'):
            import_lines.append(line)
        elif 'Rng' in line or 'Array' in line or 'rand::' in line or 'ndarray::' in line:
            # Add common imports
            if 'use ndarray::' not in '\n'.join(import_lines):
                import_lines.insert(0, 'use ndarray::{Array1, Array2};')
            if 'use rand::' not in '\n'.join(import_lines) and 'Rng' in line:
                import_lines.insert(0, 'use rand::Rng;')
    
    return import_lines

def create_function_file(func_name, func_content, output_dir):
    """Create a separate file for the function."""
    # Extract imports
    imports = extract_imports(func_content)
    
    # Add crate-internal imports
    internal_imports = []
    if 'distinct_indices' in func_content:
        internal_imports.append('use crate::distinct_indices;')
    if 'mutant_rand1' in func_content:
        internal_imports.append('use crate::mutant_rand1;')
    if 'LinearPenalty' in func_content:
        internal_imports.append('use crate::LinearPenalty;')
    
    # Build file content
    file_content = []
    
    # Add external imports
    if imports:
        file_content.extend(imports)
        file_content.append('')
    
    # Add internal imports
    if internal_imports:
        file_content.extend(internal_imports)
        file_content.append('')
    
    # Make function pub(crate)
    func_lines = func_content.split('\n')
    if func_lines[0].strip().startswith('fn '):
        func_lines[0] = func_lines[0].replace('fn ', 'pub(crate) fn ')
    
    file_content.extend(func_lines)
    
    # Write to file
    filename = os.path.join(output_dir, f'{func_name}.rs')
    with open(filename, 'w') as f:
        f.write('\n'.join(file_content))
    
    print(f"Created {filename}")

def main():
    # Read the original mod.rs file
    with open('mod.rs', 'r') as f:
        content = f.read()
    
    # Find all functions
    functions = find_functions(content)
    print(f"Found {len(functions)} functions:")
    
    for func_name, start_line, first_line in functions:
        print(f"  {func_name} at line {start_line + 1}: {first_line[:60]}...")
    
    print("\nExtracting functions...")
    
    # Extract each function
    for func_name, start_line, _ in functions:
        func_content = extract_function(content, func_name, start_line)
        if func_content:
            create_function_file(func_name, func_content, '.')
        else:
            print(f"Failed to extract {func_name}")

if __name__ == '__main__':
    main()
