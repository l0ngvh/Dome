import collections
import hashlib
import json
import pathlib

root = pathlib.Path(__file__).parent
records = [(line, json.loads(text)['record']) for line, text in enumerate((root / 'raw.jsonl').open(), 1)]
session = records[0][1]
cases = session['case_names']
analysis = json.loads((root / 'analysis.json').read_text())
trials = []
active = None
for line, row in records:
    if row['kind'] == 'trial_begin':
        active = dict(row, rows=[])
        trials.append(active)
    if active is not None:
        active['rows'].append((line, row))
    if row['kind'] == 'trial_end':
        active = None

def check_snapshot(snapshot):
    assert snapshot['started_us'] <= snapshot['finished_us'], 'snapshot time'
    fixtures = snapshot['fixtures']
    assert len(set(item['handle'] for item in fixtures)) == len(fixtures), 'duplicate fixture'
    for item in fixtures:
        assert item['valid'] and item['visible'] and item['style_read_error'] is None, 'fixture validity'
        assert item['topmost'] == bool(item['extended_style'] & 8), 'flag decoding'
    if snapshot['scope'] == 'local':
        assert fixtures == snapshot['fixture_rereads'], 'fixture changed across traversal'
        neighbors = snapshot['neighbors']
    else:
        assert snapshot['enumeration_ok'], 'global enumeration'
        neighbors = snapshot['raw_enumeration']
    graph = {item['handle']: item for item in neighbors}
    assert len(graph) == len(neighbors) and 0 not in graph, 'duplicate graph handle'
    for item in neighbors:
        assert item['valid'] and item['process'] and item['thread'], 'neighbor identity'
        if 'topmost' in item:
            assert item['style_read_error'] is None and item['topmost'] == bool(item['extended_style'] & 8), 'neighbor flag'
        for direction, reverse in [('above', 'below'), ('below', 'above')]:
            if item[direction] in graph:
                assert graph[item[direction]][reverse] == item['handle'], 'nonreciprocal links'
        seen = set()
        current = item['handle']
        while current in graph:
            assert current not in seen, 'cycle'
            seen.add(current)
            current = graph[current]['below']
    for item in fixtures:
        assert all(graph[item['handle']][key] == value for key, value in item.items() if key in graph[item['handle']]), 'fixture graph differs'
        assert all(item[direction] == 0 or item[direction] in graph for direction in ('above', 'below')), 'missing neighbor'
    if snapshot['scope'] == 'global':
        heads = [item for item in neighbors if not item['above']]
        assert len(heads) == 1, 'global head'
        order = []
        current = heads[0]['handle']
        while current:
            assert current in graph, 'missing global neighbor'
            order.append(current)
            current = graph[current]['below']
        assert len(order) == len(graph), 'global coverage'
    return graph

def above(graph, upper, lower):
    current = upper
    while current in graph:
        current = graph[current]['below']
        if current == lower:
            return True
    return False

def shape(snapshot, slots):
    fixtures = snapshot['fixtures']
    names = {item['handle']: slot for item, slot in zip(fixtures, slots)}
    def name(handle):
        return names.get(handle, str(handle))
    graph = check_snapshot(snapshot)
    return {'bands': ''.join('T' if item['topmost'] else 'N' for item in fixtures),
            'neighbors': {slot: [name(item['above']), name(item['below'])] for slot, item in zip(slots, fixtures)},
            'order': sorted(slots, key=lambda slot: sum(above(graph, other['handle'], fixtures[slots.index(slot)]['handle']) for other in fixtures)),
            'global_chain': [name(item['handle']) + ('T' if item['topmost'] else 'N') for item in sorted(graph.values(), key=lambda item: sum(above(graph, other, item['handle']) for other in graph))] if snapshot['scope'] == 'global' else None}

invalid = []
for line, row in records:
    if 'snapshot' in row:
        try:
            check_snapshot(row['snapshot'])
        except AssertionError as error:
            invalid.append(dict(line=line, kind=row['kind'], sample=row.get('sample'), reason=str(error), native_reason=row['snapshot'].get('invalid_reason')))

summaries = []
all_calls = []
all_gaps = []
all_spans = []
all_reads = []
for trial in trials:
    rows = trial['rows']
    number = cases.index(trial['case']) + 1
    slots = trial['fixture_slots']
    grouped = collections.defaultdict(list)
    for line, row in rows:
        grouped[row['kind']].append((line, row))
    def single(kind):
        assert len(grouped[kind]) == 1, kind
        return grouped[kind][0][1]['snapshot']
    before = single('before')
    fixtures = before['fixtures']
    names = {item['handle']: slot for slot, item in zip(slots, fixtures)}
    names.update({-1: 'Topmost', -2: 'NotTopmost', 1: 'Bottom'})
    identities = [(item['handle'], item['process'], item['thread']) for item in fixtures]
    baseline = fixtures[:4] if 'O' in slots else fixtures
    assert len(set(item['process'] for item in baseline)) == len(baseline)
    assert len(set(item['thread'] for item in baseline)) == len(baseline)
    assert all(not item['owner'] for item in baseline)
    if 'O' in slots:
        owner = fixtures[slots.index(trial['owned_by'])]
        owned = fixtures[4]
        assert owned['owner'] == owner['handle']
        assert (owned['process'], owned['thread']) == (owner['process'], owner['thread'])
    calls = {}
    call_lines = collections.defaultdict(list)
    for line, row in rows:
        call = row.get('call', row.get('reply', {}).get('record', {}).get('call'))
        if not call:
            continue
        identity = (call['calling_process'], call['calling_thread'], call['started_us'])
        assert identity not in calls or calls[identity] == call
        calls[identity] = call
        call_lines[identity].append(line)
        assert call['ok'] and call['error'] is None and call['last_error_on_failure'] is None
        assert call['api'] == 'SetWindowPos' and call['flags'] in (19, 16403)
        assert call['started_us'] <= call['returned_us']
        assert all(call[key] == 0 for key in ('x', 'y', 'width', 'height'))
        target = next(item for item in fixtures if item['handle'] == call['window'])
        caller = fixtures[3] if call['flags'] == 16403 else target
        assert (caller['process'], caller['thread']) == (call['calling_process'], call['calling_thread'])
    calls = sorted(calls.values(), key=lambda call: call['started_us'])
    all_calls.extend(calls)
    endpoints = {}
    for kind in ('first_settling', 'settling'):
        samples = grouped[kind]
        if not samples:
            continue
        assert [row['sample'] for _, row in samples] == list(range(21))
        snapshots = [row['snapshot'] for _, row in samples]
        gaps = [right['started_us'] - left['finished_us'] for left, right in zip(snapshots, snapshots[1:])]
        assert min(gaps) >= 25000
        assert snapshots[-1]['started_us'] - snapshots[0]['started_us'] >= 500000
        all_gaps.extend(gaps)
        all_spans.append(snapshots[-1]['finished_us'] - snapshots[0]['started_us'])
        all_reads.extend(s['finished_us'] - s['started_us'] for s in snapshots)
        for snapshot in snapshots[-5:]:
            check_snapshot(snapshot)
            assert [(item['handle'], item['process'], item['thread']) for item in snapshot['fixtures']] == identities
        plateau = [tuple((item['topmost'], item['above'], item['below'], item['owner']) for item in s['fixtures']) for s in snapshots[-5:]]
        assert len(set(plateau)) == 1
        endpoints[kind] = dict(lines=[line for line, _ in samples[-5:]], shape=shape(snapshots[-1], slots))
    endpoint = single('bottom_global_endpoint') if grouped['bottom_global_endpoint'] else grouped['settling'][-1][1]['snapshot'] if grouped['settling'] else before
    measured = [call for call in calls if call['returned_us'] >= before['finished_us'] and call['started_us'] <= endpoint['finished_us']]
    expected = []
    def allow(window, after, flags, preceding, following):
        expected.append((window, after, flags, single(preceding)['finished_us'], single(following)['started_us']))
    if number not in (23, 25):
        if number in (5, 13, 14, 15):
            allow('A', 'NotTopmost' if number in (5, 13) else 'Topmost', 16403, 'held_before_post', 'after_async_post_while_held')
        if number in (16, 17, 18):
            allow('C', 'A', 16403, 'queried_normal_a_before_c_post', 'after_async_post_while_held')
            if number in (16, 18):
                allow('A', 'Topmost', 19, 'after_async_post_while_held', 'after_a_promotion_while_c_held')
            if number == 18:
                allow('A', 'NotTopmost', 19, 'after_a_promotion_while_c_held', 'after_a_demotion_while_c_held')
        else:
            if number in (2, 10):
                allow('X', 'NotTopmost', 19, 'before', 'after_explicit_notopmost')
            if number in (3, 26):
                allow('X', 'Topmost', 19, 'before', 'after_explicit_topmost')
            target = 'A' if number in (19, 20, 21, 27) else 'O' if number == 24 else 'X'
            reference = 'P' if number in (4, 11, 20, 21) else 'X' if number in (19, 27) else 'Topmost' if number in (22, 24) else 'Bottom' if number in (28, 29) else 'A'
            allow(target, reference, 19, 'before_insert', 'immediate_after_local_write')
            if number == 12:
                allow('X', 'A', 19, 'before_repeated_insert', 'immediate_after_repeated_write')
    assert len(expected) == len(measured), (number, expected, measured)
    previous = before['finished_us']
    for call, wanted in zip(measured, expected):
        assert (names[call['window']], names[call['after']], call['flags']) == wanted[:3]
        assert max(previous, wanted[3]) <= call['started_us'] <= call['returned_us'] <= wanted[4]
        previous = call['returned_us']
    race = None
    if number in (5, 13, 14, 15, 16, 17, 18):
        held_slot = 'C' if number in (16, 17, 18) else 'A'
        held_fixture = fixtures[slots.index(held_slot)]
        replies = [row['reply'] for _, row in rows if row['kind'] == 'fixture_reply' and row['process'] == held_fixture['process']]
        held = [r for r in replies if r['record']['kind'] == 'held']
        released = [r for r in replies if r['record']['kind'] == 'released_before_pump']
        assert len(held) == len(released) == 1
        hold_time = held[0]['unix_us']
        release_time = released[0]['unix_us']
        assert 0 < release_time - hold_time < 3000000
        post = next(call for call in measured if call['flags'] == 16403)
        assert hold_time <= post['started_us'] <= post['returned_us'] <= release_time
        if number in (16, 17, 18):
            queried = single('queried_normal_a_before_c_post')
            prerelease = single('before_c_release')
            assert not queried['fixtures'][0]['topmost']
            assert prerelease['finished_us'] <= release_time <= single('immediate_after_release')['started_us']
            assert all(call['started_us'] < release_time for call in measured)
            assert not any(call['window'] == fixtures[2]['handle'] and call['started_us'] >= queried['finished_us'] for call in calls)
            start_graph = check_snapshot(queried)
            release_graph = check_snapshot(prerelease)
            final_graph = check_snapshot(endpoint)
            a, _, predecessor, overlay, other = fixtures
            assert above(start_graph, predecessor['handle'], other['handle'])
            assert above(release_graph, predecessor['handle'], other['handle'])
            assert above(final_graph, other['handle'], predecessor['handle'])
            assert endpoint['fixtures'][4]['above'] == a['handle']
            assert prerelease['fixtures'][4]['above'] != a['handle']
            assert not prerelease['fixtures'][4]['topmost']
            race = dict(hold_span_us=release_time-hold_time, release_us=release_time, release_line=next(line for line, row in rows if row.get('reply') == released[0]),
                        queried=shape(queried, slots), prerelease=shape(prerelease, slots), crossed_stationary_P=True)
        else:
            race = dict(hold_span_us=release_time-hold_time, release_us=release_time)
    inventory = before.get('fixture_process_classes', [])
    unexpected = [item for item in inventory if item['window']['handle'] not in {f['handle'] for f in fixtures}]
    sample_shapes = {}
    for kind in ('setup_x', 'after_explicit_topmost', 'before_insert', 'immediate_after_local_write', 'immediate_after_release', 'bottom_global_endpoint'):
        if grouped[kind]:
            try:
                sample_shapes[kind] = shape(single(kind), slots)
            except AssertionError as error:
                sample_shapes[kind] = {'invalid': str(error)}
    summaries.append(dict(case_number=number, environment=trial['environment'], repetition=trial['repetition'], slots=slots,
        lines={kind: [line for line, _ in group] for kind, group in grouped.items() if group and kind not in ('settling', 'first_settling')},
        before=shape(before, slots), stages=sample_shapes, endpoints=endpoints, race=race,
        native_calls=[dict(line=call_lines[(call['calling_process'], call['calling_thread'], call['started_us'])], window=names[call['window']], after=names[call['after']], flags=call['flags'], started_us=call['started_us'], returned_us=call['returned_us']) for call in calls],
        measured_call_count=len(measured), ownership=[dict(slot=slot, **{k:item[k] for k in ('handle','owner','process','thread')}) for slot, item in zip(slots, fixtures)],
        unexpected_fixture_process_windows=unexpected))

schedule = [(row['environment'], row['case'], row['repetition']) for row in session['schedule']]
actual = [(row['environment'], row['case'], row['repetition']) for row in trials]
assert actual == schedule and len(set(actual)) == 165
ready_records = []
current_environment = None
for line, row in records:
    if row['kind'] == 'environment_ready':
        current_environment = row['environment']
        ready_records.append(dict(line=line, **row))
    if row['kind'] == 'fixture_reply' and row['reply']['record']['kind'] == 'ready':
        ready_records.append(dict(line=line, environment=current_environment, **row['reply']['record']))
for row in ready_records:
    desktop = row['desktop']
    method = row['input_method']
    assert desktop['actual']['name'] == desktop['requested']
    assert desktop['verified'] and not desktop['switch_requested'] and not (desktop['requested_open_access'] & 256)
    assert method['ok'] and method['argument'] == 0 and method['before_first_window']
    assert (method['process'], method['thread']) == (row['process'], row['thread'])
    assert (desktop['process'], desktop['thread']) == (row['process'], row['thread'])
    assert (desktop['requested'] == 'Default') == (row['environment'] == 'ordinary')
original_files = [path for path in root.rglob('*') if path.is_file() and '.generated.' not in path.name and path.name != 'analysis.json']
original_inventory = [dict(file=str(path.relative_to(root)), bytes=path.stat().st_size, sha256=hashlib.sha256(path.read_bytes()).hexdigest()) for path in sorted(original_files)]
report = dict(raw_sha256=hashlib.sha256((root/'raw.jsonl').read_bytes()).hexdigest(),
    schedule_complete=True, original_file_count=len(original_inventory), original_total_bytes=sum(item['bytes'] for item in original_inventory), original_files=original_inventory,
    counters=dict(collections.Counter(row['kind'] for _, row in records)),
    unique_native_call_count=len(all_calls), measured_call_count=sum(row['measured_call_count'] for row in summaries), all_call_schedule_and_timing_checks_passed=True,
    snapshot_count=sum('snapshot' in row for _,row in records), invalid_snapshots=invalid,
    timing=dict(settling_groups=len(all_spans), gap_min_us=min(all_gaps), gap_max_us=max(all_gaps), span_min_us=min(all_spans), span_max_us=max(all_spans), read_min_us=min(all_reads), read_max_us=max(all_reads)),
    ready_records=ready_records, trials=summaries)
(root/'verification.generated.json').write_text(json.dumps(report, indent=2)+'\n')
print(json.dumps({key:value for key,value in report.items() if key not in ('trials','original_files','ready_records')}, indent=2))
for number in range(1,30):
    for environment in ('ordinary','private'):
        rows = [row for row in summaries if row['case_number']==number and row['environment']==environment]
        if not rows:
            continue
        shapes=collections.Counter(json.dumps(row['endpoints'].get('settling', {}).get('shape'),sort_keys=True) for row in rows)
        print(number,environment, 'REP0', rows[0]['before'], 'ENDPOINTS',dict(shapes), 'ANCHORS', {key:rows[0]['lines'].get(key) for key in ('before','setup_eligibility','before_insert','treatment_call','trial_end')})
