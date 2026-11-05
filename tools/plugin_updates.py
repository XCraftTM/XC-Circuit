"""Block updates without an app update: packs the bundled block plugins
(Circuit Core, Discord Bot, Fluxer Bot …) as zips with an index.json, which
the Plugins workflow publishes on the repository's "plugins" release. The app
downloads a plugin from there when its version is higher than its own copy.

    python tools/plugin_updates.py pack [out-dir]   # zips + index.json (default dist/plugin-updates)
    python tools/plugin_updates.py check            # a changed plugin must have a new version

`check` compares every plugin with the published index: a plugin whose files
changed while its version stayed the same (or went down) fails, with what to
do. Native plugins (compiled code) aren't published this way.
"""
import hashlib
import json
import os
import sys
import tomllib
import urllib.request
import zipfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PLUGINS = os.path.join(ROOT, 'plugins')
INDEX_URL = 'https://github.com/XCraftTM/XC-Circuit/releases/download/plugins/index.json'
TEXT = ('.js', '.cjs', '.mjs', '.ts', '.toml', '.json', '.md', '.svg', '.txt', '.css', '.html')
# The same time for every file, so the same content gives the same zip.
STAMP = (2020, 1, 1, 0, 0, 0)


def plugins():
    """The publishable plugins: (folder, manifest), by id."""
    out = []
    for name in sorted(os.listdir(PLUGINS)):
        toml_path = os.path.join(PLUGINS, name, 'plugin.toml')
        if not os.path.isfile(toml_path):
            continue
        with open(toml_path, 'rb') as f:
            manifest = tomllib.load(f)
        if 'native' in manifest:
            continue
        out.append((os.path.join(PLUGINS, name), manifest))
    return out


def files_of(folder):
    """Every file of a plugin, by its path inside the plugin, sorted."""
    out = []
    for d, dirs, files in os.walk(folder):
        dirs[:] = sorted(x for x in dirs if not x.startswith('.') and x != 'node_modules')
        for f in sorted(files):
            if f.startswith('.'):
                continue
            full = os.path.join(d, f)
            out.append((os.path.relpath(full, folder).replace(os.sep, '/'), full))
    return out


def read(full):
    """A file's bytes; text with \\n line ends, so checkouts on Windows and Linux agree."""
    data = open(full, 'rb').read()
    return data.replace(b'\r\n', b'\n') if full.endswith(TEXT) else data


def content_hash(folder):
    h = hashlib.sha256()
    for rel, full in files_of(folder):
        h.update(rel.encode() + b'\0' + read(full) + b'\0')
    return h.hexdigest()


def pack(out_dir):
    os.makedirs(out_dir, exist_ok=True)
    entries = []
    for folder, m in plugins():
        pid, version = m['id'], m['version']
        file = f'{pid}-{version}.zip'
        path = os.path.join(out_dir, file)
        with zipfile.ZipFile(path, 'w', zipfile.ZIP_DEFLATED) as z:
            for rel, full in files_of(folder):
                info = zipfile.ZipInfo(f'{pid}/{rel}', STAMP)
                info.compress_type = zipfile.ZIP_DEFLATED
                info.external_attr = 0o644 << 16
                z.writestr(info, read(full))
        data = open(path, 'rb').read()
        entries.append({
            'id': pid,
            'name': m.get('name', pid),
            'version': version,
            'app': (m.get('engines') or {}).get('app'),
            'file': file,
            'size': len(data),
            'sha256': hashlib.sha256(data).hexdigest(),
            'contentHash': content_hash(folder),
        })
        print(f'{pid} {version}: {file} ({len(data) // 1024} KB)')
    with open(os.path.join(out_dir, 'index.json'), 'w', encoding='utf-8', newline='\n') as f:
        json.dump({'format': 1, 'plugins': entries}, f, indent=2)
        f.write('\n')


def version_key(v):
    return tuple(int(p) if p.isdigit() else 0 for p in v.lstrip('v').split('-')[0].split('.'))


def check():
    try:
        with urllib.request.urlopen(INDEX_URL, timeout=20) as r:
            published = {p['id']: p for p in json.load(r)['plugins']}
    except Exception as e:  # nothing published yet, or offline
        print(f'No published index to compare with ({e}); nothing to check.')
        return 0
    problems = []
    for folder, m in plugins():
        pid, version = m['id'], m['version']
        old = published.get(pid)
        if not old:
            continue
        changed = content_hash(folder) != old['contentHash']
        if version_key(version) < version_key(old['version']):
            problems.append(f'{pid}: version {version} is lower than the published {old["version"]}.')
        elif changed and version_key(version) == version_key(old['version']):
            problems.append(
                f'{pid}: its files changed, but its version is still {version}. '
                f'Raise `version` in plugins/{os.path.basename(folder)}/plugin.toml (such as {bump(version)}), '
                'so apps notice the update.'
            )
    for p in problems:
        print(p)
    if not problems:
        print('Every changed plugin has a new version.')
    return 1 if problems else 0


def bump(version):
    parts = list(version_key(version)) + [0, 0, 0]
    return f'{parts[0]}.{parts[1]}.{parts[2] + 1}'


if __name__ == '__main__':
    what = sys.argv[1] if len(sys.argv) > 1 else ''
    if what == 'pack':
        pack(sys.argv[2] if len(sys.argv) > 2 else os.path.join(ROOT, 'dist', 'plugin-updates'))
    elif what == 'check':
        sys.exit(check())
    else:
        print(__doc__)
        sys.exit(2)
