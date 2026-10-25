import json
import pathlib
import collections
import hashlib

p = pathlib.Path(__file__).parent
a = json.loads((p / 'analysis.json').read_text())
print({k: v for k, v in a.items() if k not in ('matrix', 'trials')})
for k, v in a['matrix'].items():
    trials = [t for t in a['trials'] if t['environment'] + '/' + t['case'] == k]
    print(k, v, collections.Counter(x.get('reason', '') for x in trials), [{z: x.get(z) for z in ('settled', 'owner_group', 'control_passed')} for x in trials[:1]])
binary = (p / 'windows-topmost-insertion.exe').read_bytes()
out = []
for entry in json.loads((p / 'hashes.json').read_text()):
    path = p / entry['file'] if entry['file'].endswith('.exe') else p / 'source' / entry['file']
    data = path.read_bytes()
    actual = hashlib.sha256(data).hexdigest()
    normalized = data.replace(b'\r\n', b'\n').replace(b'\n', b'\r\n')
    direct = actual == entry['sha256']
    reconstruction = hashlib.sha256(normalized).hexdigest() == entry['sha256']
    reconstructed = data if direct else normalized
    out.append(dict(**entry, pulled_sha256=actual, direct_match=direct, crlf_match=reconstruction, embedded=None if path.suffix == '.exe' else reconstructed in binary))
raw_hash = hashlib.sha256((p / 'raw.jsonl').read_bytes()).hexdigest()
report = dict(manifest=out, raw_sha256=raw_hash, native_raw_manifest=json.loads((p / 'raw-native-sha256.json').read_text()))
(p / 'provenance.generated.json').write_text(json.dumps(report, indent=2) + '\n')
print('PROVENANCE', json.dumps(report))
