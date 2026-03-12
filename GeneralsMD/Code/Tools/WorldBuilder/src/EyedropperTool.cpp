// EyedropperTool.cpp
// Texture tiling tool for worldbuilder.
// Author: John Ahlquist, April 2001

#include "StdAfx.h" 
#include "resource.h"

#include "EyedropperTool.h"
#include "TerrainMaterial.h"
#include "WHeightMapEdit.h"
#include "WorldBuilderDoc.h"
#include "WorldBuilderView.h"
#include "MainFrm.h"
#include "DrawObject.h"

//
// EyedropperTool class.
//
/// Constructor
EyedropperTool::EyedropperTool(void) :
	Tool(ID_EYEDROPPER_TOOL, IDC_EYEDROPPER) 
{
}
	
/// Destructor
EyedropperTool::~EyedropperTool(void) 
{
}


/// Shows the terrain materials options panel.
void EyedropperTool::activate() 
{
	CMainFrame::GetMainFrame()->showOptionsDialog(IDD_TERRAIN_MATERIAL);
	TerrainMaterial::setToolOptions(true);
	DrawObject::setDoBrushFeedback(false);
}

/// Perform the tool behavior on mouse down.
/** Finds the current texture class, and tells the terrain material panel to use it as fg. */
void EyedropperTool::mouseDown(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc) 
{
	if (m != TRACK_L) return;

	Coord3D cpt;
	pView->viewToDocCoords(viewPt, &cpt);

	CPoint ndx;
	if (!pDoc->getCellIndexFromCoord(cpt, &ndx))
		return;

	WorldHeightMapEdit *pMap = pDoc->GetHeightMap();
	Int texClass = pMap->getTextureClass(ndx.x, ndx.y, true);
	TerrainMaterial::setFgTexClass(texClass);
}

