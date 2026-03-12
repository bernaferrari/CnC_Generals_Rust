// BrushTool.h
// Texture tiling tools for worldbuilder.
// Author: John Ahlquist, April 2001

#pragma once

#ifndef BRUSHTOOL_H
#define BRUSHTOOL_H

#include "Tool.h"
class WorldHeightMapEdit;
/*************************************************************************/
/**                             BrushTool
	 Does the Height Brush tool operation. 
***************************************************************************/
///  Height brush tool.
class BrushTool : public Tool 
{
protected:
	WorldHeightMapEdit *m_htMapEditCopy; ///< ref counted.
	WorldHeightMapEdit *m_htMapFeatherCopy; ///< ref counted.

	static Int m_brushWidth;
	static Int m_brushFeather;
	static Bool m_brushSquare;
	static Int m_brushHeight;

public:
	BrushTool(void);
	~BrushTool(void);

public:
	static Int getWidth(void) {return m_brushWidth;};  ///<Returns width.
	static Int getFeather(void) {return m_brushFeather;}; ///<Returns feather.
	static Int getHeight(void) {return m_brushHeight;}; ///<Returns height.
	static void setWidth(Int width);
	static void setFeather(Int feather);
	static void setHeight(Int height);

public:
	virtual void mouseDown(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual void mouseUp(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual void mouseMoved(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual WorldHeightMapEdit *getHeightMap(void) {return m_htMapEditCopy;};
	virtual void activate(); ///< Become the current tool.
	virtual Bool followsTerrain(void) {return false;};

};


#endif //BRUSHTOOL_H
