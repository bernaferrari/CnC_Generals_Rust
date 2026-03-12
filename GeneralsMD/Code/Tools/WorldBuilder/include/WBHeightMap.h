
#ifndef __WBHEIGHTMAP_H_
#define __WBHEIGHTMAP_H_

#include "W3DDevice/GameClient/FlatHeightMap.h"	
#include "W3DDevice/GameClient/HeightMap.h"	
#define dont_USE_FLAT_HEIGHT_MAP // Use the origina height map for mission disk. jba. [4/15/2003]
#ifdef USE_FLAT_HEIGHT_MAP
class WBHeightMap : public FlatHeightMapRenderObjClass
#else
class WBHeightMap : public HeightMapRenderObjClass
#endif	
{	

public:
	WBHeightMap(void);

	/////////////////////////////////////////////////////////////////////////////
	// Render Object Interface (W3D methods)
	/////////////////////////////////////////////////////////////////////////////
	virtual void					Render(RenderInfoClass & rinfo);
	virtual Bool					Cast_Ray(RayCollisionTestClass & raytest);

	virtual Real getHeightMapHeight(Real x, Real y, Coord3D* normal);	///<return height and normal at given point
	virtual Real getMaxCellHeight(Real x, Real y);	///< returns maximum height of the 4 cell corners.

	void setDrawEntireMap(Bool entire) {m_drawEntireMap = entire;};
	Bool getDrawEntireMap(void) {return m_drawEntireMap;};
	void setFlattenHeights(Bool flat);

protected:
	void flattenHeights(void);
protected:
	Bool m_drawEntireMap;
	Bool m_flattenHeights;
};

#endif  // end __WBHEIGHTMAP_H_
