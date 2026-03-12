// ObjectTool.h
// Texture tiling tools for worldbuilder.
// Author: John Ahlquist, April 2001

#pragma once

#ifndef OBJECTTOOL_H
#define OBJECTTOOL_H

#include "Tool.h"
class WorldHeightMapEdit;
/*************************************************************************/
/**                             ObjectTool
	 Does the BlendEdgesOut tool operation. 
***************************************************************************/
///  Blend edges out tool.
class ObjectTool : public Tool 
{
protected:
	CPoint m_downPt2d;
	Coord3D m_downPt3d;

public:
	ObjectTool(void);
	~ObjectTool(void);

public:
	static Real calcAngle(Coord3D downPt, Coord3D curPt, WbView* pView);

public:
	/// Perform tool on mouse down.
	virtual void mouseDown(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual void mouseUp(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual void mouseMoved(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual void activate(); ///< Become the current tool.
	virtual void deactivate(); ///< Become not the current tool.
};


#endif //TOOL_H
