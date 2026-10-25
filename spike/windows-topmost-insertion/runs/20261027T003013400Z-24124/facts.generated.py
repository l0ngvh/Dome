import collections
import hashlib
import json
import pathlib
root = pathlib.Path(__file__).parent
verification = json.loads((root / 'verification.generated.json').read_text())
analysis = json.loads((root / 'analysis.json').read_text())
records = [(i, json.loads(line)['record']) for i, line in enumerate((root / 'raw.jsonl').open(), 1)]
print('Totals', collections.Counter(row['status'] for row in analysis['trials']), 'control failed', sum(row.get('control_passed') is False for row in analysis['trials']))
print('Original file stats', verification['original_file_count'], verification['original_total_bytes'])
print('Raw bytes', (root / 'raw.jsonl').stat().st_size)
print('Nonempty stderr', [(item['file'], item['bytes']) for item in verification['original_files'] if 'stderr' in item['file'] and item['bytes']])
print('IME and desktops', len(verification['ready_records']), collections.Counter(row['environment'] for row in verification['ready_records']))
print('Unexpected windows', [(row['case_number'], row['repetition'], row['unexpected_fixture_process_windows']) for row in verification['trials'] if row['unexpected_fixture_process_windows']])
print('Invalid snapshots by case')
for invalid in verification['invalid_snapshots']:
    trial = next(t for t in verification['trials'] if t['lines']['trial_begin'][0] <= invalid['line'] <= t['lines']['trial_end'][0])
    print(trial['case_number'], trial['repetition'], invalid)
print('Case16 immediates', [(t['repetition'], t['stages']['immediate_after_release']) for t in verification['trials'] if t['case_number'] == 16])
for number in (6,8,20,21,22,23,24,25,26,27,28,29):
    for trial in verification['trials']:
        if trial['case_number'] != number:
            continue
        print('CASE', number, 'rep', trial['repetition'], 'anchors', {k:v for k,v in trial['lines'].items() if k in ('trial_begin','owned_identity','setup_x','before','setup_eligibility','before_insert','treatment_call','after_explicit_topmost','trial_end','bottom_global_endpoint')}, 'plateau', trial['endpoints'].get('settling',{}).get('lines'), 'ownership', trial['ownership'][-1] if number in range(22,28) else None)
for number in (16,17,18):
    for trial in verification['trials']:
        if trial['case_number'] == number:
            print('RACE', number, trial['repetition'], 'anchors', {k:v for k,v in trial['lines'].items() if k in ('before','queried_normal_a_before_c_post','external_async_call','after_a_promotion_while_c_held','after_a_demotion_while_c_held','before_c_release','immediate_after_release')}, trial['race'], 'plateau',trial['endpoints']['settling']['lines'])
for number in (6,8,21):
    for trial in verification['trials']:
        if trial['case_number'] != number:
            continue
        assert trial['before']['global_chain'] == trial['stages']['before_insert']['global_chain']
        expected = {6:['AT','BN','PN','XN'],8:['BT','AT','XN','PN'],21:['AT','BT','PT','XN']}[number]
        assert trial['before']['global_chain'] == expected
for number in (23,25):
    for trial in verification['trials']:
        if trial['case_number'] == number:
            assert trial['before']['bands'] == 'TTNNT'
            assert trial['stages']['setup_x']['bands'] == 'TTNNT'
for number in (28,29):
    for trial in verification['trials']:
        if trial['case_number'] == number:
            assert trial['stages']['bottom_global_endpoint']['global_chain'] == ['AT','BT','PN','XN']
print('Raw global-premise, rejection-band and Bottom assertions passed')
for item in verification['original_files']:
    assert hashlib.sha256((root / item['file']).read_bytes()).hexdigest() == item['sha256']
print('All 53 original files unchanged since diagnostic inventory')
