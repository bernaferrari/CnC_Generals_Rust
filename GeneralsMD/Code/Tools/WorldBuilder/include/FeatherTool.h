// FeatherTool.h
// Texture tiling tools for worldbuilder.
// Author: John Ahlquist, April 2001

#pragma once

#ifndef FEATHERTOOL_H
#define FEATHERTOOL_H

#include "Tool.h"
class WorldHeightMapEdit;
/**************************************************************************/
/**                             FeatherTool
	 Does the smooth height map tool operation. 
***************************************************************************/
///  smooth height map tool.
class FeatherTool : public Tool 
{
protected:
	WorldHeightMapEdit *m_htMapEditCopy; //< ref counted.
	WorldHeightMapEdit *m_htMapFeatherCopy; //< ref counted.
	WorldHeightMapEdit *m_htMapRateCopy; //< ref counted.

	static Int m_feather;
	static Int m_rate;
	static Int m_radius;
public:
	FeatherTool(void);
	~FeatherTool(void);

	static void setFeather(Int feather);
	static void setRate(Int rate);
	static void setRadius(Int Radius);
public:
	virtual void mouseDown(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual void mouseUp(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual void mouseMoved(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual WorldHeightMapEdit *getHeightMap(void) {return m_htMapEditCopy;};
	virtual void activate(); ///< Become the current tool.
};


#endif //FEATHERTOOL_H
