// WaypointTool.h
// Texture selection tool for worldbuilder.
// Author: John Ahlquist, April 2001

#pragma once

#ifndef WAYPOINTTOOL_H
#define WAYPOINTTOOL_H

#include "Tool.h"
class WorldHeightMapEdit;
class MapObject;
/*************************************************************************/
/**                             WaypointTool
	 Does the add/select waypoint operation. 
***************************************************************************/
///  Select tile tool.
class WaypointTool : public Tool 
{
public:
	WaypointTool(void);
	~WaypointTool(void);

protected:
	Int m_downWaypointID;
	Coord3D m_mouseDownPt;
	static Bool m_isActive;

protected: 
	MapObject *pickWaypoint(Coord3D loc);

public:
	static Bool isActive(void) {return m_isActive;};

public:
	/// Perform tool on mouse down.
	virtual void mouseDown(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual void mouseMoved(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual void mouseUp(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual void activate(); ///< Become the current tool.
	virtual void deactivate(); ///< Become not the current tool.
};


#endif //WAYPOINTTOOL_H
