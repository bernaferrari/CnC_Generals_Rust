// addplayerdialog.cpp : implementation file
//

#include "stdafx.h"
#include "worldbuilder.h"
#include "addplayerdialog.h"
#include "Common/WellKnownKeys.h"
#include "Common/PlayerTemplate.h"
#include "GameLogic/SidesList.h"
#include "WorldBuilderDoc.h"
#include "CUndoable.h"

/////////////////////////////////////////////////////////////////////////////
// AddPlayerDialog dialog


AddPlayerDialog::AddPlayerDialog(AsciiString side, CWnd* pParent /*=NULL*/)
	: CDialog(AddPlayerDialog::IDD, pParent)
{
	//{{AFX_DATA_INIT(AddPlayerDialog)
		// NOTE: the ClassWizard will add member initialization here
	//}}AFX_DATA_INIT

	m_side = side;
	m_addedSide.clear();
}


void AddPlayerDialog::DoDataExchange(CDataExchange* pDX)
{
	CDialog::DoDataExchange(pDX);
	//{{AFX_DATA_MAP(AddPlayerDialog)
		// NOTE: the ClassWizard will add DDX and DDV calls here
	//}}AFX_DATA_MAP
}


BEGIN_MESSAGE_MAP(AddPlayerDialog, CDialog)
	//{{AFX_MSG_MAP(AddPlayerDialog)
	ON_CBN_EDITCHANGE(IDC_COMBO1, OnEditchangeCombo1)
	//}}AFX_MSG_MAP
END_MESSAGE_MAP()

/////////////////////////////////////////////////////////////////////////////
// AddPlayerDialog message handlers

void AddPlayerDialog::OnOK() 
{	
	CComboBox *faction = (CComboBox*)GetDlgItem(IDC_COMBO1);

	if (faction) 
	{
		// get the text out of the combo. If it is user-typed, sel will be -1, otherwise it will be >=0
		CString theText;
		Int sel = faction->GetCurSel();
		if (sel >= 0) {
			faction->GetLBText(sel, theText);
		} else {
			faction->GetWindowText(theText);
		}
		AsciiString name((LPCTSTR)theText);

		const PlayerTemplate* pt = ThePlayerTemplateStore->findPlayerTemplate(NAMEKEY(name));
		if (pt)
		{
			m_addedSide = pt ? pt->getName() : AsciiString::TheEmptyString;
			SidesList newSides = *TheSidesList;
			newSides.addPlayerByTemplate(m_addedSide);
			Bool modified = newSides.validateSides();
			DEBUG_ASSERTLOG(!modified,("had to clean up sides in AddPlayerDialog::OnOK"));

			CWorldBuilderDoc* pDoc = CWorldBuilderDoc::GetActiveDoc();
			SidesListUndoable *pUndo = new SidesListUndoable(newSides, pDoc);
			pDoc->AddAndDoUndoable(pUndo);
			REF_PTR_RELEASE(pUndo); // belongs to pDoc now.	
		}
	}

	CDialog::OnOK();
}

void AddPlayerDialog::OnCancel() 
{
	m_addedSide.clear();
	CDialog::OnCancel();
}

BOOL AddPlayerDialog::OnInitDialog() 
{
	CDialog::OnInitDialog();
	
	CComboBox *factions = (CComboBox*)GetDlgItem(IDC_COMBO1);
	factions->ResetContent();
	if (ThePlayerTemplateStore)
	{
		for (int i = 0; i < ThePlayerTemplateStore->getPlayerTemplateCount(); i++)
		{
			const PlayerTemplate* pt = ThePlayerTemplateStore->getNthPlayerTemplate(i);
			if (!pt)
				continue;
			if (m_side.isEmpty() || m_side == pt->getSide())
				factions->AddString(pt->getName().str());
		}
	}
	factions->SetCurSel(0);
	
	return TRUE; 
}

void AddPlayerDialog::OnEditchangeCombo1() 
{
	// TODO: Add your control notification handler code here
	
}
