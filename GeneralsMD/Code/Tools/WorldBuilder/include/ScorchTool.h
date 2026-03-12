// ScorchTool.h
// Author: Dennis Griffin, April 2002

#pragma once

#ifndef SCORCHTOOL_H
#define SCORCHTOOL_H

#include "Tool.h"
class WorldHeightMapEdit;
class MapObject;
/*************************************************************************/
/**                             ScorchTool
***************************************************************************/
///  Scorch tool.
class ScorchTool : public Tool 
{
public:
	ScorchTool(void);
	~ScorchTool(void);

protected:
	Coord3D m_mouseDownPt;

protected: 
	MapObject *pickScorch(Coord3D loc);

public:
	/// Perform tool on mouse down.
	virtual void mouseDown(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual void mouseMoved(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual void mouseUp(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual void activate(); ///< Become the current tool.
	virtual void deactivate(); ///< Become not the current tool.
};


#endif //SCORCHTOOL_H
