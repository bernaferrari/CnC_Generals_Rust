// HandScrollTool.h
// Scrolling tool for worldbuilder.
// Author: John Ahlquist, May 2001

#pragma once

#ifndef HandScrollTool_H
#define HandScrollTool_H

#include "Tool.h"
/**************************************************************************
                            HandScrollTool
***************************************************************************/
///  Scroll tool.
class HandScrollTool : public Tool 
{
public:
	HandScrollTool(void);
	~HandScrollTool(void);

protected:
	enum {HYSTERESIS = 3};
	CPoint	m_prevPt2d;
	CPoint	m_downPt2d;
	Bool		m_scrolling;
	UINT		m_mouseDownTime;		// if m_trackingMode != TRACK_NONE, tickcount when mouse went down

public:
	/// Start scrolling.
	virtual void mouseDown(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	/// Scroll.
	virtual void mouseMoved(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	/// End scroll.
	virtual void mouseUp(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual void activate(); ///< Become the current tool.
	virtual Bool followsTerrain(void) {return false;};
};


#endif //TOOL_H
