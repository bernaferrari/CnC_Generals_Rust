// ExportScriptsOptions.cpp : implementation file
//

#include "stdafx.h"
#include "worldbuilder.h"
#include "ExportScriptsOptions.h"

/////////////////////////////////////////////////////////////////////////////
// ExportScriptsOptions dialog
Bool ExportScriptsOptions::m_units = true;
Bool ExportScriptsOptions::m_waypoints = true;
Bool ExportScriptsOptions::m_triggers = true;
Bool ExportScriptsOptions::m_allScripts = false;
Bool ExportScriptsOptions::m_sides = true;

ExportScriptsOptions::ExportScriptsOptions(CWnd* pParent /*=NULL*/)
	: CDialog(ExportScriptsOptions::IDD, pParent)
{
	//{{AFX_DATA_INIT(ExportScriptsOptions)
		// NOTE: the ClassWizard will add member initialization here
	//}}AFX_DATA_INIT
}


void ExportScriptsOptions::DoDataExchange(CDataExchange* pDX)
{
	CDialog::DoDataExchange(pDX);
	//{{AFX_DATA_MAP(ExportScriptsOptions)
		// NOTE: the ClassWizard will add DDX and DDV calls here
	//}}AFX_DATA_MAP
}


BEGIN_MESSAGE_MAP(ExportScriptsOptions, CDialog)
	//{{AFX_MSG_MAP(ExportScriptsOptions)
	//}}AFX_MSG_MAP
END_MESSAGE_MAP()

/////////////////////////////////////////////////////////////////////////////
// ExportScriptsOptions message handlers

void ExportScriptsOptions::OnOK() 
{
	// TODO: Add extra validation here
	
	CButton *pButton = (CButton*)GetDlgItem(IDC_WAYPOINTS);
	m_waypoints = pButton->GetCheck()==1;
	pButton = (CButton*)GetDlgItem(IDC_UNITS);
	m_units = pButton->GetCheck()==1;
	pButton = (CButton*)GetDlgItem(IDC_TRIGGERS);
	m_triggers = pButton->GetCheck()==1;
	pButton = (CButton*)GetDlgItem(IDC_ALL_SCRIPTS);
	m_allScripts = pButton->GetCheck()==1;
	pButton = (CButton*)GetDlgItem(IDC_SIDES);
	m_sides = pButton->GetCheck()==1;


	CDialog::OnOK();
}

BOOL ExportScriptsOptions::OnInitDialog() 
{
	CDialog::OnInitDialog();
	
	CButton *pButton = (CButton*)GetDlgItem(IDC_WAYPOINTS);
	pButton->SetCheck(m_waypoints?1:0);
	pButton = (CButton*)GetDlgItem(IDC_UNITS);
	pButton->SetCheck(m_units?1:0);
	pButton = (CButton*)GetDlgItem(IDC_TRIGGERS);
	pButton->SetCheck(m_triggers?1:0);
	pButton = (CButton*)GetDlgItem(IDC_ALL_SCRIPTS);
	pButton->SetCheck(m_allScripts?1:0);
	pButton = (CButton*)GetDlgItem(IDC_SELECTED_SCRIPTS);
	pButton->SetCheck(m_allScripts?0:1);
	pButton = (CButton*)GetDlgItem(IDC_SIDES);
	pButton->SetCheck(m_sides?1:0);

	return TRUE;  // return TRUE unless you set the focus to a control
	              // EXCEPTION: OCX Property Pages should return FALSE
}
