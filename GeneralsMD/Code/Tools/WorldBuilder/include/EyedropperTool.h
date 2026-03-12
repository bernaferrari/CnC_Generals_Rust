// EyedropperTool.h
// Texture selection tool for worldbuilder.
// Author: John Ahlquist, April 2001

#pragma once

#ifndef EYEDROPPERTOOL_H
#define EYEDROPPERTOOL_H

#include "Tool.h"
class WorldHeightMapEdit;
/*************************************************************************/
/**                             EyedropperTool
	 Does the select tile from drawing window tool operation. 
***************************************************************************/
///  Select tile tool.
class EyedropperTool : public Tool 
{
public:
	EyedropperTool(void);
	~EyedropperTool(void);

public:
	/// Perform tool on mouse down.
	virtual void mouseDown(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual void activate(); ///< Become the current tool.
};


#endif //TOOL_H
