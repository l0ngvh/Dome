"""Generated read-only schema 6 diagnostic. Writes only generated files beside this script."""
import collections
import hashlib
import json
import pathlib

root = pathlib.Path(__file__).resolve().parent
raw = (root / 'raw.jsonl').read_bytes()
records = [(number, {**wrapper['record'], 'recorded_us': wrapper['unix_us']}) for number, line in enumerate(raw.decode('utf-8-sig').splitlines(), 1) for wrapper in [json.loads(line)]]
analysis = json.loads((root / 'analysis.json').read_text())
cases = records[0][1]['case_names']
errors = []
checks = collections.Counter()

def check(condition, message, line=None):
    checks[message] += 1
    if not condition:
        errors.append({'check': message, 'line': line})

def identity(window):
    return tuple(window[key] for key in ('handle', 'process', 'thread'))

def state(snapshot, slots):
    fixtures = snapshot['fixtures']
    names = {window['handle']: slot for slot, window in zip(slots, fixtures)}
    graph = {window['handle']: window for window in snapshot.get('raw_enumeration', snapshot.get('neighbors', []))}
    def precedes(left, right):
        seen = set()
        current = left
        while current in graph and current not in seen:
            seen.add(current)
            current = graph[current]['below']
            if current == right:
                return True
        return False
    order = sorted(names, key=lambda handle: sum(precedes(other, handle) for other in names))
    return {'bands': {slot: window['topmost'] for slot, window in zip(slots, fixtures)},
            'order': '-'.join(names[handle] for handle in order),
            'windows': {slot: window for slot, window in zip(slots, fixtures)},
            'started_us': snapshot['started_us'], 'finished_us': snapshot['finished_us']}

trials = []
active = None
environment = None
ready = {}
controllers = {}
owned_lifetimes = []
outside_calls = []
for line, record in records:
    kind = record['kind']
    if kind == 'environment_ready':
        environment = record['environment']
        controllers[environment] = (record['process'], record['thread'])
        ready = {}
    if kind == 'overlay_identity':
        ready['X'] = identity(record['initial'])
    if kind == 'fixture_reply' and record['reply']['record'].get('kind') == 'ready':
        reply = record['reply']['record']
        spawn = next(row for _, row in reversed(records[:line]) if row['kind'] == 'spawn' and row['process'] == record['process'])
        ready['P' if spawn['label'] == 'normalA' else spawn['label']] = identity(reply['initial'])
        check(reply['desktop']['verified'] and reply['input_method']['ok'] and reply['input_method']['before_first_window'], 'fixture desktop and IME verified', line)
        check(reply['desktop']['requested'] == next(row['desktop']['requested'] for _, row in records if row['kind'] == 'environment_ready' and row['environment'] == environment), 'fixture desktop matches environment', line)
    if kind == 'trial_begin':
        active = {'begin': record, 'line': line, 'records': [], 'ready': ready.copy(), 'environment': environment}
    if active is not None:
        active['records'].append((line, record))
    elif 'call' in record or kind == 'fixture_reply' and 'call' in record['reply']['record']:
        outside_calls.append(line)
    if kind == 'trial_end':
        trials.append(active)
        active = None

check(not outside_calls, 'no native calls outside trial envelopes')
flags = collections.Counter()
call_durations = []
settling_gaps = []
settling_spans = []
snapshot_durations = []
invalid_native = []
inventories = []
outputs = []
for trial in trials:
    begin = trial['begin']
    number = cases.index(begin['case']) + 1
    rows = trial['records']
    slots = begin['fixture_slots']
    snapshots = {record['kind']: (line, record['snapshot']) for line, record in rows if 'snapshot' in record and record['kind'] not in ('settling', 'first_settling')}
    ending = rows[-1][1]
    start, end = begin['recorded_us'], ending['recorded_us']
    before = snapshots['before'][1]
    fixtures = before['fixtures']
    handles = {slot: window['handle'] for slot, window in zip(slots, fixtures)}
    expected = [identity(window) for window in fixtures]
    for slot, window in zip(slots[:4], fixtures[:4]):
        check(identity(window) == trial['ready'][slot], 'baseline identities match environment creation', trial['line'])
    creations = [(line, record) for line, record in rows if record['kind'] == 'owned_identity']
    if creations:
        line, creation = creations[0]
        window, call = creation['window'], creation['creation']
        owner_index = 0 if begin['owned_by'] == 'A' else 3
        owner = fixtures[owner_index]
        check(identity(window) == expected[4] and window['owner'] == owner['handle'] == call['owner'], 'owned identity matches this lifetime and native owner', line)
        check((window['process'], window['thread']) == (owner['process'], owner['thread']) == (call['calling_process'], call['calling_thread']), 'owned constructor owner-local identity', line)
        check(call['style'] == 0x90000000 and call['extended_style'] == 0x08000080, 'owned constructor popup visible without explicit topmost', line)
        created = snapshots['after_owned_creation'][1]
        check(start <= call['started_us'] <= call['returned_us'] <= created['started_us'] <= created['finished_us'] <= end, 'owned creation bracketed by full observation', line)
        if number in (30, 31, 32):
            promoted = snapshots['owner_promoted_before_creation'][1]
            check(len(promoted['fixtures']) == 4 and identity(promoted['fixtures'][3]) == expected[3] and promoted['fixtures'][3]['topmost'], 'actual X topmost before O creation', line)
            check(promoted['finished_us'] <= call['started_us'], 'full precreation reread ends before constructor starts', line)
            check(all(created['fixtures'][index]['topmost'] for index in (3, 4)), 'creation inherits owned flag and preserves owner flag', line)
        if number in (33, 34):
            check(not any(created['fixtures'][index]['topmost'] for index in (3, 4)), 'ordered-promotion pair created normal', line)
        if number in (30, 31, 32, 33, 34):
            check(all(before['fixtures'][index]['topmost'] for index in (3, 4)) and all(snapshots['before_insert'][1]['fixtures'][index]['topmost'] for index in (3, 4)), 'both-topmost premise verified before treatment', line)
        owned_lifetimes.append({'environment': trial['environment'], 'case': number, 'repetition': begin['repetition'], 'identity': identity(window), 'start': start, 'end': end, 'line': line})
    unique = {}
    target_reads = []
    for line, record in rows:
        payload = record['reply']['record'] if record['kind'] == 'fixture_reply' else record
        if 'call' not in payload:
            continue
        call = payload['call']
        key = tuple(call[key] for key in ('calling_process', 'calling_thread', 'started_us'))
        if key in unique:
            existing = unique[key]
            check(existing['call'] == call and {existing['kind'], record['kind']} == {'fixture_reply', 'treatment_call'}, 'only identical native reply treatment duplicate', line)
        else:
            unique[key] = {'call': call, 'line': line, 'kind': record['kind']}
        if record['kind'] in ('fixture_reply', 'external_async_call'):
            check('post_return_target_read' in payload, 'every fixture or async call has target read', line)
            read = payload['post_return_target_read']
            following = min(s['started_us'] for _, row in rows if (s := row.get('snapshot')) and s['started_us'] >= call['returned_us'])
            check(read['handle'] == call['window'] and identity(read) in expected, 'native post-return target read identity', line)
            check(call['returned_us'] <= payload['read_started_us'] <= payload['read_finished_us'] <= following, 'post-return read before earliest collector read by timestamp', line)
            target_reads.append(line)
    calls = sorted(unique.values(), key=lambda row: row['call']['started_us'])
    previous = start
    for entry in calls:
        call, line = entry['call'], entry['line']
        check(previous <= call['started_us'] <= call['returned_us'] <= end, 'native calls chronological nonoverlapping within lifetime', line)
        previous = call['returned_us']
        check(call['ok'] and call['error'] is None and call['last_error_on_failure'] is None, 'native API reports success', line)
        check(call['api'] == 'SetWindowPos' and all(call[key] == 0 for key in ('x', 'y', 'width', 'height')), 'only zero-geometry SetWindowPos calls', line)
        owner = next(window for window in fixtures if window['handle'] == call['window'])
        expected_caller = controllers[trial['environment']] if call['flags'] == 16403 else (owner['process'], owner['thread'])
        check((call['calling_process'], call['calling_thread']) == expected_caller, 'native call owning thread or explicit async controller', line)
        flags[call['flags']] += 1
        call_durations.append(call['returned_us'] - call['started_us'])
    allowed = []
    def allow(slot, after, kind, preceding=None, flags_value=19):
        snapshot = snapshots[kind][1]
        lower = preceding if preceding is not None else (snapshots[allowed[-1][5]][1]['finished_us'] if allowed else start)
        allowed.append((handles[slot], handles.get(after, after), flags_value, lower, snapshot['started_us'], kind))
        return snapshot['finished_us']
    if number in (30, 31, 32, 33, 34):
        allow('X', -2, 'owner_reset_before_creation')
    if number in (30, 31, 32):
        allow('X', -1, 'owner_promoted_before_creation', snapshots['owner_reset_before_creation'][1]['finished_us'])
    after_creation = snapshots['after_owned_creation'][1]['finished_us'] if creations else start
    allow('P', -2, 'setup_normal_reference', after_creation)
    allow('B', -2 if number in (6, 7) else -1, 'setup_b', snapshots['setup_normal_reference'][1]['finished_us'])
    allow('A', -2 if number in (14, 15, 16, 17, 18) else -1, 'setup_a', snapshots['setup_b'][1]['finished_us'])
    if number in (16, 17, 18):
        allow('C', -2, 'setup_c', snapshots['setup_a'][1]['finished_us'])
    if number == 34:
        allow('O', -1, 'setup_o_first', snapshots['setup_a'][1]['finished_us'])
    if number not in (30, 31, 32):
        allow('X', -1 if number in (4, 9, 10, 23, 25, 29, 33, 34) else -2, 'setup_x')
    if number == 33:
        allow('O', -1, 'setup_o_last', snapshots['setup_x'][1]['finished_us'])
    extras = {8: [('A', 'B', 'setup_clean_prefix_a'), ('X', 'P', 'setup_displaced_x')], 28: [('X', -1, 'setup_displacement_promote'), ('X', -2, 'setup_displaced_x')], 20: [('X', 'P', 'setup_normal_predecessor')], 21: [('P', -1, 'setup_topmost_predecessor'), ('P', 'B', 'setup_clean_prefix_p'), ('X', -1, 'setup_boundary_overlay_promote'), ('X', -2, 'setup_boundary_overlay')]}
    if number in (16, 17, 18):
        extras[number] = [('X', 'A', 'setup_race_x_after_a'), ('P', 'X', 'setup_race_p_after_x'), ('C', 'P', 'setup_race_c_after_p')]
    for slot, after, kind in extras.get(number, []):
        allow(slot, after, kind)
    setup_count = len(allowed)
    if number not in (23, 25):
        if number in (5, 13, 14, 15, 16, 17, 18):
            race = number in (16, 17, 18)
            allow('C' if race else 'A', 'A' if race else -1 if number in (14, 15) else -2, 'after_async_post_while_held', snapshots['queried_normal_a_before_c_post' if race else 'held_before_post'][1]['finished_us'], 16403)
            if number in (16, 18):
                allow('A', -1, 'after_a_promotion_while_c_held', snapshots['after_async_post_while_held'][1]['finished_us'])
            if number == 18:
                allow('A', -2, 'after_a_demotion_while_c_held', snapshots['after_a_promotion_while_c_held'][1]['finished_us'])
        treatment_flags = 531 if number in (35, 36) else 19
        if number in (2, 3, 10, 26, 36):
            promote = number in (3, 26, 36)
            allow('X', -1 if promote else -2, 'after_explicit_topmost' if promote else 'after_explicit_notopmost', before['finished_us'], treatment_flags)
        if number not in (16, 17, 18):
            target = 'O' if number in (24, 31) else 'A' if number in (19, 20, 21, 27) else 'X'
            reference = 1 if number in (28, 29) else -1 if number in (22, 24, 35) else -2 if number in (30, 31, 33, 34) else 'X' if number in (19, 27) else 'P' if number in (4, 11, 20, 21) else 'A'
            allow(target, reference, 'immediate_after_local_write', snapshots['before_insert'][1]['finished_us'], treatment_flags)
            if number == 12:
                allow('X', 'A', 'immediate_after_repeated_write', snapshots['before_repeated_insert'][1]['finished_us'])
    check(len(calls) == len(allowed), 'all calls accounted for by source-derived allowlist', trial['line'])
    for entry, (window, after, flag, earliest, latest, kind) in zip(calls, allowed):
        call = entry['call']
        check((call['window'], call['after'], call['flags']) == (window, after, flag) and earliest <= call['started_us'] <= call['returned_us'] <= latest, 'call exactly inside allowed observation interval', entry['line'])
    final_samples = [(line, record) for line, record in rows if record['kind'] == 'settling']
    if final_samples:
        check([record['sample'] for _, record in final_samples] == list(range(21)), 'all scheduled final samples retained', trial['line'])
        stable = [record['snapshot'] for _, record in final_samples[-5:]]
        check(all(not s['invalid_reason'] for s in stable) and len({json.dumps(s['fixtures'], sort_keys=True) for s in stable}) == 1, 'final five valid and identical full fixture fields', final_samples[-5][0])
        settling_spans.append(final_samples[-1][1]['snapshot']['finished_us'] - final_samples[0][1]['snapshot']['started_us'])
        settling_gaps += [right['snapshot']['started_us'] - left['snapshot']['finished_us'] for (_, left), (_, right) in zip(final_samples, final_samples[1:])]
        last = snapshots.get('bottom_global_endpoint', (None, final_samples[-1][1]['snapshot']))[1]
        check(all(entry['call']['returned_us'] <= last['finished_us'] for entry in calls), 'no unexpected post-endpoint write', trial['line'])
    stages = {}
    for line, record in rows:
        if 'snapshot' not in record:
            continue
        snapshot = record['snapshot']
        snapshot_durations.append(snapshot['finished_us'] - snapshot['started_us'])
        if snapshot['invalid_reason']:
            invalid_native.append({'line': line, 'kind': record['kind'], 'case': number, 'reason': snapshot['invalid_reason']})
            continue
        windows = snapshot['fixtures']
        permitted = expected[:4] if record['kind'] in ('owner_reset_before_creation', 'owner_promoted_before_creation', 'failure_inventory') else expected
        check([identity(w) for w in windows] == permitted, 'snapshot lifetime-scoped identities', line)
        graph = {window['handle']: window for window in snapshot.get('raw_enumeration', snapshot.get('neighbors', []))}
        check(len(graph) == len(snapshot.get('raw_enumeration', snapshot.get('neighbors', []))), 'neighbor graph has unique handles', line)
        for window in windows:
            check(window['handle'] in graph and all(graph[window['handle']][key] == window[key] for key in ('process', 'thread', 'above', 'below', 'valid')), 'fixture fields match traversed neighbor graph', line)
        for handle, node in graph.items():
            for direction, reciprocal in (('above', 'below'), ('below', 'above')):
                adjacent = node[direction]
                if adjacent in graph:
                    check(graph[adjacent][reciprocal] == handle, 'neighbor edges reciprocal', line)
        if snapshot['scope'] == 'global':
            heads = [handle for handle, node in graph.items() if node['above'] == 0]
            chain = []
            current = heads[0] if len(heads) == 1 else None
            while current in graph and current not in chain:
                chain.append(current)
                current = graph[current]['below']
            check(len(heads) == 1 and current == 0 and len(chain) == len(graph), 'global graph single complete chain', line)
            if trial['environment'] == 'private':
                check(set(graph) == set(handles.values()), 'private global enumeration only scheduled fixtures', line)
        if snapshot['scope'] == 'local':
            check(snapshot['fixture_rereads'] == windows, 'full fixture rereads agree', line)
        for window in windows:
            check(window['valid'] and window['visible'] and window['style_read_error'] is None and bool(window['extended_style'] & 8) == window['topmost'], 'native styles and visibility agree', line)
        if len(windows) == 5 and begin['owned_by']:
            check(windows[4]['owner'] == windows[0 if begin['owned_by'] == 'A' else 3]['handle'], 'every snapshot native GW_OWNER agrees', line)
        if 'fixture_process_classes' in snapshot:
            inventory = snapshot['fixture_process_classes']
            allowed_handles = set(handles.values())
            check(all(row['class_name'] == 'DomeTopmostInsertionSpike' and row['class_read_error'] is None and row['window']['handle'] in allowed_handles for row in inventory), 'inventories contain only scheduled fixture classes and handles', line)
            inventories.append({'line': line, 'environment': trial['environment'], 'count': len(inventory), 'case': number})
        if record['kind'] != 'settling' or record['sample'] in (16, 20):
            stages[record['kind'] + ('_' + str(record['sample']) if 'sample' in record else '')] = {'line': line, **state(snapshot, slots)}
    result = next(row for row in analysis['trials'] if (row['case'], row['environment'], row['repetition']) == (begin['case'], begin['environment'], begin['repetition']))
    outputs.append({'number': number, 'environment': begin['environment'], 'repetition': begin['repetition'], 'begin_line': trial['line'], 'end_line': rows[-1][0], 'status': result['status'], 'question_observed': result.get('question_observed', False), 'reference_expectation_contradicted': result.get('reference_expectation_contradicted', False), 'calls': calls, 'target_reads': target_reads, 'stages': stages})

manifest = json.loads((root / 'hashes.json').read_text())
executable = (root / 'windows-topmost-insertion.exe').read_bytes()
hashes = []
for item in manifest:
    path = root / ('source/' + item['file'] if item['file'] != 'windows-topmost-insertion.exe' else item['file'])
    data = path.read_bytes()
    reconstructed = data.replace(b'\r\n', b'\n').replace(b'\n', b'\r\n')
    direct = hashlib.sha256(data).hexdigest() == item['sha256']
    restored = hashlib.sha256(reconstructed).hexdigest() == item['sha256']
    matching = data if direct else reconstructed
    embedded = matching in executable if item['file'] != 'windows-topmost-insertion.exe' else None
    check(direct or restored, 'manifest source or executable bytes verified')
    if embedded is not None:
        check(embedded, 'all source bytes embedded in executable')
    hashes.append({'file': item['file'], 'direct_hash_match': direct, 'crlf_reconstruction_match': restored, 'embedded_bytes_match': embedded})
check(hashlib.sha256(raw).hexdigest() == json.loads((root / 'raw-native-sha256.json').read_text())['sha256'], 'native supervisor raw hash matches')
logs = [{'path': str(path.relative_to(root)), 'bytes': path.stat().st_size} for path in root.rglob('*stderr.log') if not path.name.startswith('generated-')]
check(all(row['bytes'] == 0 for row in logs), 'all captured native and tool stderr files empty')
reuse = collections.Counter(tuple(row['identity']) for row in owned_lifetimes)
report = {'generated': True, 'errors': errors, 'checks': dict(checks), 'raw_bytes': len(raw), 'raw_lines': len(records), 'raw_sha256': hashlib.sha256(raw).hexdigest(), 'hashes': hashes, 'native_stderr': logs, 'calls_by_flags': dict(flags), 'native_call_count': sum(flags.values()), 'call_duration_us': [min(call_durations), max(call_durations)], 'settling_read_span_us': [min(settling_spans), max(settling_spans)], 'settling_gap_us': [min(settling_gaps), max(settling_gaps)], 'snapshot_duration_us': [min(snapshot_durations), max(snapshot_durations)], 'invalid_native_snapshots': invalid_native, 'outside_trial_calls': outside_calls, 'owned_lifetimes': owned_lifetimes, 'reused_owned_identity_tuples': [{'identity': key, 'lifetimes': count} for key, count in reuse.items() if count > 1], 'inventories': inventories, 'trials': outputs}
matrix = []
for number, case in enumerate(cases, 1):
    row = {'number': number, 'case': case, 'environments': {}}
    for environment in ('ordinary', 'private'):
        selected = [trial for trial in outputs if trial['number'] == number and trial['environment'] == environment]
        counts = collections.Counter(trial['status'] for trial in selected)
        outcomes = collections.Counter(json.dumps({'before_bands': trial['stages']['before']['bands'], 'before_fixture_order': trial['stages']['before']['order'], 'endpoint_bands': trial['stages'].get('settling_20', {}).get('bands'), 'endpoint_fixture_order': trial['stages'].get('settling_20', {}).get('order')}, sort_keys=True) for trial in selected)
        row['environments'][environment] = {'scheduled': len(selected), 'not_scheduled': not selected, 'setup_not_run': counts['setup_not_run'], 'protocol_invalid': counts['protocol_invalid'], 'measured_operation_failed_prefix': counts['measured_operation_failed_prefix'], 'question_observed': sum(trial['question_observed'] for trial in selected), 'reference_expectation_contradicted': sum(trial['reference_expectation_contradicted'] for trial in selected), 'outcomes': [{'count': count, **json.loads(outcome)} for outcome, count in outcomes.items()], 'raw_trial_begin_lines': [trial['begin_line'] for trial in selected]}
    matrix.append(row)
(root / 'generated-matrix.json').write_text(json.dumps({'generated': True, 'matrix': matrix}, indent=2) + '\n')
(root / 'generated-independent-audit.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps({key: value for key, value in report.items() if key not in ('checks', 'trials', 'owned_lifetimes', 'inventories', 'native_stderr', 'hashes')}, indent=2))
raise SystemExit(2 if errors else 0)
