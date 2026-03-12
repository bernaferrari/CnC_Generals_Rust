// EditGroup.cpp : implementation file
//

#include "stdafx.h"
#include "worldbuilder.h"
#include "EditGroup.h"
#include "GameLogic/Scripts.h"

/////////////////////////////////////////////////////////////////////////////
// EditGroup dialog


EditGroup::EditGroup(ScriptGroup *pGroup, CWnd* pParent /*=NULL*/)
	: CDialog(EditGroup::IDD, pParent),
	m_scriptGroup(pGroup)
{
	//{{AFX_DATA_INIT(EditGroup)
		// NOTE: the ClassWizard will add member initialization here
	//}}AFX_DATA_INIT
}


void EditGroup::DoDataExchange(CDataExchange* pDX)
{
	CDialog::DoDataExchange(pDX);
	//{{AFX_DATA_MAP(EditGroup)
		// NOTE: the ClassWizard will add DDX and DDV calls here
	//}}AFX_DATA_MAP
}


BEGIN_MESSAGE_MAP(EditGroup, CDialog)
	//{{AFX_MSG_MAP(EditGroup)
	//}}AFX_MSG_MAP
END_MESSAGE_MAP()

/////////////////////////////////////////////////////////////////////////////
// EditGroup message handlers

void EditGroup::OnOK() 
{
	CString name;
	GetDlgItem(IDC_GROUP_NAME)->GetWindowText(name);
	m_scriptGroup->setName(AsciiString(name));
	CButton *pButton = (CButton*)GetDlgItem(IDC_GROUP_ACTIVE);
	m_scriptGroup->setActive(pButton->GetCheck()==1);
	pButton = (CButton*)GetDlgItem(IDC_GROUP_SUBROUTINE);
	m_scriptGroup->setSubroutine(pButton->GetCheck()==1);
	CDialog::OnOK();
}

BOOL EditGroup::OnInitDialog() 
{
	CDialog::OnInitDialog();
	
	CButton *pButton = (CButton*)GetDlgItem(IDC_GROUP_ACTIVE);
	pButton->SetCheck(m_scriptGroup->isActive()?1:0);
	pButton = (CButton*)GetDlgItem(IDC_GROUP_SUBROUTINE);
	pButton->SetCheck(m_scriptGroup->isSubroutine()?1:0);

	CEdit *pEdit = (CEdit*)GetDlgItem(IDC_GROUP_NAME);
	pEdit->SetWindowText(m_scriptGroup->getName().str());
	
	return TRUE;  // return TRUE unless you set the focus to a control
	              // EXCEPTION: OCX Property Pages should return FALSE
}
