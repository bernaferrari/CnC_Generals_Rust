// AutoEdgeOutTool.cpp
// Texture tiling tool for worldbuilder.
// Author: John Ahlquist, April 2001

#include "StdAfx.h" 
#include "resource.h"

#include "AutoEdgeOutTool.h"
#include "CUndoable.h"
#include "MainFrm.h"
#include "WHeightMapEdit.h"
#include "WorldBuilderDoc.h"
#include "WorldBuilderView.h"
//
// AutoEdgeOutTool class.
//
/// Constructor
AutoEdgeOutTool::AutoEdgeOutTool(void) :
	Tool(ID_AUTO_EDGE_OUT_TOOL, IDC_AUTO_EDGE_OUT) 
{
}
	
/// Destructor
AutoEdgeOutTool::~AutoEdgeOutTool(void) 
{
}

/// Shows the brush options panel.
void AutoEdgeOutTool::activate() 
{
	Tool::activate();
	CMainFrame::GetMainFrame()->showOptionsDialog(IDD_BLEND_MATERIAL);
}


/** Execute the tool on mouse down - Create a copy of the height map
* to edit, blend the edges, and give the undoable command to the doc. */
void AutoEdgeOutTool::mouseDown(TTrackingMode m, CPoint viewPt, WbView* pView, CWorldBuilderDoc *pDoc) 
{
	if (m != TRACK_L) return;

	Coord3D cpt;
	pView->viewToDocCoords(viewPt, &cpt);

	CPoint ndx;
	if (!pDoc->getCellIndexFromCoord(cpt, &ndx)) {
		return;
	}

//	WorldHeightMapEdit *pMap = pDoc->GetHeightMap();
	WorldHeightMapEdit *htMapEditCopy = pDoc->GetHeightMap()->duplicate();
	htMapEditCopy->autoBlendOut(ndx.x, ndx.y, BlendMaterial::getBlendTexClass());
	IRegion2D partialRange = {0,0,0,0};
	pDoc->updateHeightMap(htMapEditCopy, false, partialRange);
	WBDocUndoable *pUndo = new WBDocUndoable(pDoc, htMapEditCopy);
	pDoc->AddAndDoUndoable(pUndo);
	REF_PTR_RELEASE(pUndo); // belongs to pDoc now.
	REF_PTR_RELEASE(htMapEditCopy);
}

