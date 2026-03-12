// GroveTool.h
// Author: John Ahlquist, May 2001

#pragma once

#ifndef GroveTool_H
#define GroveTool_H

#include "lib/Basetype.h"
#include "Tool.h"
#include "common/MapObject.h"
class WorldHeightMapEdit;

/*************************************************************************/
/**                             GroveTool
	 Does the add a grove of trees tool operation. 
***************************************************************************/
///  Add a grove of trees tool.
class GroveTool : public Tool 
{
protected:
	enum {HYSTERESIS = 3};
	CPoint m_downPt;
	Bool m_dragging;
	MapObject *m_headMapObj;

protected:
	void plantTree( Coord3D *pos );
	void plantShrub( Coord3D *pos );
	void plantGrove( Coord3D pos, Coord3D prevDir, Real baseHeight, Int level, CPoint bounds );
	void _plantGroveInBox(CPoint tl, CPoint br, WbView* pView);

	void addObj(Coord3D *pos, AsciiString name);
	void activate();

public:
	GroveTool(void);
	~GroveTool(void);

public:
	/// Perform tool on mouse down.
	virtual void mouseDown(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual void mouseUp(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual void mouseMoved(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
};


#endif //TOOL_H
