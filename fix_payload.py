import os
import re

for root, _, files in os.walk('src'):
    for file in files:
        if not file.endswith('.rs'): continue
        path = os.path.join(root, file)
        with open(path, 'r') as f:
            content = f.read()
        
        # Regex to find 'Transaction {' and append 'payload: vec![],' before the closing '}'
        # This is tricky because of nested structs.
        # Instead, let's find 
etwork_id: ... and append payload: vec![], after it.
        # All Transaction instantiations must set network_id.
        new_content = re.sub(r'(network_id:\s*[^,}\n]+,?)', r'\1\n            payload: vec![],', content)
        
        # We only want to replace inside Transaction { } but the above is safe enough for network_id
        if new_content != content:
            with open(path, 'w') as f:
                f.write(new_content)
            print('Updated', path)
