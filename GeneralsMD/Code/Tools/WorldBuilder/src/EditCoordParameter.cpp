// EditCoordParameter.cpp : implementation file
//

#include "stdafx.h"
#include "worldbuilder.h"
#include "EditCoordParameter.h"
#include "GameLogic/Scripts.h"
#include "GameLogic/SidesList.h"
#include "GameLogic/PolygonTrigger.h"

/////////////////////////////////////////////////////////////////////////////
// EditCoordParameter dialog


EditCoordParameter::EditCoordParameter(CWnd* pParent /*=NULL*/)
	: CDialog(EditCoordParameter::IDD, pParent)
{
	//{{AFX_DATA_INIT(EditCoordParameter)
		// NOTE: the ClassWizard will add member initialization here
	//}}AFX_DATA_INIT
}


void EditCoordParameter::DoDataExchange(CDataExchange* pDX)
{
	CDialog::DoDataExchange(pDX);
	//{{AFX_DATA_MAP(EditCoordParameter)
		// NOTE: the ClassWizard will add DDX and DDV calls here
	//}}AFX_DATA_MAP
}


BEGIN_MESSAGE_MAP(EditCoordParameter, CDialog)
	//{{AFX_MSG_MAP(EditCoordParameter)
	//}}AFX_MSG_MAP
END_MESSAGE_MAP()

/////////////////////////////////////////////////////////////////////////////
// EditCoordParameter message handlers

BOOL EditCoordParameter::OnInitDialog() 
{
	CDialog::OnInitDialog();
	
	CEdit *pEditX = (CEdit *)GetDlgItem(IDC_EDIT_X);
	CEdit *pEditY = (CEdit *)GetDlgItem(IDC_EDIT_Y);
	CEdit *pEditZ = (CEdit *)GetDlgItem(IDC_EDIT_Z);

	m_parameter->getCoord3D(&m_coord);
	CString string;
	string.Format("%.2f", m_coord.x);
	pEditX->SetWindowText(string);
	string.Format("%.2f", m_coord.y);
	pEditY->SetWindowText(string);
	string.Format("%.2f", m_coord.z);
	pEditZ->SetWindowText(string);

	return FALSE;  // return TRUE unless you set the focus to a control
	              // EXCEPTION: OCX Property Pages should return FALSE
}

void EditCoordParameter::OnOK() 
{
	CEdit *pEditX = (CEdit *)GetDlgItem(IDC_EDIT_X);
	CEdit *pEditY = (CEdit *)GetDlgItem(IDC_EDIT_Y);
	CEdit *pEditZ = (CEdit *)GetDlgItem(IDC_EDIT_Z);
	CString txt;

	pEditX->GetWindowText(txt);
	Real theReal;
	if (1==sscanf(txt, "%f", &theReal)) {
		m_coord.x = theReal;
	} else {
		pEditX->SetFocus();
		::MessageBeep(MB_ICONEXCLAMATION);
		return;
	}
	pEditY->GetWindowText(txt);
	if (1==sscanf(txt, "%f", &theReal)) {
		m_coord.y = theReal;
	} else {
		pEditX->SetFocus();
		::MessageBeep(MB_ICONEXCLAMATION);
		return;
	}
	pEditZ->GetWindowText(txt);
	if (1==sscanf(txt, "%f", &theReal)) {
		m_coord.z = theReal;
	} else {
		pEditX->SetFocus();
		::MessageBeep(MB_ICONEXCLAMATION);
		return;
	}
	m_parameter->friend_setCoord3D(&m_coord);
	CDialog::OnOK();
}

void EditCoordParameter::OnCancel() 
{

	CDialog::OnCancel();
}
