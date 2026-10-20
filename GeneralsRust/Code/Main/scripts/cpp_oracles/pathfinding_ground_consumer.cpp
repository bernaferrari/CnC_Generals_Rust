// Original component bodies are appended verbatim. Adapter contract in receipt.json.
#include <cmath>
#include <iostream>
#include <iomanip>
#include <sstream>
#include <string>
#include <vector>
using Real=float; using Bool=bool; using Int=int;
constexpr Bool FALSE=false;
enum PathfindLayerEnum { LAYER_GROUND=0,LAYER_WALL=1,LAYER_BRIDGE=2 };
constexpr Real PATHFIND_CELL_SIZE_F=10.f; constexpr Int PATHFIND_CELL_SIZE=10;
constexpr Real LAYER_Z_CLOSE_ENOUGH_F=10.f;
struct Coord3D {Real x=0,y=0,z=0;void zero(){x=y=z=0;}};
struct Coord2 {Real x=0,y=0;};struct Region2D{Coord2 lo,hi;};
struct PathfindCell {
 enum CellType{CELL_CLEAR=0,CELL_CLIFF=1,CELL_WATER=2,CELL_OBSTACLE=3,CELL_IMPASSABLE=4,CELL_BRIDGE_IMPASSABLE=5};
 CellType type=CELL_CLEAR;PathfindLayerEnum layer=LAYER_GROUND,connect=LAYER_WALL;bool pinched=false;
 void reset(){type=CELL_CLEAR;connect=LAYER_WALL;pinched=false;}void setLayer(PathfindLayerEnum x){layer=x;}PathfindLayerEnum getLayer()const{return layer;}
 void setType(CellType x){type=x;}CellType getType()const{return type;}void setPinched(bool x){pinched=x;}void releaseInfo(){}
 void setConnectLayer(PathfindLayerEnum x){connect=x;}PathfindLayerEnum getConnectLayer()const{return connect;}
};
struct Bridge {Real height=55;PathfindLayerEnum layer=LAYER_BRIDGE;Bridge*next=nullptr;bool on=true,entry=false,side=false,end=false;
 bool isPointOnBridge(const Coord3D*)const{return on;}Real getBridgeHeight(const Coord3D*,Coord3D*)const{return height;}PathfindLayerEnum getLayer()const{return layer;}Bridge*getNext()const{return next;}
 bool isCellOnSide(const Region2D*)const{return side;}bool isCellOnEnd(const Region2D*)const{return end;}bool isCellEntryPoint(const Region2D*)const{return entry;}
};
struct WaterHandle{};
class TerrainLogic {public:
 Real ground=50,water=50;bool wet=false,cliff=false;Real corners[4]={50,50,50,50};bool corner_mode=false;Bridge*first=nullptr;WaterHandle m_gridWaterHandle,polygon;
 Real getGroundHeight(Real x,Real y)const {if(!corner_mode)return ground;return corners[(x>0?1:0)+(y>0?2:0)];}
 Bridge*getFirstBridge(){return first;}Bridge*findBridgeLayerAt(const Coord3D*,PathfindLayerEnum l,Bool)const{for(auto*p=first;p;p=p->next)if(p->layer==l)return p;return nullptr;}
 const WaterHandle*getWaterHandle(Real,Real){return wet?&polygon:nullptr;}Real getWaterHeight(const WaterHandle*){return water;}Bool isCliffCell(Real,Real){return cliff;}
 PathfindLayerEnum getLayerForDestination(const Coord3D*);Bool isUnderwater(Real,Real,Real*waterZ=nullptr,Real*terrainZ=nullptr);
 virtual Real getLayerHeight(Real,Real,PathfindLayerEnum,Coord3D*normal=nullptr,Bool clip=true)const=0;
};
struct RenderObject {TerrainLogic*owner=nullptr;Real getHeightMapHeight(Real x,Real y,Coord3D*normal){if(normal){normal->zero();normal->z=1;}return owner->getGroundHeight(x,y);}};
static RenderObject*TheTerrainRenderObject=nullptr;
class W3DTerrainLogic:public TerrainLogic {public:Real getLayerHeight(Real,Real,PathfindLayerEnum,Coord3D*normal=nullptr,Bool clip=true)const override;};
class Pathfinder {public:Real wallHeight=20;Bool wall=false;PathfindCell groundCell;Real getWallHeight(){return wallHeight;}Bool isPointOnWall(const Coord3D*){return wall;}PathfindCell*getCell(PathfindLayerEnum,Int,Int){return &groundCell;}
 void adjustCoordToCell(Int,Int,Bool,Coord3D&,PathfindLayerEnum);void classifyMapCell(Int,Int,PathfindCell*);
};
struct AI{Pathfinder*pf;Pathfinder*pathfinder(){return pf;}};static AI*TheAI=nullptr;static TerrainLogic*TheTerrainLogic=nullptr;
struct Visual{void getWaterGridHeight(Real,Real,Real*out){*out=0;}};static Visual*TheTerrainVisual=nullptr;
class PathfindLayer {public:PathfindLayerEnum m_layer=LAYER_BRIDGE;Bridge*m_bridge=nullptr;void classifyLayerMapCell(Int,Int,PathfindCell*,Bridge*);};
#define USE_THE_TERRAIN_OBJECT
PathfindLayerEnum TerrainLogic::getLayerForDestination(const Coord3D *pos)
{
	Bridge *pBridge = getFirstBridge();
	PathfindLayerEnum bestLayer = LAYER_GROUND;
	Real bestDistance = fabs(pos->z - getGroundHeight(pos->x, pos->y));

	if (bestDistance > TheAI->pathfinder()->getWallHeight()/2) {
		// check wall.
		if (TheAI->pathfinder()->isPointOnWall(pos)) {
			Real delta = fabs(pos->z-TheAI->pathfinder()->getWallHeight());
			if (delta<bestDistance) {
				bestLayer = (PathfindLayerEnum)LAYER_WALL;
				bestDistance = delta;
			}
		}
	}

	while (pBridge ) {
		if (pBridge->isPointOnBridge(pos) ) {
			Real delta = fabs(pos->z-pBridge->getBridgeHeight(pos, NULL));
			if (delta<bestDistance) {
				bestLayer = pBridge->getLayer();
				bestDistance = delta;
			}
		}
		pBridge = pBridge->getNext();
	}
	return(bestLayer);
}

Real W3DTerrainLogic::getLayerHeight( Real x, Real y, PathfindLayerEnum layer, Coord3D* normal, Bool clip ) const
{
#ifdef USE_THE_TERRAIN_OBJECT

	if (!TheTerrainRenderObject)
	{
		if (normal)
		{
			//return a default normal pointing up
			normal->x=0.0f;
			normal->y=0.0f;
			normal->z=1.0f;
		}
		return 0;
	}

	Real height = TheTerrainRenderObject->getHeightMapHeight(x,y,normal);

	if (layer != LAYER_GROUND)
	{
		Coord3D loc;
		loc.x = x;
		loc.y = y;
		loc.z = height;
		if (layer == LAYER_WALL)
		{
			if (!clip || TheAI->pathfinder()->isPointOnWall(&loc))
			{
				return TheAI->pathfinder()->getWallHeight();
			}
			else
			{
				return height;
			}
		}
		Bridge* pBridge;
		if ((pBridge = findBridgeLayerAt(&loc, layer, clip)) != 0)
		{
			Real bridgeHeight = pBridge->getBridgeHeight(&loc, normal);
			if (bridgeHeight > height)
			{
				return bridgeHeight;	// Don't return bridge height if it's in the ground.
			}
		}
	}

	return height;

#endif
}

void Pathfinder::adjustCoordToCell(Int cellX, Int cellY, Bool centerInCell, Coord3D &pos, PathfindLayerEnum layer)
{
	if (centerInCell) {
		pos.x = ((Real)cellX + 0.5f) * PATHFIND_CELL_SIZE_F;
		pos.y = ((Real)cellY + 0.5f) * PATHFIND_CELL_SIZE_F;
	} else {
		pos.x = ((Real)cellX+0.05) * PATHFIND_CELL_SIZE_F;
		pos.y = ((Real)cellY+0.05) * PATHFIND_CELL_SIZE_F;
	}
	pos.z = TheTerrainLogic->getLayerHeight( pos.x, pos.y, layer );
}

void Pathfinder::classifyMapCell( Int i, Int j , PathfindCell *cell)
{
	Coord3D topLeftCorner, bottomRightCorner;


	Bool hasObstacle =  (cell->getType() == PathfindCell::CELL_OBSTACLE) ;

	topLeftCorner.y = (Real)j * PATHFIND_CELL_SIZE_F;
	bottomRightCorner.y = topLeftCorner.y + PATHFIND_CELL_SIZE_F;

	topLeftCorner.x = (Real)i * PATHFIND_CELL_SIZE_F;
	bottomRightCorner.x = topLeftCorner.x + PATHFIND_CELL_SIZE_F;

	cell->setPinched(false);

	PathfindCell::CellType type = PathfindCell::CELL_CLEAR;
	if (TheTerrainLogic->isCliffCell(topLeftCorner.x, topLeftCorner.y))
	{
		type = PathfindCell::CELL_CLIFF;
	}

	//
	// If any corners are underwater, this is a water cell
	//
	if (TheTerrainLogic->isUnderwater( topLeftCorner.x, topLeftCorner.y ) ) type = PathfindCell::CELL_WATER;
	if (TheTerrainLogic->isUnderwater( topLeftCorner.x, bottomRightCorner.y) ) type = PathfindCell::CELL_WATER;
	if (TheTerrainLogic->isUnderwater( bottomRightCorner.x, bottomRightCorner.y ) ) type = PathfindCell::CELL_WATER;
	if (TheTerrainLogic->isUnderwater( bottomRightCorner.x, topLeftCorner.y ) ) type = PathfindCell::CELL_WATER;

	if (hasObstacle) {
		type =  PathfindCell::CELL_OBSTACLE;
	}
	cell->setType( type );
	cell->releaseInfo();
}

void PathfindLayer::classifyLayerMapCell( Int i, Int j , PathfindCell *cell, Bridge *theBridge)
{
	Coord3D topLeftCorner, bottomRightCorner;

	topLeftCorner.y = (Real)j * PATHFIND_CELL_SIZE_F;
	bottomRightCorner.y = topLeftCorner.y + PATHFIND_CELL_SIZE_F;

	topLeftCorner.x = (Real)i * PATHFIND_CELL_SIZE_F;
	bottomRightCorner.x = topLeftCorner.x + PATHFIND_CELL_SIZE_F;


	Int bridgeCount = 0;
	Coord3D pt;
	if (theBridge->isPointOnBridge(&topLeftCorner) ) {
		bridgeCount++;
	}
	pt = topLeftCorner;
	pt.y = bottomRightCorner.y;
	if (theBridge->isPointOnBridge(&pt) ) {
		bridgeCount++;
	}
	if (theBridge->isPointOnBridge(&bottomRightCorner) ) {
		bridgeCount++;
	}
	pt = topLeftCorner;
	pt.x = bottomRightCorner.x;
	if (theBridge->isPointOnBridge(&pt) ) {
		bridgeCount++;
	}
	cell->reset( );
	cell->setLayer(m_layer);
	cell->setType(PathfindCell::CELL_IMPASSABLE);
	if (bridgeCount == 4) {
		cell->setType(PathfindCell::CELL_CLEAR);
	} else {
		if (bridgeCount!=0) {
			cell->setType(PathfindCell::CELL_BRIDGE_IMPASSABLE); // it's off the bridge.
		}

		// check against the end lines.

		Region2D cellBounds;
		cellBounds.lo.x = topLeftCorner.x;
		cellBounds.lo.y = topLeftCorner.y;
		cellBounds.hi.x = bottomRightCorner.x;
		cellBounds.hi.y = bottomRightCorner.y;

		if (m_bridge->isCellOnSide(&cellBounds)) {
			cell->setType(PathfindCell::CELL_BRIDGE_IMPASSABLE);
		} else {
			if (m_bridge->isCellOnEnd(&cellBounds)) {
				cell->setType(PathfindCell::CELL_CLEAR);
			}
			if (m_bridge->isCellEntryPoint(&cellBounds)) {
				cell->setType(PathfindCell::CELL_CLEAR);
				cell->setConnectLayer(LAYER_GROUND);
				PathfindCell *groundCell = TheAI->pathfinder()->getCell(LAYER_GROUND, i, j );
				groundCell->setConnectLayer(cell->getLayer());
			}
		}
	}
	Coord3D center = topLeftCorner;
	center.x += PATHFIND_CELL_SIZE/2;
	center.y += PATHFIND_CELL_SIZE/2;
	if (cell->getType()!=PathfindCell::CELL_IMPASSABLE) {
		if (!(cell->getConnectLayer()==LAYER_GROUND) ) {
			// Check for bridge clearance.  If the ground isn't 1 pathfind cells below, mark impassable.
			Real groundHeight = TheTerrainLogic->getLayerHeight( center.x, center.y, LAYER_GROUND );
			Real bridgeHeight = theBridge->getBridgeHeight( &center, NULL );
			if (groundHeight+LAYER_Z_CLOSE_ENOUGH_F > bridgeHeight) {
				PathfindCell *groundCell = TheAI->pathfinder()->getCell(LAYER_GROUND,i, j);
				if (!(groundCell->getType()==PathfindCell::CELL_OBSTACLE)) {
					groundCell->setType(PathfindCell::CELL_BRIDGE_IMPASSABLE);
				}
			}
		}
	}
	return;
}

Bool TerrainLogic::isUnderwater( Real x, Real y, Real *waterZ, Real *terrainZ )
{

	// get the water handle at this location
	const WaterHandle *waterHandle = getWaterHandle( x, y );

	// if no water here, no height, no nuttin
	if( waterHandle == NULL )
  {
    // but we have to return the terrain Z if requested!
    if (terrainZ)
      *terrainZ=getGroundHeight(x,y);
		return FALSE;
  }

	//
	// if this water handle is a grid water use the grid height function, otherwise look into
	// the polygon trigger
	//
	Real wZ = 0.0f;
	if( waterHandle == &m_gridWaterHandle )
		TheTerrainVisual->getWaterGridHeight( x, y, &wZ );
	else
		wZ = getWaterHeight( waterHandle );

	// fill out the waterZ parameter with the water height
	if( waterZ )
		*waterZ = wZ;

	// see if the terrain height here is below the water
	Real terrainHeight = getGroundHeight( x, y );
	if (terrainZ)
		*terrainZ = terrainHeight;

	return terrainHeight < wZ;

}

int main(){std::string line;std::cout<<std::setprecision(9);while(std::getline(std::cin,line)){if(line.empty()||line[0]=='#')continue;
 std::istringstream in(line);std::string tag,mode;in>>tag>>mode;W3DTerrainLogic terrain;Pathfinder path;AI ai{&path};RenderObject render{&terrain};Visual visual;TheAI=&ai;TheTerrainLogic=&terrain;TheTerrainRenderObject=&render;TheTerrainVisual=&visual;
 Bridge bridge;terrain.first=&bridge;Coord3D pos;PathfindCell cell;PathfindLayer layer;layer.m_bridge=&bridge;
 if(mode=="layer"){in>>terrain.ground>>pos.z>>bridge.height;pos.x=50;pos.y=30;auto selected=terrain.getLayerForDestination(&pos);std::cout<<tag<<" layer "<<selected<<" ground "<<terrain.getGroundHeight(pos.x,pos.y)<<"\n";}
 else if(mode=="clearance"){int entry,obstacle;in>>terrain.ground>>bridge.height>>entry>>obstacle;bridge.entry=entry;bridge.on=!entry;path.groundCell.type=obstacle?PathfindCell::CELL_OBSTACLE:PathfindCell::CELL_CLEAR;layer.classifyLayerMapCell(0,0,&cell,&bridge);std::cout<<tag<<" ground_type "<<path.groundCell.type<<" bridge_type "<<cell.type<<" connect "<<cell.connect<<"\n";}
 else if(mode=="adjust"){int x,y,center,l;in>>terrain.ground>>bridge.height>>x>>y>>center>>l;path.adjustCoordToCell(x,y,center,pos,static_cast<PathfindLayerEnum>(l));std::cout<<tag<<" xyz "<<pos.x<<" "<<pos.y<<" "<<pos.z<<"\n";}
 else if(mode=="water"){int obstacle,cliff;in>>terrain.water>>terrain.corners[0]>>terrain.corners[1]>>terrain.corners[2]>>terrain.corners[3]>>obstacle>>cliff;terrain.corner_mode=true;terrain.wet=true;terrain.cliff=cliff;cell.type=obstacle?PathfindCell::CELL_OBSTACLE:PathfindCell::CELL_CLEAR;path.classifyMapCell(0,0,&cell);std::cout<<tag<<" type "<<cell.type<<"\n";}
 else return 2;if(!in)return 3;
 }}
