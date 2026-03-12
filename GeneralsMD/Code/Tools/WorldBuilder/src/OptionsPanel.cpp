// OptionsPanel.cpp : implementation file
//

#include "stdafx.h"
#include "worldbuilder.h"
#include "worldbuilderdoc.h"
#include "OptionsPanel.h"

/////////////////////////////////////////////////////////////////////////////
// COptionsPanel dialog


COptionsPanel::COptionsPanel(Int dlgid /*=0*/, CWnd* pParent /*=NULL*/)
	: CDialog(dlgid ? dlgid : COptionsPanel::IDD, pParent)
{
	//{{AFX_DATA_INIT(COptionsPanel)
		// NOTE: the ClassWizard will add member initialization here
	//}}AFX_DATA_INIT
}


void COptionsPanel::DoDataExchange(CDataExchange* pDX)
{
	CDialog::DoDataExchange(pDX);
	//{{AFX_DATA_MAP(COptionsPanel)
		// NOTE: the ClassWizard will add DDX and DDV calls here
	//}}AFX_DATA_MAP
}


BEGIN_MESSAGE_MAP(COptionsPanel, CDialog)
	//{{AFX_MSG_MAP(COptionsPanel)
	ON_WM_MOVE()
	ON_COMMAND(ID_EDIT_REDO, OnEditRedo)
	ON_UPDATE_COMMAND_UI(ID_EDIT_REDO, OnUpdateEditRedo)
	ON_COMMAND(ID_EDIT_UNDO, OnEditUndo)
	ON_UPDATE_COMMAND_UI(ID_EDIT_UNDO, OnUpdateEditUndo)
	//}}AFX_MSG_MAP
END_MESSAGE_MAP()

/////////////////////////////////////////////////////////////////////////////
// COptionsPanel message handlers

void COptionsPanel::OnMove(int x, int y) 
{
	CDialog::OnMove(x, y);
	
	if (this->IsWindowVisible() && !this->IsIconic()) {
		CRect frameRect;
		GetWindowRect(&frameRect);
		::AfxGetApp()->WriteProfileInt(OPTIONS_PANEL_SECTION, "Top", frameRect.top);
		::AfxGetApp()->WriteProfileInt(OPTIONS_PANEL_SECTION, "Left", frameRect.left);
	}
	
}


void COptionsPanel::OnEditRedo() 
{
	// Redirect undo/redo to the doc so they get executed.
	CWorldBuilderDoc *pDoc = CWorldBuilderDoc::GetActiveDoc();
	if (pDoc) {
		pDoc->OnEditRedo();
	}
}

void COptionsPanel::OnUpdateEditRedo(CCmdUI* pCmdUI) 
{
	// Redirect undo/redo to the doc so they get executed.
	CWorldBuilderDoc *pDoc = CWorldBuilderDoc::GetActiveDoc();
	if (pDoc) {
		pDoc->OnUpdateEditRedo(pCmdUI);
	}
}

void COptionsPanel::OnEditUndo() 
{
	// Redirect undo/redo to the doc so they get executed.
	CWorldBuilderDoc *pDoc = CWorldBuilderDoc::GetActiveDoc();
	if (pDoc) {
		pDoc->OnEditUndo();
	}
}

void COptionsPanel::OnUpdateEditUndo(CCmdUI* pCmdUI) 
{
	// Redirect undo/redo to the doc so they get executed.
	CWorldBuilderDoc *pDoc = CWorldBuilderDoc::GetActiveDoc();
	if (pDoc) {
		pDoc->OnUpdateEditUndo(pCmdUI);
	}
}
