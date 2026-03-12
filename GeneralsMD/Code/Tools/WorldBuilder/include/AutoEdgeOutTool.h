// AutoEdgeOutTool.h
// Texture tiling tools for worldbuilder.
// Author: John Ahlquist, April 2001

#pragma once

#ifndef AUTOEDGEOUTTOOL_H
#define AUTOEDGEOUTTOOL_H

#include "Tool.h"
class WorldHeightMapEdit;
/*************************************************************************/
/**                             AutoEdgeOutTool
	 Does the BlendEdgesOut tool operation. 
***************************************************************************/
///  Blend edges out tool.
class AutoEdgeOutTool : public Tool 
{
public:
	AutoEdgeOutTool(void);
	~AutoEdgeOutTool(void);

public:
	/// Perform tool on mouse down.
	virtual void mouseDown(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual void activate(); ///< Become the current tool.
};


#endif //TOOL_H
