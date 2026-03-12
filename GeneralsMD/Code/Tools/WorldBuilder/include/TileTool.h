// TileTool.h
// Texture tiling tools for worldbuilder.
// Author: John Ahlquist, April 2001

#pragma once

#ifndef TILETOOL_H
#define TILETOOL_H

#include "Tool.h"
class WorldHeightMapEdit;
/*************************************************************************
**                             TileTool
***************************************************************************/
class TileTool : public Tool 
{
protected:
	WorldHeightMapEdit *m_htMapEditCopy; //< ref counted.
	Int									m_textureClassToDraw;
	CPoint							m_prevViewPt;

public:
	TileTool(void);
	~TileTool(void);

public:
	virtual void mouseDown(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual void mouseUp(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual void mouseMoved(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual WorldHeightMapEdit *getHeightMap(void) {return m_htMapEditCopy;};
	virtual void activate(); ///< Become the current tool.
	virtual Int getWidth(void) {return 1;};
};

/*************************************************************************
**                             BigTileTool
***************************************************************************/
class BigTileTool : public TileTool 
{

protected:
	static Int m_currentWidth;

public:
 	virtual void activate(); ///< Become the current tool.

public:
	BigTileTool(void);

	static void setWidth(Int width) ;
	virtual Int getWidth(void) {return m_currentWidth;};

};
#endif //TOOL_H
