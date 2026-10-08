#!/usr/bin/env python3
"""Verify diagnostic controls; these are scalar checks, not Main runtime tests."""
import argparse,json,pathlib,struct,hashlib
ROOT=pathlib.Path(__file__).resolve().parent
FLOAT= lambda s: struct.unpack('!f',bytes.fromhex(s))[0]
def read(path):
 rows={}
 for line in path.read_text().splitlines():
  if line.startswith('#'):continue
  row=line.split()
  if not row:continue
  assert len(row)==30,(path,row)
  key=tuple(row[:2]);assert key not in rows
  rows[key]=row
 assert len(rows)==24,(path,len(rows))
 return rows

def main():
 p=argparse.ArgumentParser();p.add_argument('--run',type=pathlib.Path,default=ROOT/'run-1');a=p.parse_args()
 names=['original','mutation-dot-speed','mutation-late-sample']
 data={name:read(a.run/(name+'.txt')) for name in names}
 original=data['original'];dot=data[names[1]];late=data[names[2]]
 assert original.keys()==dot.keys()==late.keys()
 def impulse(row):return [FLOAT(s) for s in row[21:24]]
 def projection(row):return sum(FLOAT(row[21+i])*FLOAT(row[11+i]) for i in range(3))
 changed={name:[] for name in names[1:]}
 details=[]
 for key,row in original.items():
  assert int(row[27])==1,'original rotation stub must be called'
  assert int(row[29])==110,'motive window must remain armed'
  assert int(row[28])==(0 if key[1]=='zero_gap' else 2),'force block reached'
  for name in names[1:]:
   if row[21:24]!=data[name][key][21:24]:changed[name].append('/'.join(key))
  details.append({'lane':key[0],'case':key[1],'original_impulse_bits':row[21:24],'original_impulse':impulse(row),'original_projection_onto_supplied_post_direction':projection(row),'dot_projection':projection(dot[key]),'late_projection':projection(late[key])})
 assert len(changed[names[1]])==10,changed
 assert len(changed[names[2]])==4,changed
 for lane in ['raw-frame','host-impulse']:
  axis=(lane,'axis_accel');assert original[axis]==dot[axis]==late[axis]
  assert impulse(original[axis])[0]>0
  for case in ['diagonal_sign','diagonal_reflect','vertical_control']:
   key=(lane,case);assert impulse(original[key])[0]>0>impulse(dot[key])[0]
  key=(lane,'component_cancellation');assert impulse(original[key])[0]<0<impulse(dot[key])[0]
  for case in ['turn_snapshot','turn_limited']:
   key=(lane,case);assert abs(projection(original[key]))>abs(projection(late[key]))
  assert original[(lane,'vertical_control')][18]==original[(lane,'diagonal_sign')][18]
  assert original[(lane,'vertical_control')][21:24]==original[(lane,'diagonal_sign')][21:24]
  assert original[(lane,'vertical_control')][26]==original[(lane,'vertical_control')][4]
 expected={'positive_brake_below':-.5,'positive_brake_equal':-1,'positive_brake_above':-1,'negative_brake_below':.5,'negative_brake_equal':1,'negative_brake_above':-1,'zero_gap':0,'zero_braking':0}
 for case,value in expected.items():assert impulse(original['exact-frame',case])==[value,0,0]
 frozen=(ROOT.parent.parent/'fixtures/legs_force_original.txt').read_bytes()
 assert (a.run/'original.txt').read_bytes()==frozen,'frozen original output drift'
 summary={'status':'passed','original_rows':24,'diagnostic_mutations':2,'changed_force_rows':changed,'positive_axis_control':'unchanged and nonzero in both lanes','vertical_control':'same planar force and preserved vertical input','exact_frame_controls':expected,'frozen_original_sha256':hashlib.sha256(frozen).hexdigest(),'projection_note':'Approximate diagnostic projection calculated in Python double from frozen f32 components; not an extra extracted original scalar.','rows':details}
 (a.run/'control-verification.json').write_text(json.dumps(summary,indent=2)+'\n')
 print(json.dumps({k:v for k,v in summary.items() if k!='rows'}))
if __name__=='__main__':main()
