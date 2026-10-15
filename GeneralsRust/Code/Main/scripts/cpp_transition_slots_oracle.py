#!/usr/bin/env python3
"""Compile the verbatim original TransitionDamageFX callback with trace probes.

This is a component oracle for callback order, masks, sparse slots and cleanup.
Probe services stand in for engine allocation/geometry/OCL/FX; this does not run
retail content or claim particle attachment, RNG or complete-game equivalence.
"""
from __future__ import annotations
import argparse, hashlib, json, pathlib, subprocess

ROOT = pathlib.Path(__file__).resolve().parents[4]
SOURCE = 'GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Damage/TransitionDamageFX.cpp'

def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()

def callback(source: str) -> str:
    start = source.index('void TransitionDamageFX::onBodyDamageStateChange(')
    opening = source.index('{', start)
    depth = 1
    end = opening + 1
    while depth:
        depth += (source[end] == '{') - (source[end] == '}')
        end += 1
    return source[start:end]

PROBES = r'''
#include <iostream>
#include <cstdlib>
#include <string>
using Int=int; using BodyDamageType=int; using ParticleSystemID=int;
#define IS_CONDITION_WORSE( a, b ) ( a > b )
enum { DAMAGE_MODULE_MAX_FX = 12 };
const int INVALID_PARTICLE_SYSTEM_ID=0; const float INVALID_ANGLE=-999.0f;
struct Coord3D { float x=0,y=0,z=0; };
struct DamageInfo { struct { int m_sourceID=7; int m_damageType=0; } in; };
struct Drawable {};
struct Loc { int slot; const char* kind; };
struct ParticleSystemTemplate {int slot;};
struct F {int slot;}; struct O {int slot;};
void trace(const std::string& text){std::cout<<text<<"\n";}
struct Body {const DamageInfo* last=nullptr; const DamageInfo* getLastDamageInfo(){return last;}};
struct Object {
 Drawable draw; Body body; Coord3D position;
 Drawable* getDrawable(){return &draw;} Body* getBodyModule(){return &body;}
 const Coord3D* getPosition(){return &position;}
 void convertBonePosToWorldPos(Coord3D* in,void*,Coord3D* out,void*){out->x=in->x+position.x;}
};
struct Logic {Object source; Object* findObjectByID(int){return &source;}} logic;
Logic* TheGameLogic=&logic;
struct ParticleSystem {
 int slot; int getSystemID(){return 1000+slot;}
 void setPosition(const Coord3D*){trace("set:"+std::to_string(slot));}
 void attachToObject(Object*){trace("attach:"+std::to_string(slot));}
};
struct Manager {
 unsigned failure=0; ParticleSystem systems[12];
 void destroyParticleSystemByID(int id){trace("destroy:"+std::to_string(id));}
 ParticleSystem* createParticleSystem(const ParticleSystemTemplate* t){trace("create:"+std::to_string(t->slot));if(failure&(1u<<t->slot))return nullptr;systems[t->slot].slot=t->slot;return &systems[t->slot];}
} manager;
Manager* TheParticleSystemManager=&manager;
struct FXList {static void doFXPos(F* fx,const Coord3D*){trace("fx:"+std::to_string(fx->slot));}};
struct ObjectCreationList {static void create(O* o,Object*,const Coord3D*,const Coord3D*,float){trace("ocl:"+std::to_string(o->slot));}};
// Mask service is explicitly injected: the callback's branch ordering is under
// test, not DamageTypeFlags enum mapping (which belongs to Damage.cpp).
bool getDamageTypeFlag(unsigned mask,int type){return (mask&(1u<<type))!=0;}
struct TransitionDamageFXModuleData {
 struct FI {F* fx=nullptr;Loc locInfo;}; struct OI {O* ocl=nullptr;Loc locInfo;};
 struct PI {const ParticleSystemTemplate* particleSysTemplate=nullptr;Loc locInfo;};
 FI m_fxList[3][12]; OI m_OCL[3][12]; PI m_particleSystem[3][12];
 unsigned m_damageFXTypes=1,m_damageOCLTypes=1,m_damageParticleTypes=1;
};
struct TransitionDamageFX {
 Object owner; TransitionDamageFXModuleData data; int m_particleSystemID[3][12]{};
 Object* getObject(){return &owner;}
 const TransitionDamageFXModuleData* getTransitionDamageFXModuleData(){return &data;}
 Coord3D getLocalEffectPos(const Loc* loc,Drawable*){trace(std::string("local:")+loc->kind+":"+std::to_string(loc->slot));return Coord3D{float(loc->slot),0,0};}
 void onBodyDamageStateChange(const DamageInfo*,BodyDamageType,BodyDamageType);
};
'''
MAIN = r'''
int main(int argc,char** argv){
 if(argc!=12)return 2;
 auto arg=[&](int i){return unsigned(std::strtoul(argv[i],nullptr,10));};
 TransitionDamageFX module; F fx[12];O ocl[12];ParticleSystemTemplate particle[12];
 int oldState=arg(1),newState=arg(2);DamageInfo damage;
 module.owner.body.last=arg(10)?&damage:nullptr;
 module.data.m_damageFXTypes=arg(7);module.data.m_damageOCLTypes=arg(8);module.data.m_damageParticleTypes=arg(9);
 manager.failure=arg(11);
 for(int slot=0;slot<12;++slot){
  fx[slot].slot=ocl[slot].slot=particle[slot].slot=slot;
  if(arg(3)&(1u<<slot))module.data.m_fxList[newState][slot].fx=&fx[slot];
  if(arg(4)&(1u<<slot))module.data.m_OCL[newState][slot].ocl=&ocl[slot];
  if(arg(5)&(1u<<slot))module.data.m_particleSystem[newState][slot].particleSysTemplate=&particle[slot];
  module.data.m_fxList[newState][slot].locInfo={slot,"fx"};
  module.data.m_OCL[newState][slot].locInfo={slot,"ocl"};
  module.data.m_particleSystem[newState][slot].locInfo={slot,"particle"};
  if(arg(6)&(1u<<slot))module.m_particleSystemID[oldState][slot]=100+slot;
 }
 module.onBodyDamageStateChange(&damage,oldState,newState);
 std::cout<<"old_ids:";for(int slot=0;slot<12;++slot)std::cout<<(slot?",":"")<<module.m_particleSystemID[oldState][slot];std::cout<<"\n";
 std::cout<<"new_ids:";for(int slot=0;slot<12;++slot)std::cout<<(slot?",":"")<<module.m_particleSystemID[newState][slot];std::cout<<"\n";
}
'''

def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=pathlib.Path, required=True)
    parser.add_argument('--cxx', default='c++')
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    source = (ROOT / SOURCE).read_bytes()
    extracted = callback(source.decode())
    harness = PROBES + '\n' + extracted + '\n' + MAIN
    (args.output / 'original_callback.cpp').write_text(extracted)
    (args.output / 'oracle.cpp').write_text(harness)
    exe = args.output / 'oracle'
    build = subprocess.run([args.cxx, '-std=c++17', '-O0', str(args.output / 'oracle.cpp'), '-o', str(exe)], capture_output=True, text=True)
    (args.output / 'build.log').write_text(build.stdout + build.stderr)
    build.check_returncode()
    bits = lambda *slots: sum(1 << slot for slot in slots)
    fixtures = [
        ('sparse_twelve_slots', 0,1,bits(0,4,11),bits(0,2,11),bits(1,4,11),bits(0,5,11),1,1,1,1,0),
        ('fx_mask_off',0,1,4095,4095,4095,bits(2,8),0,1,1,1,0),
        ('ocl_mask_off',0,1,4095,4095,4095,0,1,0,1,1,0),
        ('particle_mask_off',0,1,4095,4095,4095,0,1,1,0,1,0),
        ('no_last_damage_ignores_masks',0,1,bits(11),bits(11),bits(11),bits(0,11),0,0,0,0,0),
        ('improvement_only_cleans_old',1,0,4095,4095,4095,bits(0,4,11),1,1,1,1,0),
        ('same_state_only_cleans_old',1,1,4095,4095,4095,bits(2,11),1,1,1,1,0),
        ('failed_particle_allocation',0,1,bits(4),bits(4),bits(4,11),0,1,1,1,1,bits(4)),
    ]
    output = []
    for fixture in fixtures:
        run = subprocess.run([str(exe), *map(str,fixture[1:])], capture_output=True, text=True, check=True)
        output.append({'name':fixture[0], 'arguments':list(fixture[1:]), 'trace':run.stdout.splitlines()})
    (args.output / 'fixtures.json').write_text(json.dumps(output, indent=2)+'\n')
    pins = {SOURCE:digest(source), 'extracted_callback':digest(extracted.encode()), 'harness':digest(harness.encode()), 'executable':digest(exe.read_bytes()), 'script':digest(pathlib.Path(__file__).read_bytes())}
    (args.output / 'evidence.json').write_text(json.dumps({'scope':__doc__, 'source_sha256':pins, 'fixture_count':len(output), 'fixture_sha256':digest((args.output/'fixtures.json').read_bytes()), 'compiler':subprocess.check_output([args.cxx,'--version'],text=True).splitlines()[0]},indent=2)+'\n')
    print(f'{len(output)} original C++ callback fixtures: {args.output / "fixtures.json"}')

if __name__ == '__main__':
    main()
