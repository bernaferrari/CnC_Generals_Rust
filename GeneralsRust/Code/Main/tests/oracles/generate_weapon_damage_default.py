#!/usr/bin/env python3
"""Execute only exact original enum/default/getter/copy statements in a tiny harness.
Not the original engine, INI parser, damage-delivery code, armor, or save protocol.
"""
import argparse,hashlib,json,re
from pathlib import Path
parser=argparse.ArgumentParser()
parser.add_argument('--root',type=Path,default=Path('/workspace/shared/generals-reconstruction-20261006/worktrees/common-parity'))
parser.add_argument('--out',type=Path,default=Path(__file__).resolve().parent)
args=parser.parse_args()
root=args.root
out=args.out
out.mkdir(parents=True,exist_ok=True)
paths={'cpp':'GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Weapon.cpp','weapon':'GeneralsMD/Code/GameEngine/Include/GameLogic/Weapon.h','damage':'GeneralsMD/Code/GameEngine/Include/GameLogic/Damage.h'}
pins={'cpp':'1e0cd5d73d6a0640e5231b129b98413ee953cfd7fbeb9e6119e783c2317978c8','weapon':'5cf2471d7960e606788e52744f2932571a9319b439ec07d1e70d8cd02ba4a3d2','damage':'7d7d7ba2718b856ae510640fb6a3db84729844849b670df927dc9ffeed8a0385'}
for key,path in paths.items():
 assert hashlib.sha256((root/path).read_bytes()).hexdigest()==pins[key],f'source pin drift: {path}'
src={k:(root/v).read_text() for k,v in paths.items()}
extracted=[]
def capture(key,pattern,start=1,end=None):
 lines=src[key].splitlines(keepends=True)
 offset=len(''.join(lines[:start-1]))
 region=''.join(lines[start-1:end])
 matches=list(re.finditer(pattern,region,re.M|re.S))
 assert len(matches)==1,(pattern,start,end,len(matches))
 m=matches[0]
 text=m.group(0)
 extracted.append({'path':paths[key],'source_sha256':hashlib.sha256((root/paths[key]).read_bytes()).hexdigest(),'start_line':src[key][:offset+m.start()].count('\n')+1,'end_line':src[key][:offset+m.end()].count('\n')+1,'text':text})
 return text
ed=capture('damage',r'enum DamageType\s*\{.*?\n\};')
et=capture('damage',r'enum DeathType\s*\{.*?\n\};')
assign=[]
for field in ['primaryDamageRadius','damageType','deathType','weaponSpeed']:
 assign.append(capture('cpp',rf'^[ \t]*m_{field}[^\n]*;[^\n]*',231,308))
getters=[capture('weapon',r'^\s*inline DamageType getDamageType\(\) const \{ return m_damageType; \}'),capture('weapon',r'^\s*inline DeathType getDeathType\(\) const \{ return m_deathType; \}')]
copy=[capture('cpp',r'^[ \t]*DamageType damageType = getDamageType\(\);',1255,1270),capture('cpp',r'^[ \t]*DeathType deathType = getDeathType\(\);',1255,1270),capture('cpp',r'^[ \t]*damageInfo.in.m_damageType = damageType;',1375,1385),capture('cpp',r'^[ \t]*damageInfo.in.m_deathType = deathType;',1375,1385)]
program='// PARTIAL SOURCE EXTRACTION; harness substitutes engine surroundings.\n#include <cstdio>\n'+ed+'\n'+et+'\n'
program+='struct DamageInfo { struct { DamageType m_damageType; DeathType m_deathType; } in; };\n'
program+='struct WeaponTemplate { float m_primaryDamageRadius, m_weaponSpeed; DamageType m_damageType; DeathType m_deathType; WeaponTemplate() {\n'+'\n'.join(assign)+'\n}\n'+'\n'.join(getters)+'\nDamageInfo copied() const { DamageInfo damageInfo;\n'+'\n'.join(copy)+'\nreturn damageInfo; } };\n'
program+='int main() { for (float speed : {0.0f, 999999.0f, 2.0f}) { for (float radius : {0.0f, 4.0f}) { WeaponTemplate w; w.m_weaponSpeed=speed; w.m_primaryDamageRadius=radius; auto d=w.copied(); std::printf("%.0f %.0f %d %d\\n", speed,radius,int(d.in.m_damageType),int(d.in.m_deathType)); } } }\n'
program=program.replace('#include <cstdio>','#include <cstdio>\n#include <initializer_list>')
(out/'weapon_default_original.cpp').write_text(program)
(out/'extraction.json').write_text(json.dumps({'limit':'Partial original-source extraction of defaults and type copy; no complete constructor, parser, runtime, armor or save comparison.','extracted':extracted},indent=2)+'\n')
print('extracted',len(extracted),'source spans')
