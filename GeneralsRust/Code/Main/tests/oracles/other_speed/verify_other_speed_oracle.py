#!/usr/bin/env python3
"""Verify orthogonal original OTHER metric and sampling-time witnesses."""
import argparse, hashlib, json, pathlib, struct
ROOT=pathlib.Path(__file__).resolve().parent
FROZEN_SHA256='8d1119310807f7d3abb1a781059888690629f5ba01e87cc6febf778b778188ef'
NAMES=['axis_accel','metric_diagonal','metric_reflection','cancellation','negative_diagonal',
       'negative_axis_control','timing_turn30','timing_reflect30','timing_turn90',
       'timing_negative_turn30','masking_diagonal_turn','zero_turn_control','slide_zero_control']
LANES=['raw-frame','host-impulse']
VARIANTS=['original','mutation_entry_dot_metric','mutation_late_signed_sample','mutation_late_dot_both']
def value(bits):return struct.unpack('!f',bytes.fromhex(bits))[0]
def vector(row,start):return [value(x) for x in row[start:start+3]]
def read(path):
    rows={}
    for line in path.read_text().splitlines():
        if not line or line.startswith('#'):continue
        row=line.split();assert len(row)==47,(path,len(row))
        key=tuple(row[:2]);assert key not in rows;rows[key]=row
    assert set(rows)=={(lane,name) for lane in LANES for name in NAMES}
    return rows
def projection(row):
    return sum(a*b for a,b in zip(vector(row,32),vector(row,13)))
def main():
    p=argparse.ArgumentParser();p.add_argument('--run',type=pathlib.Path,default=ROOT/'run-original');a=p.parse_args()
    frozen=(ROOT/'../../fixtures/other_speed_original.txt').read_bytes()
    assert hashlib.sha256(frozen).hexdigest()==FROZEN_SHA256,'frozen fixture SHA256 mismatch'
    assert (a.run/'original.txt').read_bytes()==frozen,'frozen original output drift'
    data={name:read(a.run/(name+'.txt')) for name in VARIANTS};original=data['original']
    changes={name:[] for name in VARIANTS[1:]}
    approach_margin={name:float('inf') for name in VARIANTS}
    for key,row in original.items():
        assert value(row[19])>0 and value(row[20])>0 and value(row[21])>0
        assert value(row[22])==1 and value(row[27])>0
        assert int(row[42])==(0 if key[1]=='slide_zero_control' else 1)
        assert list(map(int,row[43:]))==[1,0,2,110]
        assert row[12]==row[28] and row[16:19]==row[29:32]
        for name in VARIANTS:
            other=data[name][key]
            assert row[:23]==other[:23] and row[25:32]==other[25:32] and row[41:]==other[41:]
            force=vector(other,32);entry=vector(other,13)
            assert sum(v*v for v in force)>0,'nonzero impulse required'
            assert force[2]==0
            if key[1]!='slide_zero_control':
                cross=force[0]*entry[1]-force[1]*entry[0]
                assert abs(cross)<1e-5,'direction fix must remain frozen in every variant'
            # Diagnostic guard only, using Python double on recorded f32 scalars.
            # It establishes a large inactive margin, not exact C++ slowdown bits.
            speed_index=24 if name in ['mutation_late_signed_sample','mutation_late_dot_both'] and key[1]!='slide_zero_control' else 23
            speed=value(other[speed_index]);slow=max(speed,0)**2/abs(value(other[21]))*.5*1.05
            margin=value(other[27])-slow
            assert margin>10,'approach could change the selected goal'
            approach_margin[name]=min(approach_margin[name],margin)
            if name!='original' and row[32:35]!=other[32:35]:changes[name].append('/'.join(key))
    metric={'metric_diagonal','metric_reflection','cancellation','negative_diagonal','masking_diagonal_turn'}
    timing={'timing_turn30','timing_reflect30','timing_turn90','timing_negative_turn30'}
    expected=[metric,timing,(metric-{'masking_diagonal_turn'})|timing]
    for name,cases in zip(VARIANTS[1:],expected):
        assert set(changes[name])=={f'{lane}/{case}' for lane in LANES for case in cases}
    for lane in LANES:
        for case in ['axis_accel','negative_axis_control','zero_turn_control','slide_zero_control']:
            assert all(data[name][lane,case]==original[lane,case] for name in VARIANTS)
        for case in ['metric_diagonal','metric_reflection']:
            key=(lane,case)
            assert projection(original[key])>0>projection(data[VARIANTS[1]][key])
            assert original[key]==data[VARIANTS[2]][key],'no turn isolates speed metric'
        key=(lane,'cancellation')
        assert value(original[key][23])>0 and value(data[VARIANTS[1]][key][23])==0
        assert projection(original[key])<0<projection(data[VARIANTS[1]][key])
        key=(lane,'negative_diagonal')
        assert value(original[key][23])<0 and value(data[VARIANTS[1]][key][23])<value(original[key][23])
        assert 0<projection(original[key])<projection(data[VARIANTS[1]][key])
        for case in ['timing_turn30','timing_reflect30','timing_turn90']:
            key=(lane,case)
            assert original[key]==data[VARIANTS[1]][key],'axis input isolates sampling phase'
            assert projection(original[key])<0<projection(data[VARIANTS[2]][key])
        key=(lane,'timing_negative_turn30')
        assert projection(original[key])>projection(data[VARIANTS[2]][key])>0
        key=(lane,'masking_diagonal_turn')
        assert original[key][32:35]==data[VARIANTS[3]][key][32:35]
        assert projection(original[key])>0>projection(data[VARIANTS[1]][key])
    rows=[]
    for case in NAMES:
        variants={}
        for name in VARIANTS:
            row=data[name]['host-impulse',case];imp=vector(row,32)
            variants[name]={'entry_speed':value(row[23]),'supplied_post_turn_speed':value(row[24]),
                            'impulse_original_bits':row[32:35],'adapted_host_impulse':[imp[0],imp[2],-imp[1]],
                            'diagnostic_projection_onto_entry':projection(row)}
        rows.append({'case':case,'variants':variants})
    result={'status':'passed','original_rows':26,'frozen_original_sha256':FROZEN_SHA256,
            'changed_force_rows':changes,'minimum_diagnostic_approach_margin':approach_margin,
            'direction_scope':'Cached entry direction unchanged in every variant; rested slide direction unchanged',
            'scope':'Original-only scalar extraction. Supplied rotation. No Main runtime or nonzero slide/Hover/Wings claim.',
            'projection_note':'Approximate Python-double projection/margin of frozen f32 values is diagnostic, not additional extracted original arithmetic.',
            'host_rows':rows}
    (a.run/'control-verification.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps({k:v for k,v in result.items() if k!='host_rows'}))
if __name__=='__main__':main()
