// BuildListTool.h
// Build list tool for worldbuilder.
// Author: John Ahlquist, Nov 2001

#pragma once

#ifndef BUILDLISTTOOL_H
#define BUILDLISTTOOL_H

#include "Tool.h"
#include "Common/AsciiString.h"
#include "PickUnitDialog.h"
class WorldHeightMapEdit;
class BuildListInfo;

/*************************************************************************/
/**                             BuildListTool
	 Does the add item to build list tool operation. 
***************************************************************************/
class BuildListTool : public Tool 
{
protected:
	CPoint m_downPt2d;
	Coord3D m_downPt3d;
	Coord3D m_prevPt3d;
	BuildListInfo *m_curObject;

	Bool m_moving; ///< True if we are drag moving an object.
	Bool m_rotating; ///< True if we are rotating an object.

	Bool m_mouseUpRotate;///< True if we are over the "rotate" hotspot.
	HCURSOR m_rotateCursor;
	Bool m_mouseUpMove;///< True if we are over the "move" hotspot.
	HCURSOR m_moveCursor;
	HCURSOR m_pointerCursor;
	PickUnitDialog m_pickBuildingDlg;
	Bool m_created;

	static PickUnitDialog *m_static_pickBuildingDlg;
	static Bool	m_isActive;

public:
	BuildListTool(void);
	~BuildListTool(void);

private:
	void createWindow(void);
	Bool isDoingAdd(void);

public:
	static void addBuilding(void);
	static Bool isActive(void) {return m_isActive;};

public:
	/// Perform tool on mouse down.
	virtual void mouseDown(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual void mouseUp(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual void mouseMoved(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc);
	virtual void setCursor(void);
	virtual void activate(); ///< Become the current tool.
	virtual void deactivate(); ///< Become not the current tool.
};


#endif //TOOL_H
