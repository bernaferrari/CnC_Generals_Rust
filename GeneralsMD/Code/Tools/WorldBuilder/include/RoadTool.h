// RoadTool.h
// Texture tiling tools for worldbuilder.
// Author: John Ahlquist, June 2001

#pragma once

#ifndef RoadTool_H
#define RoadTool_H

#include "Tool.h"
#include "W3DDevice/GameClient/WorldHeightMap.h"
class WorldHeightMapEdit;
#define ROAD_SNAP_DISTANCE (1.0f)
/*************************************************************************/
/**                             RoadTool
	 Does the Add a section of road tool operation. 
***************************************************************************/
///  Road segment tool.
class RoadTool : public Tool 
{
protected:
	enum {HYSTERESIS = 3,
				MIN_LENGTH = 4};
	MapObject *m_mapObj;

private:
	MapObject* RoadTool::findSegment(const Coord3D *pLoc, Coord3D *outLoc);

public:
	RoadTool(void);
	~RoadTool(void);

public:
	static Bool snap(Coord3D *pLoc, Bool skipLast);

public:
	/// Perform tool on mouse down.
	virtual void mouseDown(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual void mouseMoved(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual void mouseUp(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual void activate(); ///< Become the current tool.
};


#endif //TOOL_H
