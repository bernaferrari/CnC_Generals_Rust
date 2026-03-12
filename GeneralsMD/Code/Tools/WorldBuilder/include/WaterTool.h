// WaterTool.h
// Polygon area trigger tool for worldbuilder.
// Author: John Ahlquist, Nov. 2001

#pragma once

#ifndef WaterTool_H
#define WaterTool_H

#include "PolygonTool.h"
class WorldHeightMapEdit;
class MapObject;
class PolygonTrigger;
class MovePolygonUndoable;
/*************************************************************************/
/**                             WaterTool
	 Does the add/select water polygon operation. 
***************************************************************************/
///  Select tile tool.
class WaterTool : public PolygonTool 
{
public:
	WaterTool(void);
	~WaterTool(void);

protected: 
	static Bool		m_water_isActive;

	Real	m_currentZ;

public:
	static Bool isActive(void) {return m_water_isActive;};

public:
	/// Perform tool on mouse down.
	virtual void mouseDown(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual void mouseMoved(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual void mouseUp(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual void setCursor(void);
	virtual void activate(); ///< Become the current tool.
	virtual void deactivate(); ///< Become not the current tool.

protected:
	void fillTheArea(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	PolygonTrigger *adjustSpacing(PolygonTrigger *trigger, Real spacing);
};


#endif //WaterTool_H
