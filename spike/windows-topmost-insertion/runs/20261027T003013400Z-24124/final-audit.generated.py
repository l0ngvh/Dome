import collections
import hashlib
import json
import pathlib

root = pathlib.Path(__file__).parent
verification = json.loads((root / 'verification.generated.json').read_text())
analysis = json.loads((root / 'analysis.json').read_text())
records = [(number, json.loads(text)['record']) for number, text in enumerate((root / 'raw.jsonl').open(), 1)]
by_line = dict(records)
identities = set()
for row in verification['ready_records']:
    if 'handle' in row:
        identities.add((row['handle'], row['process'], row['thread']))
for _, row in records:
    if row['kind'] == 'overlay_identity':
        identities.add((row['handle'], row['process'], row['thread']))
    if row['kind'] == 'owned_identity':
        window = row['window']
        identities.add((window['handle'], window['process'], window['thread']))
for trial in verification['trials']:
    for item in trial['ownership']:
        assert (item['handle'], item['process'], item['thread']) in identities
inventory_counts = collections.Counter()
for line, row in records:
    snapshot = row.get('snapshot', {})
    inventory = snapshot.get('fixture_process_classes', [])
    if not inventory:
        continue
    inventory_counts[row['kind']] += 1
    assert snapshot['enumeration_ok'] and not snapshot.get('enumeration_error')
    trial = next((trial for trial in verification['trials'] if trial['lines']['trial_begin'][0] <= line <= trial['lines']['trial_end'][0]), None)
    expected = {item['handle'] for item in trial['ownership']} if trial else {item['handle'] for item in snapshot['fixtures']}
    actual = {item['window']['handle'] for item in inventory}
    assert actual == expected, (line, expected, actual)
    assert all(item['class_name'] == 'DomeTopmostInsertionSpike' and item['class_read_error'] is None for item in inventory)
    assert all(item['window']['visible'] and item['window']['valid'] and item['window']['style_read_error'] is None for item in inventory)
    assert all((item['window']['handle'],item['window']['process'],item['window']['thread']) in identities for item in inventory)
for trial in verification['trials']:
    number = trial['case_number']
    if trial['race']:
        post_record = by_line[trial['lines']['external_async_call'][0]]
        assert post_record['call']['returned_us'] <= post_record['read_started_us'] <= post_record['read_finished_us']
        assert post_record['read_finished_us'] <= trial['race']['release_us']
        if number in (5,14):
            immediate = by_line[trial['lines']['immediate_after_local_write'][0]]['snapshot']
            delayed = by_line[trial['lines']['held_after_100ms'][0]]['snapshot']
            assert delayed['started_us'] - immediate['finished_us'] >= 100000
            assert delayed['finished_us'] <= trial['race']['release_us']
        if number in (13,15):
            changed = by_line[trial['lines']['reference_transition_observed'][0]]['snapshot']
            before_insert = by_line[trial['lines']['before_insert'][0]]['snapshot']
            assert trial['race']['release_us'] <= changed['started_us'] <= changed['finished_us'] <= before_insert['started_us']
            assert changed['fixtures'][0]['topmost'] == (number == 15)
    if trial['endpoints']:
        first = by_line[trial['endpoints']['settling']['lines'][0]]['snapshot']
        for line in trial['endpoints']['settling']['lines']:
            assert by_line[line]['snapshot']['fixtures'] == first['fixtures']
for item in verification['original_files']:
    assert hashlib.sha256((root / item['file']).read_bytes()).hexdigest() == item['sha256']
report = (root / 'REPORT.generated.md').read_text()
assert len(report.split()) <= 2000
assert all(mark not in report for mark in (';', '\u2014', '\u2013'))
assert (root / 'analysis-exit.generated.txt').read_text().strip() == '2'
assert not (root / 'analysis-stderr.generated.log').read_bytes()
output = dict(all_original_files_unchanged=True, identity_checks_passed=True,
    all_inventory_enumerations_complete=True, unexpected_companions=0, inventory_counts=dict(inventory_counts),
    post_read_and_hold_interval_checks_passed=True, final_five_full_fixture_equality=True,
    snapshot_scopes=dict(collections.Counter(row['snapshot']['scope'] for _,row in records if 'snapshot' in row)),
    hold_span_min_us=min(trial['race']['hold_span_us'] for trial in verification['trials'] if trial['race']),
    hold_span_max_us=max(trial['race']['hold_span_us'] for trial in verification['trials'] if trial['race']),
    report_word_count=len(report.split()), recorded_native_api_failures=0,
    eligible=sum(trial['status']=='eligible_observation' for trial in analysis['trials']),
    failed_controls=sum(trial.get('control_passed') is False for trial in analysis['trials']))
(root / 'final-audit.generated.json').write_text(json.dumps(output, indent=2)+'\n')
print(json.dumps(output, indent=2))
