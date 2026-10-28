#!/usr/bin/env python3
"""Copy the browser UI for static hosting; calculations stay in the Rust service."""
import argparse
import json
from pathlib import Path
import shutil
from urllib.parse import urlsplit


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--api-url', help='HTTPS backend origin, or localhost HTTP for a local test')
    args = parser.parse_args()
    base = args.api_url
    if base:
        url = urlsplit(base)
        local = url.hostname in ('localhost', '127.0.0.1', '::1')
        if (url.scheme != 'https' and not (local and url.scheme == 'http')) or not url.hostname or url.username or url.password or url.path not in ('', '/') or url.query or url.fragment:
            parser.error('--api-url must be an HTTPS origin without a path (localhost HTTP is allowed)')
        base = base.rstrip('/')
    root = Path(__file__).resolve().parent.parent
    if args.output.resolve() == (root/'web').resolve():
        parser.error('--output must not overwrite the source web directory')
    args.output.mkdir(parents=True, exist_ok=True)
    for name in ('index.html', 'styles.css', 'basemap.svg', 'app.js', 'api-client.js'):
        shutil.copy2(root/'web'/name, args.output/name)
    (args.output/'config.js').write_text('// Generated public API origin. No credentials belong in this file.\nexport const apiBase = '+json.dumps(base or None)+';\n')
    print(f'Browser UI exported to {args.output}; API: {base or "not configured"}')


if __name__ == '__main__':
    main()
