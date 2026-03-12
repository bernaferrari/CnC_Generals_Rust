// FloodFillTool.cpp
// Texture tiling tool for worldbuilder.
// Author: John Ahlquist, April 2001

#include "StdAfx.h" 
#include "resource.h"

#include "FloodFillTool.h"
#include "CUndoable.h"
#include "DrawObject.h"
#include "MainFrm.h"
#include "WHeightMapEdit.h"
#include "WorldBuilderDoc.h"
#include "WorldBuilderView.h"
#include "TerrainMaterial.h"
//
// FloodFillTool class.
//

Bool FloodFillTool::m_adjustCliffTextures = false;

/// Constructor
FloodFillTool::FloodFillTool(void) :
	Tool(ID_TILE_FLOOD_FILL, IDC_FLOOD_FILL),
	m_cliffCursor(NULL)
{
}
	
/// Destructor
FloodFillTool::~FloodFillTool(void) 
{
	if (m_cliffCursor) {
		::DestroyCursor(m_cliffCursor);
	}
}


/// Shows the terrain materials options panel.
void FloodFillTool::activate() 
{
	CMainFrame::GetMainFrame()->showOptionsDialog(IDD_TERRAIN_MATERIAL);
	TerrainMaterial::setToolOptions(true);
	DrawObject::setDoBrushFeedback(false);
	m_adjustCliffTextures = false;
}

/** Set the cursor. */
void FloodFillTool::setCursor(void)   
{
	if (m_adjustCliffTextures) {
		if (m_cliffCursor == NULL) {
			m_cliffCursor = AfxGetApp()->LoadCursor(MAKEINTRESOURCE(IDC_CLIFF));
		}
		::SetCursor(m_cliffCursor);
	} else {
		Tool::setCursor();
	}
}



/// Left click code.  Sets m_textureClassToDraw and calls eitherMouseDown()
/// Perform the tool behavior on mouse down.
/** Creates a copy of the height map, flood fills it at pt with m_textureClassToDraw which
has been set by the calling routine.  Then builds
the command, and passes it to the doc. */
void FloodFillTool::mouseUp(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc) 
{
	Coord3D cpt;
	pView->viewToDocCoords(viewPt, &cpt);

	CPoint ndx;
	if (!pDoc->getCellIndexFromCoord(cpt, &ndx)) {
		return;
	}

	if (m == TRACK_L)
		m_textureClassToDraw = TerrainMaterial::getFgTexClass();
	else 
		m_textureClassToDraw = TerrainMaterial::getBgTexClass();

//	WorldHeightMapEdit *pMap = pDoc->GetHeightMap();
	WorldHeightMapEdit *htMapEditCopy = pDoc->GetHeightMap()->duplicate();
	Bool didIt = false;
	Bool shiftKey = (0x8000 & ::GetAsyncKeyState(VK_SHIFT))!=0;
	if (m_adjustCliffTextures) {
		didIt = htMapEditCopy->doCliffAdjustment(ndx.x, ndx.y);
	} else {
		didIt = htMapEditCopy->floodFill(ndx.x, ndx.y, m_textureClassToDraw, shiftKey);
	}
	if (didIt) {
		htMapEditCopy->optimizeTiles(); // force to optimize tileset
		IRegion2D partialRange = {0,0,0,0};
		pDoc->updateHeightMap(htMapEditCopy, false, partialRange);
		WBDocUndoable *pUndo = new WBDocUndoable(pDoc, htMapEditCopy);
		pDoc->AddAndDoUndoable(pUndo);
		REF_PTR_RELEASE(pUndo); // belongs to pDoc now.
	} 
	REF_PTR_RELEASE(htMapEditCopy);
}

