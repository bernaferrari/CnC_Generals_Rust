// FloodFillTool.h
// Texture tiling tools for worldbuilder.
// Author: John Ahlquist, April 2001

#pragma once

#ifndef FloodFillTool_H
#define FloodFillTool_H

#include "Tool.h"
class WorldHeightMapEdit;
/**************************************************************************
                            FloodFillTool
***************************************************************************/
///  Fill area with texture tool.
class FloodFillTool : public Tool 
{
public:
	FloodFillTool(void);
	~FloodFillTool(void);

protected:
	Int			m_textureClassToDraw; ///< The texture to fill with.  Foreground for mousedDown, background for mouseDownRt.
	HCURSOR m_cliffCursor;
	static Bool m_adjustCliffTextures;

public:
	virtual void mouseUp(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual void activate(); ///< Become the current tool.
	virtual void setCursor(void);

	Bool getAdjustCliffs(void) {return m_adjustCliffTextures;}
	void setAdjustCliffs(Bool val) {m_adjustCliffTextures = val;}

};


#endif //TOOL_H
