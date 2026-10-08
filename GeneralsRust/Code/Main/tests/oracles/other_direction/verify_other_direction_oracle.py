#!/usr/bin/env python3
"""Verify frozen OTHER extraction and intentionally faulty direction controls."""
import argparse, hashlib, json, pathlib, struct
ROOT = pathlib.Path(__file__).resolve().parent
FROZEN_SHA256 = '9be3c8eb2e4017e44a285973f7059c2e4c6514fa6d416532cd1dd80459587734'
NAMES = ['axis_nonzero','turn_90','turn_30','reflect_90','reflect_30','entry_reflect_90',
         'slide_close','slide_close_flag_off','slide_far']
LANES = ['raw-frame','host-impulse']
VARIANTS = ['original','mutation_late_normal_direction','mutation_lost_slide_override']
def value(bits):
    return struct.unpack('!f',bytes.fromhex(bits))[0]
def vector(row, start):
    return [value(x) for x in row[start:start+3]]
def read(path):
    rows = {}
    for line in path.read_text().splitlines():
        if not line or line.startswith('#'):
            continue
        row = line.split()
        assert len(row)==47,(path,len(row))
        key = tuple(row[:2])
        assert key not in rows
        rows[key]=row
    assert set(rows)=={(lane,name) for lane in LANES for name in NAMES}
    return rows
def main():
    p = argparse.ArgumentParser()
    p.add_argument('--run',type=pathlib.Path,default=ROOT/'run-original')
    a = p.parse_args()
    frozen = (ROOT.parent.parent/'fixtures/other_direction_original.txt').read_bytes()
    assert hashlib.sha256(frozen).hexdigest()==FROZEN_SHA256,'frozen fixture SHA256 mismatch'
    assert (a.run/'original.txt').read_bytes()==frozen,'frozen original output drift'
    data = {name:read(a.run/(name+'.txt')) for name in VARIANTS}
    original = data['original']
    changes = {name:[] for name in VARIANTS[1:]}
    for key, row in original.items():
        assert vector(row,2)==[0,0,0],'nonzero velocity is outside direction-only selection'
        assert value(row[23])==value(row[24])==0,'zero-speed boundary must remain zero'
        assert value(row[19])>value(row[20])>0
        assert value(row[20])==value(row[21]),'same positive acceleration and braking'
        assert value(row[22])==1
        assert value(row[27])>0
        assert int(row[42])==(0 if key[1]=='slide_close' else 1)
        assert list(map(int,row[43:]))==[1,0,2,110],'turning setter, force and motive reached'
        assert row[12]==row[28] and row[16:19]==row[29:32],'supplied rotation boundary'
        assert row[32:35]==row[35:38],'zero velocity plus sole motive impulse'
        assert sum(x*x for x in vector(row,32))>0,'positive nonzero force reachability'
        for name in VARIANTS[1:]:
            other = data[name][key]
            # Only force and force-derived velocity/displacement can change.
            assert row[:32]==other[:32] and row[41:]==other[41:]
            if row[32:35]!=other[32:35]:
                changes[name].append('/'.join(key))
    expected_late = {f'{lane}/{name}' for lane in LANES for name in NAMES
                     if name not in ['axis_nonzero','slide_close']}
    assert set(changes[VARIANTS[1]])==expected_late
    assert set(changes[VARIANTS[2]])=={f'{lane}/slide_close' for lane in LANES}
    for lane in LANES:
        axis = original[lane,'axis_nonzero']
        assert vector(axis,32)[0]>0 and vector(axis,32)[1:]==[0,0]
        assert all(data[name][lane,'axis_nonzero']==axis for name in VARIANTS)
        for name in ['turn_90','turn_30','reflect_90','reflect_30','slide_close_flag_off','slide_far']:
            row = original[lane,name]
            assert row[32:35]==axis[32:35],'cached +X entry independent of supplied turn'
        assert original[lane,'turn_90'][32:35]==original[lane,'reflect_90'][32:35]
        assert original[lane,'turn_30'][32:35]==original[lane,'reflect_30'][32:35]
        reflected = original[lane,'entry_reflect_90']
        assert vector(reflected,32)[1]>0,'entry direction is not hard-coded +X'
        assert abs(vector(reflected,32)[0])<1e-5
        slide = original[lane,'slide_close']
        assert slide[41:43]==['1','0'],'slide must skip rotation stub'
        assert vector(slide,32)[0]>0>vector(slide,32)[1]
        assert abs(vector(slide,32)[0]+vector(slide,32)[1])==0
        assert slide==data[VARIANTS[1]][lane,'slide_close'],'normal direction repair must retain slide'
        assert data[VARIANTS[2]][lane,'slide_close'][32:35]==axis[32:35]
    details = []
    for name in NAMES:
        row = original['host-impulse',name]
        impulse = vector(row,32)
        late = vector(data[VARIANTS[1]]['host-impulse',name],32)
        displacement = vector(row,38)
        details.append({'case':name,'original_impulse_bits':row[32:35],
                        'adapted_host_impulse':[impulse[0],impulse[2],-impulse[1]],
                        'late_direction_diagnostic_host_impulse':[late[0],late[2],-late[1]],
                        'adapter_host_displacement':[displacement[0],displacement[2],-displacement[1]]})
    summary = {'status':'passed','original_rows':18,'source_body':'unchanged Locomotor.cpp:2326-2404',
               'frozen_original_sha256':FROZEN_SHA256,'changed_force_rows':changes,
               'controls':'Nonzero axis; 90/30 degree supplied turns and reflections; different entry direction; close-slide, flag-off and far-slide admission controls',
               'scope':'Zero velocity direction-only extraction. No Main execution, original rotation execution, friction trajectory, or nonzero-speed parity claim.',
               'host_rows':details}
    (a.run/'control-verification.json').write_text(json.dumps(summary,indent=2)+'\n')
    print(json.dumps({k:v for k,v in summary.items() if k!='host_rows'}))
if __name__=='__main__':
    main()
