import json
import pathlib
import collections
p = pathlib.Path(__file__).parent
records = [(i, json.loads(line)) for i, line in enumerate((p / 'raw.jsonl').open(), 1)]
trials = []
current = None
for line, wrapper in records:
    row = wrapper['record']
    if row['kind'] == 'trial_begin':
        current = dict(row, rows=[])
        trials.append(current)
    if current is not None:
        current['rows'].append((line, row))
    if row['kind'] == 'trial_end':
        current = None
cases = records[0][1]['record']['case_names']
for line, wrapper in records:
    row = wrapper['record']
    if row['kind'] in ('environment_ready', 'environment_cleanup', 'environment_end', 'overlay_identity', 'creation_inventory'):
        print(line, json.dumps(row))
for trial in trials:
    number = cases.index(trial['case']) + 1
    if number < 22 or trial['repetition'] != 0:
        continue
    print('\nCASE', number, trial['environment'], trial['repetition'])
    for line, row in trial['rows']:
        if row['kind'] == 'settling' and row['sample'] not in (0, 16, 20):
            continue
        if 'snapshot' in row:
            snapshot = row['snapshot']
            row = dict(kind=row['kind'], sample=row.get('sample'), fixtures=snapshot['fixtures'], inventory=snapshot.get('fixture_inventory'))
        print(line, json.dumps(row))
print('COUNTS', collections.Counter(wrapper['record']['kind'] for _, wrapper in records))
