#!/usr/bin/env python3
"""Execute extracted original viewport arithmetic, with portable source pins.

Component only: setters, resize, integer pick conversion and local UI shims.
This does not run terrain intersections, GPU rendering or a complete game.
"""
from pathlib import Path
import argparse, hashlib, json, re, subprocess
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--repo-root', type=Path, default=Path(__file__).resolve().parents[4])
parser.add_argument('--output-dir', type=Path, required=True)
parser.add_argument('--cxx', default='clang++')
args = parser.parse_args()
ROOT = args.repo_root.resolve()
OUT = args.output_dir.resolve()
OUT.mkdir(parents=True, exist_ok=True)
sources={
'view':'GeneralsMD/Code/GameEngine/Include/GameClient/View.h',
'display':'GeneralsMD/Code/GameEngine/Source/GameClient/Display.cpp',
'w3d':'GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DView.cpp',
'bar':'GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBar.cpp',
'callback':'GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/ControlBarCallback.cpp',
'convert':'GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/Common/W3DConvert.cpp',
'bar_h':'GeneralsMD/Code/GameEngine/Include/GameClient/ControlBar.h',
'math':'GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/wwmath.h'}
texts={k:(ROOT/p).read_text() for k,p in sources.items()};fragments=[]
def pin(k,begin,end,label):
 text=texts[k];frag=text[begin:end]
 fragments.append({'label':label,'source':sources[k],'source_sha256':hashlib.sha256((ROOT/sources[k]).read_bytes()).hexdigest(),'start_line':text[:begin].count('\n')+1,'end_line':text[:end].count('\n')+1,'fragment_sha256':hashlib.sha256(frag.encode()).hexdigest(),'source_text':frag})
 return frag

def extract(k,marker):
 text=texts[k];start=text.index(marker);brace=text.index('{',start)
 mask=re.sub(r'/\*[\s\S]*?\*/|//[^\n]*|"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])*\'',lambda m:' '*len(m.group()),text)
 depth=0
 for i in range(brace,len(text)):
  if mask[i]=='{':depth+=1
  elif mask[i]=='}':
   depth-=1
   if not depth:return pin(k,start,i+1,marker)
 raise ValueError(marker)
view='\n'.join(extract('view',q) for q in ['virtual void setWidth(', 'virtual Int getWidth(', 'virtual void setHeight(', 'virtual Int getHeight(', 'virtual void setOrigin(', 'virtual void getOrigin('])
enumtext=texts['bar_h'];a=enumtext.index('enum ControlBarStages');b=enumtext.index('};',a)+2;enum=pin('bar_h',a,b,'ControlBarStages')
macros=[]
for key,pattern in [('math',r'#define WWMATH_PI[^\n]*'),('math',r'#define DEG_TO_RADF\(x\)[^\n]*')]:
 m=re.search(pattern,texts[key]);macros.append(pin(key,m.start(),m.end(),pattern))
cpp=r'''#include <iostream>
#include <vector>
#include <string>
#include <cstring>
#include <cstdint>
#include <cstdlib>
using Int=int; using Real=float; using Bool=bool; using UnsignedInt=unsigned;
constexpr Bool TRUE=true,FALSE=false;
std::vector<std::string> events;
#define DEBUG_ASSERTCRASH(condition, msg) do { if (!(condition)) events.push_back("invalid-stage"); } while(0)
'''+ '\n'.join(macros) +'\n'+enum+r'''
struct ICoord2D{Int x=0,y=0;};
struct Vector2{Real X=0,Y=0;Vector2()=default;Vector2(Real x,Real y):X(x),Y(y){}};
struct Camera {Real aspect=1,hfov=0;Vector2 min{0,0},max{1,1};int calls=0;
 void Set_Aspect_Ratio(Real x){aspect=x;++calls;}
 void Get_Viewport(Vector2&a,Vector2&b){a=min;b=max;}
 void Set_Viewport(Vector2 a,Vector2 b){min=a;max=b;}
 void Set_View_Plane(Real x,Real){hfov=x;}
};
struct View{Int m_width=640,m_height=480,m_originX=0,m_originY=0; virtual ~View()=default;
'''+view+r'''
};
struct Mouse{int calls=0;void setMouseLimits(){++calls;}};
Mouse mouse;Mouse*TheMouse=&mouse;
struct Display{UnsignedInt m_width=640,m_height=480;
 UnsignedInt getWidth(){return m_width;} UnsignedInt getHeight(){return m_height;}
 void setWidth(UnsignedInt);void setHeight(UnsignedInt);
 Bool setDisplayMode(UnsignedInt,UnsignedInt,UnsignedInt,Bool);
};
struct W3DView:View {Camera* m_3DCamera;explicit W3DView(Camera*c):m_3DCamera(c){}
 void setWidth(Int)override;void setHeight(Int)override;void setOrigin(Int,Int)override;
};
Display display;Display*TheDisplay=&display;W3DView*TheTacticalView=nullptr;
struct GameWindow{bool hidden=false;ICoord2D pos;
 void winHide(Bool h){hidden=h;events.push_back(h?"window.hide":"window.show");}
 void winSetPosition(Int x,Int y){pos={x,y};events.push_back("window.position");}
};
struct Animate{void reset(){events.push_back("animation.reset");}
 void registerGameWindow(GameWindow*,int,Bool,Int,Int){events.push_back("animation.register");}
 void reverseAnimateWindow(){events.push_back("animation.reverse");}};
constexpr int WIN_ANIMATION_SLIDE_BOTTOM=0,RECORDERMODETYPE_PLAYBACK=1,CP_MASTER=0;
struct Scheme{void setControlBarSchemeByPlayerTemplate(void*,Bool){events.push_back("scheme");}};
struct Player{void*getPlayerTemplate(){return nullptr;}};
struct PlayerList{Player p;Player*getLocalPlayer(){return &p;}};
PlayerList player_list;PlayerList*ThePlayerList=&player_list;
struct Recorder{Int mode=0;Int getMode(){return mode;}};
Recorder recorder;Recorder*TheRecorder=&recorder;
struct ControlBar{ControlBarStages m_currentControlBarStage=CONTROL_BAR_STAGE_DEFAULT;
 GameWindow* m_contextParent[1];ICoord2D m_defaultControlBarPosition{20,400};
 Animate* m_animateWindowManager;Scheme* m_controlBarSchemeManager;
 void repopulateBuildTooltipLayout(){events.push_back("tooltip");}
 void setUpDownImages(){events.push_back("updown");}
 void showSpecialPowerShortcut(){events.push_back("shortcut.show");}
 void hideSpecialPowerShortcut(){events.push_back("shortcut.hide");}
 void animateSpecialPowerShortcut(Bool x){events.push_back(x?"shortcut.animate.show":"shortcut.animate.hide");}
 void hidePurchaseScience(){events.push_back("science.hide");}
 void markUIDirty(){events.push_back("ui.dirty");}
 void switchControlBarStage(ControlBarStages);void setDefaultControlBarConfig();
 void setSquishedControlBarConfig();void setLowControlBarConfig();void setHiddenControlBar();
};
ControlBar*TheControlBar=nullptr;
struct AsciiString{explicit AsciiString(const char*){}};
struct NameKey{Int nameToKey(AsciiString){return 1;}};
NameKey namekey;NameKey*TheNameKeyGenerator=&namekey;
struct WindowManager{GameWindow*parent=nullptr;GameWindow*winGetWindowFromId(void*,Int){return parent;}};
WindowManager wm;WindowManager*TheWindowManager=&wm;
void showReplayControls(){events.push_back("replay.show");}
void hideReplayControls(){events.push_back("replay.hide");}
'''
for key,markers in [
('display',['Bool Display::setDisplayMode(', 'void Display::setWidth(', 'void Display::setHeight(']),
('w3d',['void W3DView::setHeight(', 'void W3DView::setWidth(', 'void W3DView::setOrigin(']),
('bar',['void ControlBar::switchControlBarStage(', 'void ControlBar::setDefaultControlBarConfig(', 'void ControlBar::setSquishedControlBarConfig(', 'void ControlBar::setLowControlBarConfig(', 'void ControlBar::setHiddenControlBar(']),
('callback',['void ShowControlBar(', 'void HideControlBar(']),
('convert',['void PixelScreenToW3DLogicalScreen('])]:
 for marker in markers:cpp+='\n'+extract(key,marker)+'\n'
# The actual constraint's two integer assignments are independent of scene ray shims.
text=texts['w3d'];start=text.index('void W3DView::calcCameraConstraints()');center=[]
for pattern in [r'screen\.x=0\.5f\*getWidth\(\)\+m_originX;',r'screen\.y=0\.5f\*getHeight\(\)\+m_originY;',r'screen\.y = m_originY\+ 0\.95f\*getHeight\(\);']:
 m=re.search(pattern,text[start:]);a=start+m.start();center.append(pin('w3d',a,a+len(m.group()),pattern))
cpp+=r'''
struct Samples{ICoord2D center,bottom;Real cx,cy,bx,by;};
Samples samples(W3DView&v){
 auto getWidth=[&](){return v.getWidth();};auto getHeight=[&](){return v.getHeight();};
 const Int m_originX=v.m_originX,m_originY=v.m_originY;ICoord2D screen;
'''+center[0]+'\n'+center[1]+r'''
 Samples s;s.center=screen;
'''+center[2]+r'''
 s.bottom=screen;
 PixelScreenToW3DLogicalScreen(s.center.x-m_originX,s.center.y-m_originY,&s.cx,&s.cy,getWidth(),getHeight());
 PixelScreenToW3DLogicalScreen(s.bottom.x-m_originX,s.bottom.y-m_originY,&s.bx,&s.by,getWidth(),getHeight());
 return s;
}
uint32_t bits(float x){uint32_t v;std::memcpy(&v,&x,4);return v;}
Camera cam;W3DView view(&cam);GameWindow window;Animate animate;Scheme scheme;ControlBar bar;
void setup(unsigned dw,unsigned dh,int vw=-1,int vh=-1,int ox=0,int oy=0){
 display.m_width=dw;display.m_height=dh;view.m_width=640;view.m_height=480;
 cam=Camera{};TheTacticalView=&view;view.setWidth(vw<0?dw:vw);view.setHeight(vh<0?dh:vh);view.setOrigin(ox,oy);
 bar=ControlBar{};window=GameWindow{};bar.m_contextParent[0]=&window;
 bar.m_animateWindowManager=&animate;bar.m_controlBarSchemeManager=&scheme;TheControlBar=&bar;
 wm.parent=&window;TheWindowManager=&wm;recorder.mode=0;TheRecorder=&recorder;
 events.clear();mouse.calls=0;cam.calls=0;
}
void emit(const char*name){
 std::cout<<"{\"case\":\""<<name<<"\",\"display\":["<<display.m_width<<","<<display.m_height<<"],\"view\":["<<view.m_width<<","<<view.m_height<<","<<view.m_originX<<","<<view.m_originY<<"],\"stage\":"<<int(bar.m_currentControlBarStage)<<",\"hidden\":"<<(window.hidden?"true":"false")<<",\"aspect_bits\":"<<bits(cam.aspect)<<",\"viewport_max_bits\":["<<bits(cam.max.X)<<","<<bits(cam.max.Y)<<"],\"viewport_min_bits\":["<<bits(cam.min.X)<<","<<bits(cam.min.Y)<<"],\"hfov_bits\":"<<bits(cam.hfov)<<",\"aspect_calls\":"<<cam.calls<<",\"mouse_calls\":"<<mouse.calls;
 if(view.getWidth()>0&&view.getHeight()>0){auto s=samples(view);
 std::cout<<",\"center\":["<<s.center.x<<","<<s.center.y<<"],\"bottom\":["<<s.bottom.x<<","<<s.bottom.y<<"],\"ndc_bits\":["<<bits(s.cx)<<","<<bits(s.cy)<<","<<bits(s.bx)<<","<<bits(s.by)<<"]";}
 else std::cout<<",\"samples\":null";
 std::cout<<",\"events\":[";bool first=true;for(auto&e:events){if(!first)std::cout<<",";first=false;std::cout<<"\""<<e<<"\"";}std::cout<<"]}\n";
 events.clear();cam.calls=0;mouse.calls=0;
}
int main(){
 setup(800,600);bar.setDefaultControlBarConfig();emit("default_even");
 setup(801,603);bar.setDefaultControlBarConfig();emit("default_odd");
 bar.switchControlBarStage(CONTROL_BAR_STAGE_HIDDEN);emit("hidden_stage_retains_default_height");
 HideControlBar(TRUE);emit("explicit_hide_expands_after_hidden_stage");
 ShowControlBar(TRUE);emit("show_restores_default_twice");
 bar.switchControlBarStage(CONTROL_BAR_STAGE_LOW);emit("low_stage_full");
 bar.switchControlBarStage(CONTROL_BAR_STAGE_HIDDEN);emit("hidden_stage_retains_low_height");
 setup(801,603);bar.setDefaultControlBarConfig();display.setDisplayMode(1601,1000,32,TRUE);emit("resize_preserves_integer_default_ratio");
 display.setDisplayMode(801,603,32,TRUE);emit("resize_roundtrip_truncates");
 ShowControlBar(FALSE);emit("explicit_show_recomputes_80_percent_after_resize");
 setup(1001,777,801,603,11,19);display.setDisplayMode(1603,1003,32,TRUE);emit("resize_custom_rect_origin");
 display.setDisplayMode(1001,777,32,TRUE);emit("resize_custom_roundtrip");
 setup(801,603,801,482,-3,-7);emit("negative_origin_truncation");
 setup(5,3,5,3,2,3);emit("small_odd_rect");
 setup(2,2);emit("height_two_same_pick_row");
 setup(1,1);emit("height_one_same_pick_row");
 setup(1,1,1,1,-1,-1);emit("negative_origin_crosses_zero_whole_expression_truncates");
 setup(1,1);bar.setDefaultControlBarConfig();emit("default_at_one_height_source_zero");
 setup(801,603,801,333);wm.parent=nullptr;ShowControlBar(TRUE);emit("show_missing_parent_retains_height");
 HideControlBar(FALSE);emit("hide_missing_parent_nonimmediate_retains_height");
 setup(801,603,801,333);TheWindowManager=nullptr;HideControlBar(TRUE);emit("hide_no_window_manager_retains_height");
 setup(801,603);recorder.mode=RECORDERMODETYPE_PLAYBACK;bar.switchControlBarStage(CONTROL_BAR_STAGE_LOW);emit("replay_stage_noop");
 ShowControlBar(TRUE);emit("replay_show_explicit_height_still_applies");
 setup(803,607);bar.setSquishedControlBarConfig();emit("direct_squished_helper_full_not_switch_admitted");
 setup(803,607);bar.switchControlBarStage(CONTROL_BAR_STAGE_SQUISHED);emit("switch_squished_unsupported_source_assert");
 setup(65535,65533,60001,50003,19,29);display.setDisplayMode(32767,32749,32,TRUE);emit("large_finite_resize");
 return 0;
}
'''
(OUT/'oracle.cpp').write_text(cpp)
(OUT/'source-fragments.json').write_text(json.dumps(fragments,indent=2)+'\n')
receipt={'scope':'Original numeric viewport component, not a full UI/render/game executable','fragments':len(fragments),'generator_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'oracle_sha256':hashlib.sha256((OUT/'oracle.cpp').read_bytes()).hexdigest(),'fragment_manifest_sha256':hashlib.sha256((OUT/'source-fragments.json').read_bytes()).hexdigest(),'compiler_version':subprocess.check_output([args.cxx,'--version'],text=True),'builds':[],'exclusions':['Scene pick-ray/world intersections are not executed: only original integer screen assignments and original PixelScreenToW3DLogicalScreen are executed.','UI/window/recorder/player dependencies are explicit local shims; original branch/call order and numeric setter bodies remain intact.','No SLIDE_LETTERBOX preprocessor option, matching normal source build.','Hide missing-parent+immediate=true dereferences null in original; excluded.','Zero-height Default result is recorded as original numeric invalid viewport; sample conversion skipped, not a supported rendering domain.','Squished direct helper exists but switch case is commented out; both are distinguished.']}
for label,flags in [('O0',['-O0']),('O2',['-O2']),('ubsan',['-O1','-fsanitize=undefined','-fno-sanitize-recover=all'])]:
 exe=OUT/('oracle-'+label);argv=[args.cxx,'-std=c++17','-ffp-contract=off',*flags,str(OUT/'oracle.cpp'),'-o',str(exe)]
 build=subprocess.run(argv,capture_output=True,text=True);(OUT/(label+'-build.log')).write_text(build.stdout+build.stderr)
 if build.returncode: raise RuntimeError(label+': '+build.stderr)
 run=subprocess.run([str(exe)],capture_output=True,text=True);(OUT/(label+'.jsonl')).write_text(run.stdout);(OUT/(label+'-run.log')).write_text(run.stderr)
 if run.returncode:raise RuntimeError(label+' run '+run.stderr)
 rows=[json.loads(line) for line in run.stdout.splitlines()];assert len(rows)==26
 receipt['builds'].append({'label':label,'argv':argv,'exit':run.returncode,'executable_sha256':hashlib.sha256(exe.read_bytes()).hexdigest(),'stdout_sha256':hashlib.sha256(run.stdout.encode()).hexdigest(),'stderr_sha256':hashlib.sha256(run.stderr.encode()).hexdigest(),'records':len(rows)})
receipt['outputs_identical']=len({b['stdout_sha256'] for b in receipt['builds']})==1
assert receipt['outputs_identical']
(OUT/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
print(json.dumps({'oracle':str(OUT/'oracle.cpp'),'records':26,'original_fragments':len(fragments),'identical':receipt['outputs_identical'],'receipt_sha256':hashlib.sha256((OUT/'receipt.json').read_bytes()).hexdigest()}))

(OUT / 'expected-records.json').write_text(json.dumps(
    [json.loads(line) for line in (OUT / 'O0.jsonl').read_text().splitlines()], indent=2) + '\n')
