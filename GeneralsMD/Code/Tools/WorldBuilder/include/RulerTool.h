// RulerTool.h
// Author: Mike Lytle, January 2003

#pragma once

#ifndef RULER_TOOL_H
#define RULER_TOOL_H

#include "Tool.h"

class RulerTool : public Tool
{
protected:
	Coord3D		m_downPt3d;
	int				m_rulerType; 
	WbView*		m_View;
	Real			m_savedLength;

	static RulerTool*	m_staticThis;

public:
	RulerTool(void);
	~RulerTool(void);

public:
	/// Clear the selection on activate or deactivate.
	virtual void activate();
	virtual void deactivate();

	virtual void setCursor(void);
	virtual void mouseDown(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual void mouseMoved(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual Bool followsTerrain(void) {return false;};	
	
	static void setLength(Real length);
	static Bool switchType();
	static int	getType();
	static Real getLength(void);

};

#endif //RULER_TOOL_H

