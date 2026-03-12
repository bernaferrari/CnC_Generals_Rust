// FenceTool.h
// Texture tiling tools for worldbuilder.
// Author: John Ahlquist, April 2001

#pragma once

#ifndef FenceTool_H
#define FenceTool_H

#include "Tool.h"
class WorldHeightMapEdit;
class MapObject;
class Vector3;
/*************************************************************************/
/**                             FenceTool
	 Does the fence tool operation. 
***************************************************************************/
class FenceTool : public Tool 
{
protected:
	CPoint		m_downPt2d;
	Coord3D		m_downPt3d;
	MapObject *m_mapObjectList;
	Real			m_curObjectWidth;
	Real			m_curObjectOffset;
	Int				m_objectCount;

public:
	FenceTool(void);
	~FenceTool(void);

protected:
	void updateMapObjectList(Coord3D downPt, Coord3D curPt, WbView* pView, CWorldBuilderDoc *pDoc, Bool checkPlayers); 

public:
	virtual void mouseDown(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual void mouseUp(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual void mouseMoved(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual void activate(); ///< Become the current tool.
	virtual void deactivate(); ///< Become not the current tool.
};


#endif //FenceTool_H
