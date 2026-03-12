// BlendEdgeTool.h
// Texture tiling tools for worldbuilder.
// Author: John Ahlquist, April 2001

#pragma once

#ifndef BLEND_EDGE_TOOL_H
#define BLEND_EDGE_TOOL_H

#include "Tool.h"
class WorldHeightMapEdit;
/*************************************************************************/
/**                             BlendEdgeTool
	 Does the BlendEdgesOut tool operation. 
***************************************************************************/
///  Blend edges out tool.
class BlendEdgeTool : public Tool 
{
protected:
	Coord3D m_downPt;

public:
	BlendEdgeTool(void);
	~BlendEdgeTool(void);

public:
	/// Perform tool on mouse down.
	virtual void mouseDown(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual void mouseUp(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);

};


#endif //TOOL_H
